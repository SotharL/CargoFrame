use super::*;
use crate::oauth::Tokens;
use crate::payload::{Attachment, Field};
use crate::secrets::SecretStore;
use crate::testkit::{ApproveBrowser, World};
use cargoframe_net::mock::{MockResponse, MockServer};

fn connector(server: &MockServer) -> XConnector {
    XConnector::new(model::X {
        client_id: "CID".into(),
        redirect_port: 0,
    })
    .with_endpoints(
        Endpoints {
            auth: "https://x.example/i/oauth2/authorize".into(),
            token: server.url("/2/oauth2/token"),
            revoke: server.url("/2/oauth2/revoke"),
            api: server.base(),
        },
        Duration::from_secs(5),
    )
}

fn signed_in() -> World {
    let w = World::new();
    w.secrets
        .set(
            "x/oauth",
            &serde_json::to_string(&Tokens {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                expires_at: 9_999_999,
            })
            .unwrap(),
        )
        .unwrap();
    w
}

fn x() -> MockServer {
    MockServer::start(|req| match req.path() {
        "/2/users/me" => MockResponse::json(200, json!({"data":{"id":"1","username":"jane"}})),
        "/2/media/upload" => {
            MockResponse::json(200, json!({"data":{"id":"77","media_key":"3_77"}}))
        }
        "/2/media/metadata" => MockResponse::json(200, json!({"data":{"id":"77"}})),
        "/2/tweets" => MockResponse::json(201, json!({"data":{"id":"1850","text":"x"}})),
        "/2/oauth2/token" => MockResponse::json(
            200,
            json!({"access_token":"NEW","refresh_token":"RT2","expires_in":7200,"token_type":"bearer"}),
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
fn sign_in_uses_pkce_the_fixed_callback_and_the_media_scope() {
    let server = x();
    let browser = ApproveBrowser::new();
    let w = World::new();
    let out = w
        .with_browser("x", &browser, |ctx| connector(&server).connect(ctx))
        .unwrap();
    assert_eq!(out, "Signed in to X as @jane");
    let url = browser.opened.lock().unwrap()[0].clone();
    assert!(
        url.starts_with("https://x.example/i/oauth2/authorize?"),
        "{url}"
    );
    assert!(url.contains("code_challenge_method=S256"), "{url}");
    assert!(url.contains("media.write"), "{url}");
    assert!(url.contains("offline.access"), "{url}");
    let token = &server.requests_to("/2/oauth2/token")[0];
    assert_eq!(token.form_value("client_id").as_deref(), Some("CID"));
    assert!(token.form_value("code_verifier").is_some());
    assert_eq!(token.form_value("client_secret"), None);
}

#[test]
fn without_a_client_id_it_says_what_to_paste() {
    let server = x();
    let c = XConnector::new(model::X::default()).with_endpoints(
        Endpoints {
            auth: "https://x.example/a".into(),
            token: server.url("/t"),
            revoke: server.url("/r"),
            api: server.base(),
        },
        Duration::from_secs(1),
    );
    let e = World::new().with("x", |ctx| c.connect(ctx)).unwrap_err();
    assert!(e.message.contains("Client ID"), "{}", e.message);
}

#[test]
fn a_post_uploads_the_picture_describes_it_then_posts_it() {
    let server = x();
    let p = Payload {
        title: "My App - crash".into(),
        image: Some(picture()),
        prompt: Some("It **crashes** on save".into()),
        fields: vec![Field::text("alt", "The error dialog")],
        ..Payload::default()
    };
    let out = signed_in()
        .with("x", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to X");
    assert_eq!(out.url.as_deref(), Some("https://x.com/i/web/status/1850"));

    let upload = &server.requests_to("/2/media/upload")[0];
    assert_eq!(upload.header("authorization"), Some("Bearer AT"));
    let raw = upload.text();
    assert!(
        raw.contains("name=\"media_category\"") && raw.contains("tweet_image"),
        "{raw}"
    );
    assert!(
        raw.contains("name=\"media\"; filename=\"screenshot.png\""),
        "{raw}"
    );

    let alt = server.requests_to("/2/media/metadata")[0].json();
    assert_eq!(alt["id"], "77");
    assert_eq!(alt["metadata"]["alt_text"]["text"], "The error dialog");

    let post = server.requests_to("/2/tweets")[0].json();
    assert_eq!(post["text"], "It crashes on save");
    assert_eq!(post["media"]["media_ids"][0], "77");
}

#[test]
fn a_text_only_post_has_no_media() {
    let server = x();
    let p = Payload {
        title: "My App".into(),
        fields: vec![Field::text("text", "Just words")],
        ..Payload::default()
    };
    signed_in()
        .with("x", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert!(server.requests_to("/2/media/upload").is_empty());
    let post = server.requests_to("/2/tweets")[0].json();
    assert_eq!(post, json!({"text": "Just words"}));
}

#[test]
fn an_expired_sign_in_is_renewed_and_the_new_refresh_token_kept() {
    let server = x();
    let w = World::new();
    w.secrets
        .set(
            "x/oauth",
            &serde_json::to_string(&Tokens {
                access_token: "OLD".into(),
                refresh_token: Some("RT".into()),
                expires_at: 10,
            })
            .unwrap(),
        )
        .unwrap();
    w.with("x", |ctx| connector(&server).test(ctx)).unwrap();
    let me = &server.requests_to("/2/users/me")[0];
    assert_eq!(me.header("authorization"), Some("Bearer NEW"));
    let kept: Tokens = serde_json::from_str(&w.secrets.get("x/oauth").unwrap().unwrap()).unwrap();
    assert_eq!(kept.refresh_token.as_deref(), Some("RT2"));
}

#[test]
fn a_read_only_app_is_explained() {
    let server = MockServer::start(|req| match req.path() {
        "/2/tweets" => MockResponse::json(
            403,
            json!({"title":"Forbidden","detail":"You are not permitted to perform this action."}),
        ),
        _ => MockResponse::new(404),
    });
    let p = Payload {
        title: "t".into(),
        prompt: Some("hello".into()),
        ..Payload::default()
    };
    let w = signed_in();
    let e = w
        .with("x", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(e.message.contains("read and write"), "{}", e.message);
    // refused, so nothing was posted: the next attempt may try again
    let e = w
        .with("x", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(
        !e.message.contains("may already have posted"),
        "{}",
        e.message
    );
}

#[test]
fn a_post_that_may_have_gone_through_is_never_sent_twice() {
    let server = MockServer::start(|_| MockResponse::new(503));
    let p = Payload {
        title: "t".into(),
        prompt: Some("hello".into()),
        ..Payload::default()
    };
    let w = signed_in();
    w.with("x", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    let e = w
        .with("x", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(
        e.message.contains("may already have posted"),
        "{}",
        e.message
    );
}
