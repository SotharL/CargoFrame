use super::*;
use crate::payload::Field;
use crate::testkit::World;
use cargoframe_net::mock::{MockResponse, MockServer, Recorded};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn connector(server: &MockServer) -> SlackConnector {
    SlackConnector::new(model::Slack {}).with_endpoints(Endpoints {
        api: server.url("/api"),
    })
}

fn world() -> World {
    World::new().secret("slack", "token", "xoxb-test")
}

fn ok(extra: Value) -> MockResponse {
    let mut v = json!({ "ok": true });
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    MockResponse::json(200, v)
}

fn refuse(code: &str) -> MockResponse {
    MockResponse::json(200, json!({ "ok": false, "error": code }))
}

fn payload(fields: &[(&str, &str)]) -> Payload {
    Payload {
        title: "My App - crash".into(),
        fields: fields.iter().map(|(k, v)| Field::text(k, v)).collect(),
        ..Payload::default()
    }
}

fn png() -> Attachment {
    Attachment {
        name: "screenshot.png".into(),
        mime: "image/png".into(),
        data: vec![0x89, b'P', b'N', b'G', 1, 2, 3],
    }
}

fn zip() -> Attachment {
    Attachment {
        name: "files.zip".into(),
        mime: "application/zip".into(),
        data: vec![b'P', b'K', 3, 4, 9],
    }
}

/// A Slack that accepts uploads, with a counter of the bytes it received.
fn slack_with_uploads(
    server_base: Arc<std::sync::Mutex<String>>,
) -> impl Fn(&Recorded) -> MockResponse {
    let n = Arc::new(AtomicU32::new(0));
    move |req| match req.path() {
        "/api/files.getUploadURLExternal" => {
            let i = n.fetch_add(1, Ordering::SeqCst) + 1;
            ok(json!({
                "upload_url": format!("{}/upload/{i}", server_base.lock().unwrap()),
                "file_id": format!("F{i}"),
            }))
        }
        p if p.starts_with("/upload/") => MockResponse::text(200, "OK"),
        "/api/files.completeUploadExternal" => ok(json!({ "files": [{ "id": "F1" }] })),
        "/api/chat.postMessage" => ok(json!({ "ts": "1699999999.000100" })),
        "/api/chat.getPermalink" => ok(
            json!({ "permalink": "https://acme.slack.com/archives/C0123ABCD/p1699999999000100" }),
        ),
        _ => MockResponse::new(404),
    }
}

#[test]
fn channel_ids_threads_and_text_are_read_carefully() {
    assert!(is_channel_id("C0123ABCD"));
    assert!(is_channel_id("G01234567AB"));
    assert!(!is_channel_id("general"));
    assert!(!is_channel_id("c0123abcd"));
    assert!(!is_channel_id("X0123ABCD"));

    assert_eq!(
        parse_thread(" 1699999999.000100 ").unwrap(),
        ThreadRef {
            channel: None,
            ts: "1699999999.000100".into()
        }
    );
    assert_eq!(
        parse_thread("https://acme.slack.com/archives/C0123ABCD/p1699999999000100").unwrap(),
        ThreadRef {
            channel: Some("C0123ABCD".into()),
            ts: "1699999999.000100".into()
        }
    );
    // A link to a reply names the thread it belongs to.
    assert_eq!(
        parse_thread("https://acme.slack.com/archives/C0123ABCD/p1700000005000200?thread_ts=1699999999.000100&cid=C0123ABCD")
            .unwrap()
            .ts,
        "1699999999.000100"
    );
    for bad in [
        "",
        "yesterday",
        "1699999999",
        "https://evil.example/archives/C0123ABCD/p1699999999000100",
        "https://acme.slack.com.evil.example/archives/C0123ABCD/p1699999999000100",
        "https://acme.slack.com/archives/general/p1699999999000100",
        "https://acme.slack.com/archives/C0123ABCD/p123",
        "http://acme.slack.com/archives/C0123ABCD/p1699999999000100",
    ] {
        assert!(parse_thread(bad).is_err(), "{bad}");
    }
}

