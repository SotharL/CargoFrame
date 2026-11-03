//! The real client against the local mock server.

use cargoframe_net::mock::{MockResponse, MockServer};
use cargoframe_net::{Body, Http, Method, Multipart, NetError, Request, UreqHttp};
use std::time::{Duration, Instant};

fn client() -> UreqHttp {
    UreqHttp::new()
}

#[test]
fn get_sends_headers_and_returns_status_headers_and_body() {
    let server = MockServer::start(|_| {
        MockResponse::json(200, serde_json::json!({"ok": true})).header("Retry-After", "7")
    });
    let req = Request::get(server.url("/a?x=1&y=two%20words")).header("X-Test", "hello");
    let resp = client().send(&req).unwrap();
    assert!(resp.is_success());
    assert_eq!(resp.json().unwrap()["ok"], true);
    assert_eq!(resp.retry_after(), Some(Duration::from_secs(7)));
    assert_eq!(resp.header("content-type"), Some("application/json"));

    let seen = &server.requests()[0];
    assert_eq!(seen.method, "GET");
    assert_eq!(seen.path(), "/a");
    assert_eq!(seen.query_param("y").as_deref(), Some("two words"));
    assert_eq!(seen.header("x-test"), Some("hello"));
    assert!(seen
        .header("user-agent")
        .unwrap()
        .starts_with("CargoFrame/"));
    assert!(seen.body.is_empty());
}

#[test]
fn error_statuses_are_answers_not_errors() {
    let server = MockServer::start(|_| MockResponse::text(404, "nope"));
    let resp = client().send(&Request::get(server.url("/x"))).unwrap();
    assert_eq!(resp.status, 404);
    assert_eq!(resp.text(), "nope");
    assert!(!resp.is_success());
}

