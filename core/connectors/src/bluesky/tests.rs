use super::*;
use crate::payload::{Attachment, Field};
use crate::testkit::World;
use cargoframe_net::mock::{MockResponse, MockServer};

fn connector(server: &MockServer) -> BlueskyConnector {
    BlueskyConnector::new(model::Bluesky {
        handle: "@Jane.bsky.social".into(),
        service: server.base(),
    })
}

fn signed_in() -> World {
    World::new().secret("bsky", "app_password", "abcd-efgh-ijkl-mnop")
}

fn bluesky() -> MockServer {
    MockServer::start(|req| match req.path() {
        "/xrpc/com.atproto.server.createSession" => {
            if req.json()["password"] == "abcd-efgh-ijkl-mnop" {
                MockResponse::json(
                    200,
                    json!({"did":"did:plc:jane","handle":"jane.bsky.social","accessJwt":"JWT","refreshJwt":"R"}),
                )
            } else {
                MockResponse::json(
                    401,
                    json!({"error":"AuthenticationRequired","message":"Invalid identifier or password"}),
                )
            }
        }
        "/xrpc/com.atproto.repo.uploadBlob" => MockResponse::json(
            200,
            json!({"blob":{"$type":"blob","ref":{"$link":"bafk"},"mimeType":"image/png","size":6}}),
        ),
        "/xrpc/com.atproto.repo.createRecord" => MockResponse::json(
            200,
            json!({"uri":"at://did:plc:jane/app.bsky.feed.post/3kabc","cid":"bafy"}),
        ),
        _ => MockResponse::new(404),
    })
}

fn picture() -> Attachment {
    Attachment {
        name: "screenshot.png".into(),
        mime: "image/png".into(),
        data: vec![0x89, b'P', b'N', b'G', 1, 2],
    }
}

#[test]
fn handles_are_cleaned_and_checked() {
    for (raw, want) in [
        ("jane.bsky.social", "jane.bsky.social"),
        ("@Jane.bsky.social", "jane.bsky.social"),
        (
            "https://bsky.app/profile/jane.example.com/",
            "jane.example.com",
        ),
    ] {
        assert_eq!(clean_handle(raw).unwrap(), want, "{raw}");
    }
    for bad in [
        "",
        "jane",
        "jane..social",
        "-jane.social",
        "jane doe.social",
    ] {
        assert!(clean_handle(bad).is_err(), "{bad}");
    }
}

#[test]
fn links_are_marked_in_bytes_and_stop_at_the_end_of_a_sentence() {
    let text = "Café crash: https://example.com/bug/42. See http://x.io";
    let facets = link_facets(text);
    assert_eq!(facets.len(), 2);
    let start = facets[0]["index"]["byteStart"].as_u64().unwrap() as usize;
    let end = facets[0]["index"]["byteEnd"].as_u64().unwrap() as usize;
    // "é" is two bytes, so the byte count is one more than the character count
    assert_eq!(start, "Café crash: ".len());
    assert_eq!(&text[start..end], "https://example.com/bug/42");
    assert_eq!(facets[1]["features"][0]["uri"], "http://x.io");
    assert!(link_facets("no links here, nor https:// alone").is_empty());
}

#[test]
fn the_web_address_of_a_post_is_made_from_its_record() {
    assert_eq!(
        post_url(
            "at://did:plc:jane/app.bsky.feed.post/3kabc",
            "jane.bsky.social"
        )
        .as_deref(),
        Some("https://bsky.app/profile/jane.bsky.social/post/3kabc")
    );
    assert_eq!(
        post_url("at://did:plc:jane/app.bsky.feed.like/1", "j"),
        None
    );
}

#[test]
fn time_is_written_the_way_bluesky_reads_it() {
    assert_eq!(iso_time(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso_time(1_794_096_194), "2026-11-08T00:03:14Z");
    assert_eq!(iso_time(951_782_400), "2000-02-29T00:00:00Z");
}

#[test]
fn test_says_who_is_signed_in() {
    let server = bluesky();
    let out = signed_in()
        .with("bsky", |ctx| connector(&server).test(ctx))
        .unwrap();
    assert_eq!(out, "Signed in to Bluesky as @jane.bsky.social");
    let req = &server.requests()[0];
    assert_eq!(req.json()["identifier"], "jane.bsky.social");
}

#[test]
fn a_wrong_app_password_is_said_plainly() {
    let server = bluesky();
    let w = World::new().secret("bsky", "app_password", "wrong");
    let e = w
        .with("bsky", |ctx| connector(&server).test(ctx))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::AuthExpired);
    assert!(e.message.contains("app password"), "{}", e.message);
}