#[test]
fn text_is_escaped_so_a_captured_title_cannot_notify_everyone() {
    assert_eq!(
        escape("a < b & <!channel>"),
        "a &lt; b &amp; &lt;!channel&gt;"
    );

    let mut p = payload(&[]);
    p.title = "<!channel> App".into();
    p.prompt = Some("It crashed\non save".into());
    // The prompt wins over the window title.
    assert_eq!(compose(&p), "It crashed\non save");

    // With nothing written, the window title says what the picture is.
    p.prompt = None;
    assert_eq!(compose(&p), "&lt;!channel&gt; App");

    // A bold first line when the layout maps one; the message field wins over the prompt.
    let mut p = payload(&[("title", "  Crash *report* "), ("message", "Steps & notes")]);
    p.prompt = Some("ignored".into());
    assert_eq!(compose(&p), "*Crash report*\nSteps &amp; notes");

    // Nothing at all.
    let empty = Payload::default();
    assert_eq!(compose(&empty), "");
}

#[test]
fn a_test_says_who_it_signed_in_as_and_a_bad_token_asks_for_a_new_one() {
    let server = MockServer::start(|req| match req.path() {
        "/api/auth.test" if req.header("authorization") == Some("Bearer xoxb-test") => {
            ok(json!({ "team": "Acme", "user": "cargoframe" }))
        }
        "/api/auth.test" => refuse("invalid_auth"),
        _ => MockResponse::new(404),
    });
    let w = world();
    let line = w.with("slack", |ctx| connector(&server).test(ctx)).unwrap();
    assert_eq!(line, "Signed in to Acme as cargoframe");

    let bad = World::new().secret("slack", "token", "xoxb-old");
    let e = bad
        .with("slack", |ctx| connector(&server).test(ctx))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::AuthExpired);
    assert!(e.message.contains("paste a new one"), "{}", e.message);
}

#[test]
fn without_a_token_it_says_what_to_do() {
    let server = MockServer::start(|_| MockResponse::new(500));
    let w = World::new();
    let c = connector(&server);
    assert_eq!(w.with("slack", |ctx| c.status(ctx)), Status::Disconnected);
    let e = w.with("slack", |ctx| c.test(ctx)).unwrap_err();
    assert_eq!(e.kind, FailureKind::ConnectorUnavailable);
    assert!(e.message.contains("Connections"));
    assert!(server.requests().is_empty());

    let w = world();
    assert_eq!(w.with("slack", |ctx| c.status(ctx)), Status::Connected);
    w.with("slack", |ctx| c.disconnect(ctx));
    assert_eq!(w.with("slack", |ctx| c.status(ctx)), Status::Disconnected);
}

#[test]
fn channels_come_sorted_and_grouped_across_pages() {
    let server = MockServer::start(|req| {
        if req.path() != "/api/conversations.list" {
            return MockResponse::new(404);
        }
        match req.query_param("cursor").as_deref() {
            None => ok(json!({
                "channels": [
                    { "id": "C2", "name": "random", "is_private": false },
                    { "id": "G1", "name": "secret", "is_private": true },
                ],
                "response_metadata": { "next_cursor": "page 2" }
            })),
            Some("page 2") => ok(json!({
                "channels": [{ "id": "C1", "name": "general", "is_private": false }],
                "response_metadata": { "next_cursor": "" }
            })),
            Some(_) => refuse("invalid_cursor"),
        }
    });
    let w = world();
    let items = w
        .with("slack", |ctx| connector(&server).options(ctx, "channels"))
        .unwrap();
    let names: Vec<_> = items
        .iter()
        .map(|i| (i.value.as_str(), i.label.as_str()))
        .collect();
    assert_eq!(
        names,
        [("C1", "#general"), ("C2", "#random"), ("G1", "#secret")]
    );
    assert_eq!(items[0].group.as_deref(), Some("Public channels"));
    assert_eq!(items[2].group.as_deref(), Some("Private channels"));
    let first = &server.requests()[0];
    assert_eq!(
        first.query_param("types").as_deref(),
        Some("public_channel,private_channel")
    );
    assert_eq!(
        first.query_param("exclude_archived").as_deref(),
        Some("true")
    );

    assert!(w
        .with("slack", |ctx| connector(&server).options(ctx, "emoji"))
        .is_err());
}