#[test]
fn json_form_and_multipart_bodies_arrive_intact() {
    let server = MockServer::start(|_| MockResponse::ok());
    let http = client();

    http.send(
        &Request::new(Method::Post, server.url("/json"))
            .body(Body::json(&serde_json::json!({"a": [1, 2]}))),
    )
    .unwrap();
    http.send(&Request::post(server.url("/form")).body(Body::form(&[("q", "a b&c"), ("é", "ü")])))
        .unwrap();
    let png = vec![0x89, b'P', b'N', b'G', 0, 1, 2, 255];
    http.send(&Request::post(server.url("/multi")).body(Body::multipart(
        Multipart::form_data().field("title", "Bug").file(
            "file",
            "shot.png",
            "image/png",
            png.clone(),
        ),
    )))
    .unwrap();

    let by_path = |p: &str| server.requests_to(p).remove(0);
    let json = by_path("/json");
    assert_eq!(json.header("content-type"), Some("application/json"));
    assert_eq!(json.json()["a"][1], 2);

    let form = by_path("/form");
    assert_eq!(
        form.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(form.form_value("q").as_deref(), Some("a b&c"));
    assert_eq!(form.form_value("é").as_deref(), Some("ü"));

    let multi = by_path("/multi");
    assert!(multi
        .header("content-type")
        .unwrap()
        .starts_with("multipart/form-data; boundary="));
    // the binary file is present byte for byte
    assert!(multi.body.windows(png.len()).any(|w| w == png.as_slice()));
    assert!(multi.text().contains("filename=\"shot.png\""));
}

#[test]
fn a_slow_server_times_out() {
    let server = MockServer::start(|_| MockResponse::ok().delay(Duration::from_secs(3)));
    let started = Instant::now();
    let err = client()
        .send(&Request::get(server.url("/slow")).timeout(Duration::from_millis(300)))
        .unwrap_err();
    assert_eq!(err, NetError::Timeout);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_closed_port_is_offline() {
    // bind then drop to get a port nothing listens on
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let err = client()
        .send(&Request::get(format!("http://127.0.0.1:{port}/")).timeout(Duration::from_secs(3)))
        .unwrap_err();
    assert!(matches!(err, NetError::Offline(_)), "{err:?}");
}

#[test]
fn only_https_or_this_computer() {
    let err = client()
        .send(&Request::get("http://example.com/"))
        .unwrap_err();
    assert!(matches!(err, NetError::BadUrl(_)), "{err:?}");
    let err = client()
        .send(&Request::get("https://user:pw@example.com/"))
        .unwrap_err();
    assert!(matches!(err, NetError::BadUrl(_)), "{err:?}");
}

#[test]
fn a_header_with_a_line_break_is_refused_not_sent() {
    let server = MockServer::start(|_| MockResponse::ok());
    let err = client()
        .send(&Request::get(server.url("/")).header("X-A", "b\r\nX-Injected: 1"))
        .unwrap_err();
    assert!(matches!(err, NetError::Other(_)), "{err:?}");
    assert!(server.requests().is_empty());
}

#[test]
fn redirects_do_not_carry_the_sign_in_header_to_another_host() {
    let other = MockServer::start(|_| MockResponse::text(200, "landed"));
    let target = other.url("/landing");
    let first = MockServer::start(move |_| MockResponse::new(302).header("Location", &target));
    let resp = client()
        .send(&Request::get(first.url("/start")).header("Authorization", "Bearer secret"))
        .unwrap();
    assert_eq!(resp.text(), "landed");
    assert_eq!(
        first.requests()[0].header("authorization"),
        Some("Bearer secret")
    );
    // 127.0.0.1 with another port is another origin: the header must not follow
    assert_eq!(other.requests()[0].header("authorization"), None);
}

#[test]
fn a_huge_answer_is_cut_off() {
    // 17 MB, above the 16 MB cap
    let server = MockServer::start(|_| MockResponse::ok().body(vec![b'x'; 17 * 1024 * 1024]));
    let err = client()
        .send(&Request::get(server.url("/big")))
        .unwrap_err();
    assert!(matches!(err, NetError::TooLarge { .. }), "{err:?}");
}

#[test]
fn a_request_can_refuse_to_follow_redirects() {
    let other = MockServer::start(|_| MockResponse::text(200, "landed"));
    let target = other.url("/landing");
    let first = MockServer::start(move |_| MockResponse::new(302).header("Location", &target));
    let resp = client()
        .send(
            &Request::get(first.url("/start"))
                .header("X-Api-Key", "secret")
                .no_redirects(),
        )
        .unwrap();
    assert_eq!(resp.status, 302);
    assert!(resp.header("location").unwrap().ends_with("/landing"));
    assert!(
        other.requests().is_empty(),
        "the redirect must not have been followed"
    );
}

#[test]
fn a_307_keeps_the_method_and_body_but_a_302_turns_a_post_into_a_get() {
    let landing = MockServer::start(|_| MockResponse::text(200, "landed"));
    let target = landing.url("/landing");
    let t = target.clone();
    let temp = MockServer::start(move |_| MockResponse::new(307).header("Location", &t));
    let t = target.clone();
    let found = MockServer::start(move |_| MockResponse::new(302).header("Location", &t));

    client()
        .send(&Request::post(temp.url("/")).body(Body::text("text/plain", "payload")))
        .unwrap();
    let kept = &landing.requests()[0];
    assert_eq!(
        (kept.method.as_str(), kept.text()),
        ("POST", "payload".into())
    );

    client()
        .send(&Request::post(found.url("/")).body(Body::text("text/plain", "payload")))
        .unwrap();
    let turned = &landing.requests()[1];
    assert_eq!(turned.method, "GET");
    assert!(turned.body.is_empty());
}

#[test]
fn the_sign_in_header_stays_on_a_redirect_within_the_same_site() {
    let server = MockServer::start(|req| {
        if req.path() == "/start" {
            MockResponse::new(302).header("Location", "/final")
        } else {
            MockResponse::text(200, "landed")
        }
    });
    let resp = client()
        .send(&Request::get(server.url("/start")).header("Authorization", "Bearer secret"))
        .unwrap();
    assert_eq!(resp.text(), "landed");
    assert_eq!(
        server.requests_to("/final")[0].header("authorization"),
        Some("Bearer secret")
    );
}

#[test]
fn a_redirect_to_another_kind_of_address_is_refused() {
    for location in [
        "ftp://example.com/f",
        "http://example.com/plain",
        "javascript:1",
    ] {
        let server =
            MockServer::start(move |_| MockResponse::new(302).header("Location", location));
        let err = client().send(&Request::get(server.url("/"))).unwrap_err();
        assert!(matches!(err, NetError::BadUrl(_)), "{location}: {err:?}");
    }
}

#[test]
fn a_redirect_loop_ends_with_the_last_redirect_answer() {
    let server = MockServer::start(|req| {
        let n: u32 = req
            .query_param("n")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        MockResponse::new(302).header("Location", &format!("/loop?n={}", n + 1))
    });
    let resp = client()
        .send(&Request::get(server.url("/loop?n=0")))
        .unwrap();
    assert_eq!(resp.status, 302);
    assert_eq!(server.requests().len(), 6, "the first call plus five hops");
}

// ----- downloads straight to a writer -----

#[test]
fn a_download_is_written_as_it_arrives_and_progress_is_reported() {
    let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    let served = payload.clone();
    let server = MockServer::start(move |_| MockResponse::ok().body(served.clone()));
    let mut out = Vec::new();
    let mut seen: Vec<(u64, Option<u64>)> = Vec::new();
    let done = client()
        .download(
            &Request::get(server.url("/file.bin")).header("Accept", "application/octet-stream"),
            &mut out,
            10 * 1024 * 1024,
            &mut |d, t| seen.push((d, t)),
        )
        .unwrap();
    assert!(done.is_success());
    assert_eq!(done.bytes, payload.len() as u64);
    assert_eq!(out, payload);
    assert!(seen.len() > 1, "progress is reported more than once");
    assert_eq!(
        seen.last().copied(),
        Some((payload.len() as u64, Some(payload.len() as u64)))
    );
    assert!(
        seen.windows(2).all(|w| w[0].0 < w[1].0),
        "progress only goes forward"
    );
    assert_eq!(
        server.requests()[0].header("accept"),
        Some("application/octet-stream")
    );
}

#[test]
fn a_download_that_is_refused_writes_nothing_and_says_what_the_server_said() {
    let server = MockServer::start(|_| MockResponse::text(404, "Not Found"));
    let mut out = Vec::new();
    let done = client()
        .download(
            &Request::get(server.url("/gone")),
            &mut out,
            1024,
            &mut |_, _| {},
        )
        .unwrap();
    assert_eq!(done.status, 404);
    assert!(!done.is_success());
    assert_eq!(done.message, "Not Found");
    assert!(out.is_empty());
}

#[test]
fn a_download_larger_than_the_limit_is_stopped() {
    let server = MockServer::start(|_| MockResponse::ok().body(vec![7u8; 50_000]));
    let mut out = Vec::new();
    let err = client()
        .download(
            &Request::get(server.url("/big")),
            &mut out,
            10_000,
            &mut |_, _| {},
        )
        .unwrap_err();
    assert!(err.to_string().contains("larger than"), "{err}");
    assert!(out.len() <= 10_000, "nothing past the limit is written");
}

#[test]
fn a_download_follows_a_redirect_without_the_sign_in_header_to_another_host() {
    let storage = MockServer::start(|_| MockResponse::ok().body(b"installer bytes".to_vec()));
    let target = storage.url("/signed?token=abc");
    let api = MockServer::start(move |_| MockResponse::new(302).header("Location", &target));
    let mut out = Vec::new();
    let done = client()
        .download(
            &Request::get(api.url("/assets/1")).header("Authorization", "Bearer secret"),
            &mut out,
            1024 * 1024,
            &mut |_, _| {},
        )
        .unwrap();
    assert!(done.is_success());
    assert_eq!(out, b"installer bytes");
    assert_eq!(
        api.requests()[0].header("authorization"),
        Some("Bearer secret")
    );
    assert_eq!(storage.requests()[0].header("authorization"), None);
}

#[test]
fn a_client_without_downloads_says_so() {
    struct Plain;
    impl Http for Plain {
        fn send(&self, _: &Request) -> Result<cargoframe_net::Response, NetError> {
            Err(NetError::Timeout)
        }
    }
    let err = Plain
        .download(
            &Request::get("https://example.com/x"),
            &mut Vec::new(),
            1,
            &mut |_, _| {},
        )
        .unwrap_err();
    assert!(err.to_string().contains("cannot download"));
}