#[test]
fn without_an_app_password_it_is_not_connected() {
    let server = bluesky();
    let status = World::new().with("bsky", |ctx| connector(&server).status(ctx));
    assert_eq!(status, Status::Disconnected);
    let status = signed_in().with("bsky", |ctx| connector(&server).status(ctx));
    assert_eq!(status, Status::Connected);
}

#[test]
fn a_post_carries_the_text_the_picture_and_its_description() {
    let server = bluesky();
    let p = Payload {
        title: "My App - crash".into(),
        image: Some(picture()),
        prompt: Some("It **crashes** on save, see https://example.com/42".into()),
        fields: vec![Field::text("alt", "The error dialog")],
        ..Payload::default()
    };
    let out = signed_in()
        .with("bsky", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to Bluesky as @jane.bsky.social");
    assert_eq!(
        out.url.as_deref(),
        Some("https://bsky.app/profile/jane.bsky.social/post/3kabc")
    );

    let upload = &server.requests_to("/xrpc/com.atproto.repo.uploadBlob")[0];
    assert_eq!(upload.header("authorization"), Some("Bearer JWT"));
    assert_eq!(upload.header("content-type"), Some("image/png"));

    let post = server.requests_to("/xrpc/com.atproto.repo.createRecord")[0].json();
    assert_eq!(post["repo"], "did:plc:jane");
    let record = &post["record"];
    assert_eq!(
        record["text"],
        "It crashes on save, see https://example.com/42"
    );
    assert_eq!(
        record["facets"][0]["features"][0]["uri"],
        "https://example.com/42"
    );
    assert_eq!(record["embed"]["$type"], "app.bsky.embed.images");
    assert_eq!(record["embed"]["images"][0]["alt"], "The error dialog");
    assert_eq!(
        record["embed"]["images"][0]["image"]["ref"]["$link"],
        "bafk"
    );
    assert!(record["createdAt"].as_str().unwrap().ends_with('Z'));
}

#[test]
fn a_long_text_is_cut_and_the_note_says_so() {
    let server = bluesky();
    let p = Payload {
        title: "t".into(),
        prompt: Some("word ".repeat(100)),
        ..Payload::default()
    };
    let out = signed_in()
        .with("bsky", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let post = server.requests_to("/xrpc/com.atproto.repo.createRecord")[0].json();
    assert!(post["record"]["text"].as_str().unwrap().chars().count() <= MAX_TEXT);
    assert!(out.notes.iter().any(|n| n.contains("cut to 300")));
}

#[test]
fn a_recording_sends_its_still_and_files_are_left_out_with_a_note() {
    let server = bluesky();
    let p = Payload {
        title: "t".into(),
        image: Some(Attachment {
            name: "clip.mp4".into(),
            mime: "video/mp4".into(),
            data: vec![0; 8],
        }),
        still: Some(picture()),
        files_zip: Some(Attachment {
            name: "f.zip".into(),
            mime: "application/zip".into(),
            data: vec![1],
        }),
        ..Payload::default()
    };
    let out = signed_in()
        .with("bsky", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(
        server
            .requests_to("/xrpc/com.atproto.repo.uploadBlob")
            .len(),
        1
    );
    assert!(out.notes.iter().any(|n| n.contains("not the recording")));
    assert!(out.notes.iter().any(|n| n.contains("files")));
}

#[test]
fn a_post_that_may_have_gone_through_is_never_sent_twice() {
    let server = MockServer::start(|req| match req.path() {
        "/xrpc/com.atproto.server.createSession" => MockResponse::json(
            200,
            json!({"did":"did:plc:jane","handle":"jane.bsky.social","accessJwt":"JWT"}),
        ),
        _ => MockResponse::new(502),
    });
    let w = signed_in();
    let p = Payload {
        title: "t".into(),
        prompt: Some("hello".into()),
        ..Payload::default()
    };
    let first = w
        .with("bsky", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(first.message.contains("HTTP 502"), "{}", first.message);
    // a server failure leaves the outcome unknown: the next attempt does not post again
    let again = w
        .with("bsky", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(
        again.message.contains("may already have posted"),
        "{}",
        again.message
    );
}