#[test]
fn a_message_goes_out_as_json_with_the_channel_and_thread() {
    let base = Arc::new(std::sync::Mutex::new(String::new()));
    let server = MockServer::start(slack_with_uploads(base.clone()));
    *base.lock().unwrap() = server.base();
    let w = world();
    let mut p = payload(&[
        ("channel", "C0123ABCD"),
        (
            "thread",
            "https://acme.slack.com/archives/C0123ABCD/p1699999999000100",
        ),
    ]);
    p.prompt = Some("Crash <here>".into());
    let out = w
        .with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted in the Slack thread");
    assert_eq!(
        out.url.as_deref(),
        Some("https://acme.slack.com/archives/C0123ABCD/p1699999999000100")
    );

    let post = &server.requests_to("/api/chat.postMessage")[0];
    assert_eq!(post.header("authorization"), Some("Bearer xoxb-test"));
    assert_eq!(
        post.json(),
        json!({ "channel": "C0123ABCD", "text": "Crash &lt;here&gt;", "thread_ts": "1699999999.000100" })
    );
    // Nothing was uploaded.
    assert!(server
        .requests_to("/api/files.getUploadURLExternal")
        .is_empty());
}

#[test]
fn a_channel_typed_by_name_is_looked_up() {
    let server = MockServer::start(|req| match req.path() {
        "/api/conversations.list" => ok(json!({
            "channels": [{ "id": "C0123ABCD", "name": "bugs", "is_private": false }]
        })),
        "/api/chat.postMessage" => ok(json!({ "ts": "1699999999.000100" })),
        _ => ok(json!({})),
    });
    let w = world();
    let mut p = payload(&[("channel", "#Bugs")]);
    p.prompt = Some("hi".into());
    w.with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(
        server.requests_to("/api/chat.postMessage")[0].json()["channel"],
        "C0123ABCD"
    );

    let missing = World::new().secret("slack", "token", "xoxb-test");
    let mut p = payload(&[("channel", "#nope")]);
    p.prompt = Some("hi".into());
    let e = missing
        .with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(
        e.message.contains("no channel called #nope"),
        "{}",
        e.message
    );
}

#[test]
fn the_screenshot_and_the_zip_are_uploaded_then_shared_with_the_message() {
    let base = Arc::new(std::sync::Mutex::new(String::new()));
    let server = MockServer::start(slack_with_uploads(base.clone()));
    *base.lock().unwrap() = server.base();
    let w = world();
    let mut p = payload(&[("channel", "C0123ABCD")]);
    p.prompt = Some("Look at this".into());
    p.image = Some(png());
    p.files_zip = Some(zip());
    let out = w
        .with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to Slack");

    let starts = server.requests_to("/api/files.getUploadURLExternal");
    assert_eq!(starts.len(), 2);
    assert_eq!(
        starts[0].form_value("filename").as_deref(),
        Some("screenshot.png")
    );
    assert_eq!(starts[0].form_value("length").as_deref(), Some("7"));
    assert_eq!(
        starts[1].form_value("filename").as_deref(),
        Some("files.zip")
    );

    // The bytes go to the address Slack gave, and the token does not go with them.
    let up = &server.requests_to("/upload/1")[0];
    assert_eq!(up.body, png().data);
    assert!(up.header("authorization").is_none());
    assert_eq!(server.requests_to("/upload/2")[0].body, zip().data);

    let done = &server.requests_to("/api/files.completeUploadExternal")[0];
    assert_eq!(
        done.json(),
        json!({
            "files": [{ "id": "F1", "title": "screenshot.png" }, { "id": "F2", "title": "files.zip" }],
            "channel_id": "C0123ABCD",
            "initial_comment": "Look at this",
        })
    );
    // With files, the message travels as their comment: no second post.
    assert!(server.requests_to("/api/chat.postMessage").is_empty());
}

