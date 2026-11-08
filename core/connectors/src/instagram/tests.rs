use super::*;
use crate::payload::{Attachment, Field};
use crate::secrets::SecretStore;
use crate::testkit::World;
use cargoframe_net::mock::{MockResponse, MockServer};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn settings(server: &MockServer) -> model::Instagram {
    model::Instagram {
        upload_url: server.url("/bucket/{name}"),
        public_url: "https://cdn.example.com/shots/{name}".into(),
        host_header: "AccessKey".into(),
    }
}

fn connector(server: &MockServer) -> InstagramConnector {
    InstagramConnector::new(settings(server)).with_endpoints(Endpoints {
        graph: server.url("/v23.0"),
        refresh: server.url("/refresh_access_token"),
        poll: Duration::from_millis(5),
        polls: 5,
    })
}

fn signed_in() -> World {
    World::new()
        .secret("ig", "token", "IGTOKEN")
        .secret("ig", "host_key", "KEY")
}

/// A real PNG, so it can be made into a JPEG.
fn picture() -> Attachment {
    let mut data = Vec::new();
    {
        let mut e = png::Encoder::new(&mut data, 4, 3);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .unwrap()
            .write_image_data(&[200u8; 4 * 3 * 4])
            .unwrap();
    }
    Attachment {
        name: "screenshot.png".into(),
        mime: "image/png".into(),
        data,
    }
}

/// Instagram, a picture server, and how many looks it takes before the picture is fetched.
fn instagram(ready_after: u32) -> MockServer {
    let looks = Arc::new(AtomicU32::new(0));
    MockServer::start(move |req| match req.path() {
        "/v23.0/me" => MockResponse::json(200, json!({"user_id":"1789","username":"acme"})),
        p if p.starts_with("/bucket/") => MockResponse::new(201),
        "/v23.0/1789/media" => MockResponse::json(200, json!({"id":"C1"})),
        "/v23.0/C1" => {
            let n = looks.fetch_add(1, Ordering::SeqCst);
            let status = if n + 1 >= ready_after {
                "FINISHED"
            } else {
                "IN_PROGRESS"
            };
            MockResponse::json(200, json!({"status_code": status, "id":"C1"}))
        }
        "/v23.0/1789/media_publish" => MockResponse::json(200, json!({"id":"M9"})),
        "/v23.0/M9" => MockResponse::json(
            200,
            json!({"permalink":"https://www.instagram.com/p/ABC/","id":"M9"}),
        ),
        "/refresh_access_token" => {
            MockResponse::json(200, json!({"access_token":"RENEWED","expires_in":5184000}))
        }
        _ => MockResponse::new(404),
    })
}

fn payload() -> Payload {
    Payload {
        title: "My App - crash".into(),
        image: Some(picture()),
        fields: vec![Field::text("caption", "New **build** is out")],
        ..Payload::default()
    }
}

#[test]
fn a_post_hosts_the_picture_waits_for_instagram_then_publishes() {
    let server = instagram(2);
    let w = signed_in();
    let out = w
        .with("ig", |ctx| connector(&server).send(ctx, &payload()))
        .unwrap();
    assert_eq!(out.summary, "Posted to Instagram as @acme");
    assert_eq!(out.url.as_deref(), Some("https://www.instagram.com/p/ABC/"));

    // the picture went to the person's server, as a JPEG, with its key
    let put = server
        .requests()
        .into_iter()
        .find(|r| r.path().starts_with("/bucket/cargoframe-"))
        .unwrap();
    assert_eq!(put.header("content-type"), Some("image/jpeg"));
    assert_eq!(put.header("accesskey"), Some("KEY"));
    let name = put.path().trim_start_matches("/bucket/").to_string();
    assert!(name.ends_with(".jpg"));

    // Instagram was told where to fetch it, with the caption in plain text
    let draft = server.requests_to("/v23.0/1789/media")[0].json();
    assert_eq!(
        draft["image_url"],
        format!("https://cdn.example.com/shots/{name}")
    );
    assert_eq!(draft["caption"], "New build is out");
    assert_eq!(
        server.requests_to("/v23.0/1789/media")[0].header("authorization"),
        Some("Bearer IGTOKEN")
    );
    assert_eq!(server.requests_to("/v23.0/C1").len(), 2);
    assert_eq!(
        server.requests_to("/v23.0/1789/media_publish")[0].json()["creation_id"],
        "C1"
    );

    // and the token was renewed for another 60 days
    assert_eq!(
        w.secrets.get("ig/token").unwrap().as_deref(),
        Some("RENEWED")
    );
}

#[test]
fn a_post_needs_a_picture() {
    let server = instagram(1);
    let mut p = payload();
    p.image = None;
    let e = signed_in()
        .with("ig", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(e.message.contains("needs a picture"), "{}", e.message);
}

#[test]
fn without_the_picture_server_it_says_what_is_missing() {
    let server = instagram(1);
    let c = InstagramConnector::new(model::Instagram::default()).with_endpoints(Endpoints {
        graph: server.url("/v23.0"),
        refresh: server.url("/refresh_access_token"),
        poll: Duration::from_millis(1),
        polls: 1,
    });
    let e = signed_in()
        .with("ig", |ctx| c.send(ctx, &payload()))
        .unwrap_err();
    assert!(e.message.contains("{name}"), "{}", e.message);
}

#[test]
fn a_picture_instagram_cannot_fetch_is_explained() {
    let server = MockServer::start(|req| match req.path() {
        "/v23.0/me" => MockResponse::json(200, json!({"user_id":"1789","username":"acme"})),
        p if p.starts_with("/bucket/") => MockResponse::new(200),
        "/v23.0/1789/media" => MockResponse::json(200, json!({"id":"C1"})),
        "/v23.0/C1" => MockResponse::json(200, json!({"status_code":"ERROR"})),
        _ => MockResponse::new(404),
    });
    let e = signed_in()
        .with("ig", |ctx| connector(&server).send(ctx, &payload()))
        .unwrap_err();
    assert!(e.message.contains("opens in a browser"), "{}", e.message);
}

#[test]
fn a_slow_fetch_is_a_timeout_and_the_next_attempt_does_not_upload_again() {
    let server = instagram(100);
    let w = signed_in();
    let e = w
        .with("ig", |ctx| connector(&server).send(ctx, &payload()))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::Timeout);
    w.with("ig", |ctx| connector(&server).send(ctx, &payload()))
        .unwrap_err();
    let uploads = server
        .requests()
        .iter()
        .filter(|r| r.path().starts_with("/bucket/"))
        .count();
    assert_eq!(uploads, 1);
    assert_eq!(server.requests_to("/v23.0/1789/media").len(), 1);
}

#[test]
fn an_expired_token_says_how_to_make_a_new_one() {
    let server = MockServer::start(|_| {
        MockResponse::json(
            400,
            json!({"error":{"message":"Error validating access token","type":"OAuthException","code":190}}),
        )
    });
    let e = signed_in()
        .with("ig", |ctx| connector(&server).test(ctx))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::AuthExpired);
    assert!(e.message.contains("Meta app dashboard"), "{}", e.message);
}

#[test]
fn test_says_who_is_signed_in() {
    let server = instagram(1);
    let out = signed_in()
        .with("ig", |ctx| connector(&server).test(ctx))
        .unwrap();
    assert_eq!(out, "Signed in to Instagram as @acme");
}