#[test]
fn a_picture_taken_from_a_recording_is_uploaded_beside_it() {
    let base = Arc::new(std::sync::Mutex::new(String::new()));
    let server = MockServer::start(slack_with_uploads(base.clone()));
    *base.lock().unwrap() = server.base();
    let w = world();
    let mut p = payload(&[("channel", "C0123ABCD")]);
    p.image = Some(Attachment {
        name: "recording.mp4".into(),
        mime: "video/mp4".into(),
        data: vec![0, 0, 0, 24],
    });
    p.still = Some(png());
    w.with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let names: Vec<Option<String>> = server
        .requests_to("/api/files.getUploadURLExternal")
        .iter()
        .map(|r| r.form_value("filename"))
        .collect();
    assert_eq!(
        names,
        [Some("recording.mp4".to_string()), Some("screenshot.png".to_string())]
    );
}

#[test]
fn a_retry_does_not_upload_or_post_twice() {
    let base = Arc::new(std::sync::Mutex::new(String::new()));
    let fail_complete = Arc::new(AtomicU32::new(1));
    let inner = slack_with_uploads(base.clone());
    let fc = fail_complete.clone();
    let server = MockServer::start(move |req| {
        if req.path() == "/api/files.completeUploadExternal" && fc.swap(0, Ordering::SeqCst) == 1 {
            return MockResponse::text(502, "bad gateway");
        }
        inner(req)
    });
    *base.lock().unwrap() = server.base();
    let w = world();
    let mut p = payload(&[("channel", "C0123ABCD")]);
    p.image = Some(png());
    let c = connector(&server);

    let first = w.with("slack", |ctx| c.send(ctx, &p));
    assert_eq!(first.unwrap_err().kind, FailureKind::Server(502));
    w.with("slack", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(
        server.requests_to("/api/files.getUploadURLExternal").len(),
        1
    );
    assert_eq!(server.requests_to("/upload/1").len(), 1);
    assert_eq!(
        server
            .requests_to("/api/files.completeUploadExternal")
            .len(),
        2
    );

    // Sending again after success changes nothing.
    w.with("slack", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(
        server
            .requests_to("/api/files.completeUploadExternal")
            .len(),
        2
    );

    // The same for a plain message.
    let w2 = world();
    let mut q = payload(&[("channel", "C0123ABCD")]);
    q.prompt = Some("hi".into());
    w2.with("slack", |ctx| c.send(ctx, &q)).unwrap();
    w2.with("slack", |ctx| c.send(ctx, &q)).unwrap();
    assert_eq!(server.requests_to("/api/chat.postMessage").len(), 1);
}

#[test]
fn an_upload_address_that_is_not_slacks_is_not_used() {
    let server = MockServer::start(|req| match req.path() {
        "/api/files.getUploadURLExternal" => ok(json!({
            "upload_url": "https://slack.com.evil.example/upload/1",
            "file_id": "F1",
        })),
        _ => ok(json!({})),
    });
    let w = world();
    let mut p = payload(&[("channel", "C0123ABCD")]);
    p.image = Some(png());
    let e = w
        .with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(e.message.contains("not Slack's"), "{}", e.message);
    assert_eq!(server.requests().len(), 1);

    let c = connector(&server);
    for good in [
        "https://files.slack.com/upload/v1/abc",
        "https://files-edge.slack.com:443/upload/v1/abc",
        "https://slack.com/upload",
    ] {
        let real = SlackConnector::new(model::Slack {});
        assert!(real.trusted_upload(good), "{good}");
    }
    for bad in [
        "https://slack.com.evil.example/x",
        "https://evil.example/slack.com/",
        "https://user@files.slack.com/x",
        "http://files.slack.com/x",
        "ftp://files.slack.com/x",
    ] {
        let real = SlackConnector::new(model::Slack {});
        assert!(!real.trusted_upload(bad), "{bad}");
    }
    // Only a local test server may stand in for Slack, and only when Slack itself is replaced.
    assert!(c.trusted_upload(&server.url("/upload/1")));
    assert!(!SlackConnector::new(model::Slack {}).trusted_upload(&server.url("/upload/1")));
}

#[test]
fn slacks_refusals_are_explained_and_classified() {
    let cases = [
        (
            "channel_not_found",
            FailureKind::Rejected(200),
            "invite the app first",
        ),
        (
            "not_in_channel",
            FailureKind::Rejected(200),
            "not in that channel",
        ),
        ("is_archived", FailureKind::Rejected(200), "archived"),
        ("msg_too_long", FailureKind::Rejected(200), "too long"),
        ("token_revoked", FailureKind::AuthExpired, "paste a new one"),
        ("ratelimited", FailureKind::Throttled(429), "slow down"),
        ("internal_error", FailureKind::Server(500), "on its side"),
        ("something_new", FailureKind::Rejected(200), "something_new"),
    ];
    for (code, kind, text) in cases {
        let e = api_error(&json!({ "ok": false, "error": code }), "post the message");
        assert_eq!(e.kind, kind, "{code}");
        assert!(
            e.message.starts_with("Could not post the message: "),
            "{}",
            e.message
        );
        assert!(e.message.contains(text), "{code}: {}", e.message);
    }
    let e = api_error(
        &json!({ "ok": false, "error": "missing_scope", "needed": "chat:write" }),
        "post the message",
    );
    assert!(e.message.contains("\"chat:write\""), "{}", e.message);

    // And through a real answer: HTTP 200 with ok false is a failure, a 429 keeps its wait.
    let server = MockServer::start(|req| match req.path() {
        "/api/chat.postMessage" => refuse("not_in_channel"),
        _ => MockResponse::new(429)
            .header("Retry-After", "7")
            .body(b"{}".to_vec()),
    });
    let w = world();
    let mut p = payload(&[("channel", "C0123ABCD")]);
    p.prompt = Some("hi".into());
    let e = w
        .with("slack", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(e.message.contains("not in that channel"));
    assert!(!e.kind.is_retryable());

    let mut with_file = payload(&[("channel", "C0123ABCD")]);
    with_file.image = Some(png());
    let w = world();
    let e = w
        .with("slack", |ctx| connector(&server).send(ctx, &with_file))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::Throttled(429));
    assert_eq!(e.retry_after, Some(std::time::Duration::from_secs(7)));
}

#[test]
fn problems_in_the_layout_are_caught_before_anything_is_sent() {
    let server = MockServer::start(|_| MockResponse::new(500));
    let w = world();
    let c = connector(&server);

    let no_channel = payload(&[]);
    let e = w.with("slack", |ctx| c.send(ctx, &no_channel)).unwrap_err();
    assert!(e.message.contains("needs a channel"));

    let mut other_channel = payload(&[
        ("channel", "C0000AAAA"),
        (
            "thread",
            "https://acme.slack.com/archives/C0123ABCD/p1699999999000100",
        ),
    ]);
    other_channel.prompt = Some("hi".into());
    let e = w
        .with("slack", |ctx| c.send(ctx, &other_channel))
        .unwrap_err();
    assert!(e.message.contains("different channel"));

    let bad_thread = payload(&[("channel", "C0123ABCD"), ("thread", "last tuesday")]);
    let e = w.with("slack", |ctx| c.send(ctx, &bad_thread)).unwrap_err();
    assert!(e.message.contains("not a Slack thread"));

    let mut nothing = payload(&[("channel", "C0123ABCD")]);
    nothing.title = String::new();
    let e = w.with("slack", |ctx| c.send(ctx, &nothing)).unwrap_err();
    assert!(e.message.contains("nothing to send"));

    assert!(server.requests().is_empty());
}

#[test]
fn it_describes_what_a_button_can_fill() {
    let d = SlackConnector::new(model::Slack {}).describe();
    let keys: Vec<_> = d.fields.iter().map(|f| f.key).collect();
    assert_eq!(keys, ["channel", "thread", "message", "title"]);
    assert_eq!(d.fields[0].list, Some("channels"));
    assert_eq!(d.lists[0].id, "channels");
}
