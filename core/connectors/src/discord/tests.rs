use super::*;
use crate::payload::Field;
use crate::testkit::World;
use cargoframe_net::mock::{MockResponse, MockServer, Recorded};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

const GUILD: &str = "111111111111111111";
const GUILD2: &str = "222222222222222222";
const TEXT: &str = "333333333333333333";
const FORUM: &str = "444444444444444444";
const VOICE: &str = "555555555555555555";
const THREAD: &str = "666666666666666666";
const MESSAGE: &str = "777777777777777777";
const CATEGORY: &str = "888888888888888888";
const HOOK: &str = "999000999000999000";
const HOOK_TOKEN: &str = "aB-cd_EF123";

fn connector(server: &MockServer) -> DiscordConnector {
    DiscordConnector::new(model::Discord::default()).with_endpoints(Endpoints {
        api: server.url("/api/v10"),
    })
}

fn world() -> World {
    World::new().secret("dc", "token", "bot-test")
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

/// A Discord with one server, a text channel, a forum, a voice channel and a thread.
fn discord(req: &Recorded) -> MockResponse {
    let p = req.path().trim_start_matches("/api/v10");
    match p {
        "/users/@me" => MockResponse::json(
            200,
            json!({ "username": "cargoframe", "global_name": null }),
        ),
        "/users/@me/guilds" => MockResponse::json(200, json!([{ "id": GUILD, "name": "Acme" }])),
        p if p == format!("/guilds/{GUILD}/channels") => MockResponse::json(
            200,
            json!([
                { "id": TEXT, "name": "Bugs", "type": 0, "parent_id": CATEGORY, "position": 2 },
                { "id": FORUM, "name": "feedback", "type": 15, "position": 5 },
                { "id": VOICE, "name": "talk", "type": 2 },
                { "id": CATEGORY, "name": "Support", "type": 4, "position": 1 },
            ]),
        ),
        p if p == format!("/guilds/{GUILD}/threads/active") => MockResponse::json(
            200,
            json!({ "threads": [{ "id": THREAD, "name": "Crash on save", "parent_id": TEXT }] }),
        ),
        p if p == format!("/channels/{TEXT}") => {
            MockResponse::json(200, json!({ "id": TEXT, "type": 0, "guild_id": GUILD }))
        }
        p if p == format!("/channels/{FORUM}") => {
            MockResponse::json(200, json!({ "id": FORUM, "type": 15, "guild_id": GUILD }))
        }
        p if p == format!("/channels/{VOICE}") => {
            MockResponse::json(200, json!({ "id": VOICE, "type": 2, "guild_id": GUILD }))
        }
        p if p == format!("/channels/{THREAD}") => {
            MockResponse::json(200, json!({ "id": THREAD, "type": 11, "guild_id": GUILD }))
        }
        p if p.ends_with("/messages") => MockResponse::json(200, json!({ "id": MESSAGE })),
        p if p.ends_with("/threads") => MockResponse::json(
            200,
            json!({ "id": "999999999999999999", "guild_id": GUILD }),
        ),
        _ => MockResponse::new(404),
    }
}

/// The JSON a multipart request carries in its `payload_json` part.
fn payload_json(req: &Recorded) -> Value {
    let text = req.text();
    let start = text
        .find("name=\"payload_json\"\r\n\r\n")
        .expect("a payload_json part")
        + "name=\"payload_json\"\r\n\r\n".len();
    let end = text[start..]
        .find("\r\n--")
        .map(|e| start + e)
        .unwrap_or(text.len());
    serde_json::from_str(&text[start..end]).expect("payload_json is JSON")
}

#[test]
fn places_are_read_from_ids_and_links() {
    assert_eq!(parse_place(TEXT).unwrap(), TEXT);
    assert_eq!(
        parse_place(&format!(
            "https://discord.com/channels/{GUILD}/{TEXT}/{MESSAGE}"
        ))
        .unwrap(),
        TEXT
    );
    assert_eq!(
        parse_place(&format!(
            "https://canary.discord.com/channels/{GUILD}/{THREAD}"
        ))
        .unwrap(),
        THREAD
    );
    assert_eq!(
        parse_place(&format!("https://discordapp.com/channels/@me/{TEXT}?x=1")).unwrap(),
        TEXT
    );
    for bad in [
        "",
        "general",
        "123",
        "https://evil.example/channels/111111111111111111/333333333333333333",
        "https://discord.com.evil.example/channels/111111111111111111/333333333333333333",
        "http://discord.com/channels/111111111111111111/333333333333333333",
        "https://discord.com/channels/abc/333333333333333333",
        "https://discord.com/channels/111111111111111111",
        "https://discord.com/users/111111111111111111/333333333333333333",
    ] {
        assert!(parse_place(bad).is_err(), "{bad}");
    }
}

#[test]
fn the_message_is_composed_and_cut_to_discords_limit() {
    let mut p = payload(&[("title", " Crash  *report* ")]);
    p.prompt = Some("It crashed".into());
    assert_eq!(
        compose(&p, true),
        ("**Crash report**\nIt crashed".to_string(), false)
    );
    // In a forum the title is the post's name, not part of the text.
    assert_eq!(compose(&p, false), ("It crashed".to_string(), false));
    assert_eq!(post_name(&p), "Crash *report*");

    // The window title says what the picture is when nothing was written.
    let bare = payload(&[]);
    assert_eq!(compose(&bare, true).0, "My App - crash");
    assert_eq!(post_name(&bare), "My App - crash");

    let mut long = payload(&[]);
    long.prompt = Some("x".repeat(2500));
    let (text, cut) = compose(&long, true);
    assert!(cut);
    assert_eq!(text.chars().count(), 2000);
    assert!(text.ends_with('…'));

    let mut named = payload(&[]);
    named.title = "y".repeat(300);
    assert_eq!(post_name(&named).chars().count(), 100);
    assert_eq!(post_name(&Payload::default()), "CargoFrame capture");
    assert_eq!(compose(&Payload::default(), true).0, "");
}

#[test]
fn a_test_says_who_and_how_many_servers_and_a_bad_token_asks_for_a_new_one() {
    let server = MockServer::start(|req| {
        if req.header("authorization") != Some("Bot bot-test") {
            return MockResponse::json(401, json!({ "message": "401: Unauthorized", "code": 0 }));
        }
        discord(req)
    });
    let w = world();
    let line = w.with("dc", |ctx| connector(&server).test(ctx)).unwrap();
    assert_eq!(line, "Signed in as cargoframe, in 1 server");
    assert!(server.requests()[0]
        .header("user-agent")
        .unwrap()
        .starts_with("DiscordBot ("));

    let bad = World::new().secret("dc", "token", "old");
    let e = bad
        .with("dc", |ctx| connector(&server).test(ctx))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::AuthExpired);
    assert!(e.message.contains("paste a new one"));

    let lonely = MockServer::start(|req| match req.path().trim_start_matches("/api/v10") {
        "/users/@me" => MockResponse::json(200, json!({ "username": "cargoframe" })),
        _ => MockResponse::json(200, json!([])),
    });
    let line = w.with("dc", |ctx| connector(&lonely).test(ctx)).unwrap();
    assert!(line.contains("not in any server yet"), "{line}");
}

#[test]
fn without_a_token_it_says_what_to_do() {
    let server = MockServer::start(|_| MockResponse::new(500));
    let c = connector(&server);
    let w = World::new();
    assert_eq!(w.with("dc", |ctx| c.status(ctx)), Status::Disconnected);
    let e = w.with("dc", |ctx| c.test(ctx)).unwrap_err();
    assert_eq!(e.kind, FailureKind::ConnectorUnavailable);
    assert!(server.requests().is_empty());
    let w = world();
    assert_eq!(w.with("dc", |ctx| c.status(ctx)), Status::Connected);
    w.with("dc", |ctx| c.disconnect(ctx));
    assert_eq!(w.with("dc", |ctx| c.status(ctx)), Status::Disconnected);
}

#[test]
fn channels_forums_and_threads_are_listed_by_server() {
    let server = MockServer::start(discord);
    let w = world();
    let c = connector(&server);
    let items = w.with("dc", |ctx| c.options(ctx, "channels")).unwrap();
    let shown: Vec<_> = items
        .iter()
        .map(|i| (i.value.as_str(), i.label.as_str()))
        .collect();
    // Voice channels and categories are not places to post. Channels outside any category come
    // first, like in Discord.
    assert_eq!(shown, [(FORUM, "#feedback (forum)"), (TEXT, "#bugs")]);
    assert!(items.iter().all(|i| i.server.as_deref() == Some("Acme")));
    assert_eq!(items[0].category, None);
    assert_eq!(items[1].category.as_deref(), Some("Support"));

    let threads = w.with("dc", |ctx| c.options(ctx, "threads")).unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(
        (threads[0].value.as_str(), threads[0].label.as_str()),
        (THREAD, "Crash on save")
    );
    // A thread is filed under the channel it belongs to.
    assert_eq!(threads[0].server.as_deref(), Some("Acme"));
    assert_eq!(threads[0].category.as_deref(), Some("#bugs"));
    assert!(w.with("dc", |ctx| c.options(ctx, "emoji")).is_err());
}

#[test]
fn one_server_that_refuses_does_not_hide_the_others() {
    let server = MockServer::start(|req| {
        let p = req.path().trim_start_matches("/api/v10").to_string();
        if p == "/users/@me/guilds" {
            return MockResponse::json(
                200,
                json!([{ "id": GUILD2, "name": "Beta" }, { "id": GUILD, "name": "Acme" }]),
            );
        }
        if p == format!("/guilds/{GUILD2}/channels") {
            return MockResponse::json(403, json!({ "code": 50001, "message": "Missing Access" }));
        }
        discord(req)
    });
    let w = world();
    let items = w
        .with("dc", |ctx| connector(&server).options(ctx, "channels"))
        .unwrap();
    assert_eq!(items.len(), 2);

    // A bad token is not skipped: the list fails and says so.
    let denied = MockServer::start(|req| {
        if req.path().ends_with("/guilds") || req.path().contains("/guilds?") {
            MockResponse::json(401, json!({ "message": "401: Unauthorized" }))
        } else {
            MockResponse::new(404)
        }
    });
    let e = w
        .with("dc", |ctx| connector(&denied).options(ctx, "channels"))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::AuthExpired);
}

#[test]
fn a_message_names_nobody_and_links_to_itself() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[("channel", TEXT), ("title", "Crash")]);
    p.prompt = Some("@everyone look".into());
    let out = w
        .with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to Discord");
    assert_eq!(
        out.url.as_deref(),
        Some(format!("https://discord.com/channels/{GUILD}/{TEXT}/{MESSAGE}").as_str())
    );
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(post.path(), format!("/api/v10/channels/{TEXT}/messages"));
    assert_eq!(post.header("authorization"), Some("Bot bot-test"));
    let body = post.json();
    assert_eq!(body["content"], "**Crash**\n@everyone look");
    assert_eq!(body["allowed_mentions"], json!({ "parse": [] }));
    assert_eq!(body["enforce_nonce"], true);
    assert!(body["nonce"].as_str().unwrap().len() <= 25);
}

#[test]
fn the_screenshot_and_the_zip_ride_along_as_attachments() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[("channel", TEXT)]);
    p.prompt = Some("Look".into());
    p.image = Some(png());
    p.files_zip = Some(zip());
    w.with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert!(post
        .header("content-type")
        .unwrap()
        .starts_with("multipart/form-data"));
    let body = payload_json(&post);
    assert_eq!(body["content"], "Look");
    assert_eq!(
        body["attachments"],
        json!([{ "id": 0, "filename": "screenshot.png" }, { "id": 1, "filename": "files.zip" }])
    );
    let text = String::from_utf8_lossy(&post.body).into_owned();
    assert!(text.contains("name=\"files[0]\"; filename=\"screenshot.png\""));
    assert!(text.contains("name=\"files[1]\"; filename=\"files.zip\""));
    assert!(post
        .body
        .windows(png().data.len())
        .any(|w| w == png().data.as_slice()));
}

#[test]
fn a_picture_taken_from_a_recording_rides_along_beside_it() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[("channel", TEXT)]);
    p.image = Some(Attachment {
        name: "recording.mp4".into(),
        mime: "video/mp4".into(),
        data: vec![0, 0, 0, 24],
    });
    p.still = Some(png());
    w.with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(
        payload_json(&post)["attachments"],
        json!([{ "id": 0, "filename": "recording.mp4" }, { "id": 1, "filename": "screenshot.png" }])
    );
}

#[test]
fn a_forum_gets_a_new_post_named_from_the_title() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[("channel", FORUM), ("title", "Crash on save")]);
    p.prompt = Some("Steps to reproduce".into());
    p.image = Some(png());
    let out = w
        .with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Started a post in the Discord forum");
    assert_eq!(
        out.url.as_deref(),
        Some(format!("https://discord.com/channels/{GUILD}/999999999999999999").as_str())
    );
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(post.path(), format!("/api/v10/channels/{FORUM}/threads"));
    let body = payload_json(&post);
    assert_eq!(body["name"], "Crash on save");
    assert_eq!(body["message"]["content"], "Steps to reproduce");
    assert_eq!(body["message"]["allowed_mentions"], json!({ "parse": [] }));
    assert_eq!(
        body["message"]["attachments"],
        json!([{ "id": 0, "filename": "screenshot.png" }])
    );
}

#[test]
fn a_forum_post_whose_outcome_is_unknown_is_not_tried_again() {
    let server = MockServer::start(|req| {
        if req.path().ends_with("/threads") {
            MockResponse::json(502, json!({ "message": "bad gateway" }))
        } else {
            discord(req)
        }
    });
    let w = world();
    let mut p = payload(&[("channel", FORUM)]);
    p.prompt = Some("hi".into());
    let e = w
        .with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(!e.kind.is_retryable());
    assert!(
        e.message.contains("may or may not have created the post"),
        "{}",
        e.message
    );

    // A refusal is certain, so the share is free to be tried again.
    let refused = MockServer::start(|req| {
        if req.path().ends_with("/threads") {
            MockResponse::json(
                403,
                json!({ "code": 50013, "message": "Missing Permissions" }),
            )
        } else {
            discord(req)
        }
    });
    let e = w
        .with("dc", |ctx| connector(&refused).send(ctx, &p))
        .unwrap_err();
    assert!(e.message.contains("missing a permission"), "{}", e.message);
    assert!(!e.message.contains("may or may not"));
}

#[test]
fn a_thread_wins_over_the_channel_and_a_link_will_do() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[
        ("channel", TEXT),
        (
            "thread",
            &format!("https://discord.com/channels/{GUILD}/{THREAD}/{MESSAGE}"),
        ),
    ]);
    p.prompt = Some("hi".into());
    w.with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(post.path(), format!("/api/v10/channels/{THREAD}/messages"));
}

#[test]
fn a_channel_typed_by_name_is_found_and_the_wrong_kinds_are_refused() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[("channel", "#Bugs")]);
    p.prompt = Some("hi".into());
    w.with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert!(server
        .requests()
        .iter()
        .any(|r| r.path() == format!("/api/v10/channels/{TEXT}/messages")));

    let w = world();
    let missing = payload(&[("channel", "#nope")]);
    let e = w
        .with("dc", |ctx| connector(&server).send(ctx, &missing))
        .unwrap_err();
    assert!(
        e.message.contains("no channel called #nope"),
        "{}",
        e.message
    );

    let voice = payload(&[("channel", VOICE)]);
    let e = w
        .with("dc", |ctx| connector(&server).send(ctx, &voice))
        .unwrap_err();
    assert!(e.message.contains("voice"), "{}", e.message);

    let none = payload(&[]);
    let e = w
        .with("dc", |ctx| connector(&server).send(ctx, &none))
        .unwrap_err();
    assert!(e.message.contains("needs a channel"));
    let mut empty = payload(&[("channel", TEXT)]);
    empty.title = String::new();
    let e = w
        .with("dc", |ctx| connector(&server).send(ctx, &empty))
        .unwrap_err();
    assert!(e.message.contains("nothing to send"));
}

#[test]
fn a_retry_reuses_the_nonce_and_a_finished_share_is_not_sent_again() {
    let fail_once = Arc::new(AtomicU32::new(1));
    let f = fail_once.clone();
    let server = MockServer::start(move |req| {
        if req.path().ends_with("/messages") && f.swap(0, Ordering::SeqCst) == 1 {
            return MockResponse::json(502, json!({ "message": "bad gateway" }));
        }
        discord(req)
    });
    let w = world();
    let mut p = payload(&[("channel", TEXT)]);
    p.prompt = Some("hi".into());
    let c = connector(&server);
    let first = w.with("dc", |ctx| c.send(ctx, &p));
    assert_eq!(first.unwrap_err().kind, FailureKind::Server(502));
    w.with("dc", |ctx| c.send(ctx, &p)).unwrap();
    let posts: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|r| r.method == "POST")
        .collect();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].json()["nonce"], posts[1].json()["nonce"]);

    // Sending again after success is a no-op.
    let out = w.with("dc", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(out.summary, "Posted to Discord");
    assert_eq!(
        server
            .requests()
            .iter()
            .filter(|r| r.method == "POST")
            .count(),
        2
    );
}

#[test]
fn a_long_message_is_cut_and_says_so() {
    let server = MockServer::start(discord);
    let w = world();
    let mut p = payload(&[("channel", TEXT)]);
    p.prompt = Some("z".repeat(3000));
    let out = w
        .with("dc", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert!(out.notes[0].contains("2000"));
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(
        post.json()["content"].as_str().unwrap().chars().count(),
        2000
    );
}

#[test]
fn refusals_are_explained_and_classified() {
    let resp = |status: u16, body: Value| Response {
        status,
        headers: vec![],
        body: body.to_string().into_bytes(),
    };
    let cases = [
        (
            resp(401, json!({ "message": "401: Unauthorized" })),
            FailureKind::AuthExpired,
            "paste a new one",
        ),
        (
            resp(403, json!({ "code": 50001, "message": "Missing Access" })),
            FailureKind::Rejected(403),
            "cannot see that channel",
        ),
        (
            resp(
                403,
                json!({ "code": 50013, "message": "Missing Permissions" }),
            ),
            FailureKind::Rejected(403),
            "missing a permission",
        ),
        (
            resp(404, json!({ "code": 10003, "message": "Unknown Channel" })),
            FailureKind::Rejected(404),
            "cannot find that channel",
        ),
        (
            resp(
                413,
                json!({ "code": 40005, "message": "Request entity too large" }),
            ),
            FailureKind::Rejected(413),
            "bigger than this server allows",
        ),
        (
            resp(
                400,
                json!({ "code": 50035, "message": "Invalid Form Body" }),
            ),
            FailureKind::Rejected(400),
            "Invalid Form Body",
        ),
        (
            resp(503, json!({})),
            FailureKind::Server(503),
            "on its side",
        ),
        (
            resp(400, json!({ "message": "odd" })),
            FailureKind::Rejected(400),
            "\"odd\"",
        ),
    ];
    for (r, kind, text) in cases {
        let e = discord_error(&r, "post the message");
        assert_eq!(e.kind, kind, "{}", r.status);
        assert!(
            e.message.starts_with("Could not post the message: "),
            "{}",
            e.message
        );
        assert!(e.message.contains(text), "{}", e.message);
    }
    // The wait comes from the header, or from the body's fractional seconds.
    let e = discord_error(
        &resp(429, json!({ "retry_after": 1.5, "global": false })),
        "x",
    );
    assert_eq!(e.kind, FailureKind::Throttled(429));
    assert_eq!(e.retry_after, Some(Duration::from_millis(1500)));
    let mut with_header = resp(429, json!({ "retry_after": 1.5 }));
    with_header.headers.push(("Retry-After".into(), "3".into()));
    assert_eq!(
        discord_error(&with_header, "x").retry_after,
        Some(Duration::from_secs(3))
    );
}

#[test]
fn it_describes_what_a_button_can_fill() {
    let d = DiscordConnector::new(model::Discord::default()).describe();
    let keys: Vec<_> = d.fields.iter().map(|f| f.key).collect();
    assert_eq!(keys, ["channel", "thread", "message", "title"]);
    assert_eq!(d.fields[0].list, Some("channels"));
    assert_eq!(d.fields[1].list, Some("threads"));
    assert_eq!(d.lists.len(), 2);
}

// ----- webhooks: no bot -----

fn hook_world() -> World {
    World::new()
        .secret(
            "dc",
            "hook0",
            &format!("https://discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
        )
        .secret(
            "dc",
            "hook1",
            &format!("https://discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}?wait=true"),
        )
}

fn hooks(shown_as: &str) -> DiscordConnector {
    DiscordConnector::new(model::Discord {
        mode: DiscordMode::Webhook,
        shown_as: shown_as.into(),
        hooks: vec![
            model::Hook {
                slot: 0,
                name: "bugs".into(),
                server: "Acme".into(),
            },
            model::Hook {
                slot: 1,
                name: "chat".into(),
                server: String::new(),
            },
            model::Hook {
                slot: 2,
                name: "later".into(),
                server: String::new(),
            },
        ],
    })
}

fn hooked(server: &MockServer, shown_as: &str) -> DiscordConnector {
    hooks(shown_as).with_endpoints(Endpoints {
        api: server.url("/api/v10"),
    })
}

fn hook_path() -> String {
    format!("/api/v10/webhooks/{HOOK}/{HOOK_TOKEN}")
}

/// A webhook that posts in a text channel.
fn webhook_text(req: &Recorded) -> MockResponse {
    if req.path() == hook_path() && req.method == "GET" {
        return MockResponse::json(
            200,
            json!({ "channel_id": TEXT, "guild_id": GUILD, "name": "Hook" }),
        );
    }
    if req.path() == hook_path() && req.method == "POST" {
        return MockResponse::json(200, json!({ "id": MESSAGE, "channel_id": TEXT }));
    }
    MockResponse::new(404)
}

#[test]
fn only_discords_own_webhook_links_are_taken() {
    let ok = format!("https://discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}");
    assert_eq!(
        parse_webhook(&ok).unwrap(),
        (HOOK.to_string(), HOOK_TOKEN.to_string())
    );
    assert!(parse_webhook(&format!("  {ok}?wait=true ")).is_ok());
    assert!(parse_webhook(&format!(
        "https://ptb.discord.com/api/v10/webhooks/{HOOK}/{HOOK_TOKEN}"
    ))
    .is_ok());
    for bad in [
        "",
        "not a link",
        &format!("http://discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
        &format!("https://evil.example/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
        &format!("https://discord.com.evil.example/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
        &format!("https://evil.example#.discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
        &format!("https://discord.com/api/webhooks/12/{HOOK_TOKEN}"),
        &format!("https://discord.com/api/webhooks/{HOOK}/"),
        &format!("https://discord.com/channels/{GUILD}/{TEXT}"),
    ] {
        let e = parse_webhook(bad).unwrap_err();
        assert!(e.contains("Copy Webhook URL"), "{bad}: {e}");
    }
}

#[test]
fn a_webhook_posts_without_a_bot_and_under_any_name() {
    let server = MockServer::start(webhook_text);
    let w = hook_world();
    let mut p = payload(&[("channel", "hook0"), ("title", "Crash")]);
    p.prompt = Some("@everyone look".into());
    let out = w
        .with("dc", |ctx| hooked(&server, "Louis").send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to Discord");
    assert_eq!(
        out.url.as_deref(),
        Some(format!("https://discord.com/channels/{GUILD}/{TEXT}/{MESSAGE}").as_str())
    );
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(
        post.header("authorization"),
        None,
        "a webhook needs no bot token"
    );
    assert_eq!(post.query_param("wait").as_deref(), Some("true"));
    assert_eq!(post.query_param("thread_id"), None);
    let body = post.json();
    assert_eq!(body["content"], "**Crash**\n@everyone look");
    assert_eq!(body["allowed_mentions"], json!({ "parse": [] }));
    assert_eq!(body["username"], "Louis");
    assert!(body.get("thread_name").is_none());
}

#[test]
fn a_webhook_with_one_channel_needs_no_choice_and_a_thread_link_is_used() {
    let server = MockServer::start(webhook_text);
    let w = World::new().secret(
        "dc",
        "hook0",
        &format!("https://discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
    );
    let only = DiscordConnector::new(model::Discord {
        mode: DiscordMode::Webhook,
        shown_as: String::new(),
        hooks: vec![model::Hook {
            slot: 0,
            name: "bugs".into(),
            server: String::new(),
        }],
    })
    .with_endpoints(Endpoints {
        api: server.url("/api/v10"),
    });
    let mut p = payload(&[(
        "thread",
        &format!("https://discord.com/channels/{GUILD}/{THREAD}/{MESSAGE}"),
    )]);
    p.prompt = Some("hi".into());
    w.with("dc", |ctx| only.send(ctx, &p)).unwrap();
    let post = server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(post.query_param("thread_id").as_deref(), Some(THREAD));
    assert!(
        post.json().get("username").is_none(),
        "no name given keeps the webhook's own"
    );
}

#[test]
fn a_webhook_into_a_forum_starts_a_post_named_from_the_title() {
    let server = MockServer::start(|req| {
        if req.path() == hook_path() && req.method == "GET" {
            return MockResponse::json(200, json!({ "channel_id": FORUM, "guild_id": GUILD }));
        }
        if req.path() == hook_path() && req.method == "POST" {
            // Discord refuses a plain message in a forum, and takes one with a post name.
            return if req.json().get("thread_name").is_some() {
                MockResponse::json(200, json!({ "id": MESSAGE, "channel_id": THREAD }))
            } else {
                MockResponse::json(
                    400,
                    json!({ "code": 220003, "message": "Webhooks posted to forum channels must have a thread_name or thread_id" }),
                )
            };
        }
        MockResponse::new(404)
    });
    let w = hook_world();
    let mut p = payload(&[("channel", "hook0"), ("title", "Crash on save")]);
    p.prompt = Some("details".into());
    let out = w
        .with("dc", |ctx| hooked(&server, "").send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Started a post in the Discord forum");
    assert_eq!(
        out.url.as_deref(),
        Some(format!("https://discord.com/channels/{GUILD}/{THREAD}").as_str())
    );
    let posts: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|r| r.method == "POST")
        .collect();
    assert_eq!(posts.len(), 2);
    let started = posts[1].json();
    assert_eq!(started["thread_name"], "Crash on save");
    assert_eq!(
        started["content"], "details",
        "the title is the post's name, not a bold line"
    );
}

#[test]
fn a_webhook_post_whose_outcome_is_unknown_is_not_tried_again() {
    let server = MockServer::start(|req| {
        if req.method == "POST" {
            MockResponse::json(502, json!({ "message": "bad gateway" }))
        } else {
            webhook_text(req)
        }
    });
    let w = hook_world();
    let mut p = payload(&[("channel", "hook0")]);
    p.prompt = Some("hi".into());
    let e = w
        .with("dc", |ctx| hooked(&server, "").send(ctx, &p))
        .unwrap_err();
    assert!(!e.kind.is_retryable());
    assert!(
        e.message.contains("may or may not have posted"),
        "{}",
        e.message
    );
    assert_eq!(
        server
            .requests()
            .iter()
            .filter(|r| r.method == "POST")
            .count(),
        1
    );
}

#[test]
fn a_deleted_webhook_says_to_make_a_new_one() {
    let server = MockServer::start(|_| {
        MockResponse::json(404, json!({ "code": 10015, "message": "Unknown Webhook" }))
    });
    let w = hook_world();
    let mut p = payload(&[("channel", "hook0")]);
    p.prompt = Some("hi".into());
    let e = w
        .with("dc", |ctx| hooked(&server, "").send(ctx, &p))
        .unwrap_err();
    assert_eq!(e.kind, FailureKind::AuthExpired);
    assert!(e.message.contains("make a new one"), "{}", e.message);
}

#[test]
fn webhook_channels_are_listed_by_name_and_only_when_a_link_is_stored() {
    let server = MockServer::start(webhook_text);
    let w = hook_world();
    let c = hooked(&server, "");
    let items = w.with("dc", |ctx| c.options(ctx, "channels")).unwrap();
    // Slot 2 has no link yet, so it is not offered.
    let shown: Vec<_> = items
        .iter()
        .map(|i| (i.value.as_str(), i.label.as_str(), i.server.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [("hook0", "bugs", Some("Acme")), ("hook1", "chat", None)]
    );
    // A webhook cannot look at the server, so there is no thread list to show.
    assert!(w
        .with("dc", |ctx| c.options(ctx, "threads"))
        .unwrap()
        .is_empty());
    assert!(
        server.requests().is_empty(),
        "listing channels asks Discord nothing"
    );
}

#[test]
fn testing_webhooks_checks_each_link() {
    let server = MockServer::start(webhook_text);
    let w = hook_world();
    let said = w.with("dc", |ctx| hooked(&server, "").test(ctx)).unwrap();
    assert!(said.contains("Works: bugs, chat"), "{said}");
    assert!(
        said.contains("later") && said.contains("no webhook link yet"),
        "{said}"
    );

    let none = World::new();
    let e = none
        .with("dc", |ctx| hooked(&server, "").test(ctx))
        .unwrap_err();
    assert!(e.message.contains("no webhook link yet"), "{}", e.message);
}

#[test]
fn a_webhook_connection_is_connected_by_a_link_and_forgets_all_of_them() {
    let c = hooks("");
    let w = hook_world();
    assert_eq!(w.with("dc", |ctx| c.status(ctx)), Status::Connected);
    assert_eq!(
        World::new().with("dc", |ctx| c.status(ctx)),
        Status::Disconnected
    );
    // A bot token alone does not make a webhook connection connected.
    assert_eq!(
        world().with("dc", |ctx| c.status(ctx)),
        Status::Disconnected
    );
    w.with("dc", |ctx| c.disconnect(ctx));
    assert_eq!(w.with("dc", |ctx| c.status(ctx)), Status::Disconnected);
}

#[test]
fn connections_saved_before_webhooks_stay_bots() {
    let old: model::Discord = serde_json::from_value(json!({})).unwrap();
    assert_eq!(old.mode, DiscordMode::Bot);
    assert!(old.hooks.is_empty());
}

// ----- going deeper: the threads of a channel, and finding the channel of a thread -----

const OLD_THREAD: &str = "666666666666666000";

/// Like `discord`, with older posts too: the forum has one active post and two older ones.
fn discord_with_archive(req: &Recorded) -> MockResponse {
    let p = req.path().trim_start_matches("/api/v10");
    if p == format!("/guilds/{GUILD}/threads/active") {
        return MockResponse::json(
            200,
            json!({ "threads": [
                { "id": THREAD, "name": "Crash on save", "parent_id": FORUM },
                { "id": "666666666666666001", "name": "Not mine", "parent_id": TEXT },
            ] }),
        );
    }
    if p == format!("/channels/{FORUM}/threads/archived/public") {
        return MockResponse::json(
            200,
            json!({ "threads": [
                { "id": OLD_THREAD, "name": "Crash on save", "parent_id": FORUM },
                { "id": THREAD, "name": "Crash on save", "parent_id": FORUM },
                { "id": "666666666666666500", "name": "Slow start", "parent_id": FORUM },
            ] }),
        );
    }
    discord(req)
}

#[test]
fn a_channel_says_it_has_threads_inside_and_which_field_they_fill() {
    let c = connector(&MockServer::start(discord));
    let d = c.describe();
    let channels = d.lists.iter().find(|l| l.id == "channels").unwrap();
    let inside = channels.inside.as_ref().unwrap();
    assert_eq!((inside.list, inside.field), ("threads", "thread"));
    assert!(d.fields.iter().any(|f| f.key == inside.field));
    // the same for webhooks, where the window offers the posts it made itself
    let hooked = hooks("");
    let d = hooked.describe();
    assert!(d
        .lists
        .iter()
        .any(|l| l.id == "channels" && l.inside.is_some()));
}

#[test]
fn the_threads_of_one_channel_are_its_active_ones_then_older_ones_newest_first() {
    let server = MockServer::start(discord_with_archive);
    let c = connector(&server);
    let w = world();
    let items = w
        .with("dc", |ctx| c.options(ctx, &format!("threads:{FORUM}")))
        .unwrap();
    let shown: Vec<_> = items
        .iter()
        .map(|i| (i.value.as_str(), i.group.as_deref().unwrap_or("")))
        .collect();
    // another channel's thread is not offered; a thread that is in both lists is offered once;
    // each group has the newest first
    assert_eq!(
        shown,
        [
            (THREAD, "Active"),
            ("666666666666666500", "Older"),
            (OLD_THREAD, "Older"),
        ]
    );
    // a link to the channel does as well as its id
    let again = w
        .with("dc", |ctx| {
            c.options(
                ctx,
                &format!("threads:https://discord.com/channels/{GUILD}/{FORUM}"),
            )
        })
        .unwrap();
    assert_eq!(again.len(), 3);
    // a text channel's threads: the older ones are not there (the mock has none), no error
    let text = w
        .with("dc", |ctx| c.options(ctx, &format!("threads:{TEXT}")))
        .unwrap();
    assert!(text.iter().all(|i| i.value != THREAD));
}

#[test]
fn a_very_long_forum_is_cut_to_what_the_dropdown_can_search() {
    let server = MockServer::start(|req| {
        let p = req.path().trim_start_matches("/api/v10").to_string();
        if p == format!("/channels/{FORUM}/threads/archived/public") {
            let threads: Vec<Value> = (0..300)
                .map(|n| json!({ "id": format!("{}", 700_000_000_000_000_000u64 + n), "name": format!("Post {n}") }))
                .collect();
            return MockResponse::json(200, json!({ "threads": threads }));
        }
        discord(req)
    });
    let c = connector(&server);
    let items = world()
        .with("dc", |ctx| c.options(ctx, &format!("threads:{FORUM}")))
        .unwrap();
    assert_eq!(items.len(), MAX_THREADS);
    // the newest are the ones kept
    assert_eq!(items[0].label, "Post 299");
}

#[test]
fn a_thread_list_for_a_channel_that_is_not_one_is_refused_in_words() {
    let c = connector(&MockServer::start(discord));
    let e = world()
        .with("dc", |ctx| c.options(ctx, "threads:not-a-channel"))
        .unwrap_err();
    assert!(e.message.contains("not a Discord channel"), "{}", e.message);
}

/// Two webhooks, in two channels; a thread of the second channel is refused by the first.
fn two_channels_world() -> (World, DiscordConnector) {
    let w = World::new()
        .secret(
            "dc",
            "hook0",
            &format!("https://discord.com/api/webhooks/{HOOK}/{HOOK_TOKEN}"),
        )
        .secret(
            "dc",
            "hook1",
            "https://discord.com/api/webhooks/999000999000999111/other-Token9",
        );
    let c = DiscordConnector::new(model::Discord {
        mode: DiscordMode::Webhook,
        shown_as: String::new(),
        hooks: vec![
            model::Hook {
                slot: 0,
                name: "bugs".into(),
                server: String::new(),
            },
            model::Hook {
                slot: 1,
                name: "chat".into(),
                server: String::new(),
            },
        ],
    });
    (w, c)
}

/// Channel A (hook 0) does not know the thread; channel B (hook 1) does.
fn channel_b_has_the_thread(req: &Recorded) -> MockResponse {
    let first = req.path().contains(HOOK);
    match (req.method.as_str(), first) {
        ("GET", true) => MockResponse::json(200, json!({ "channel_id": TEXT, "guild_id": GUILD })),
        ("GET", false) => {
            MockResponse::json(200, json!({ "channel_id": FORUM, "guild_id": GUILD }))
        }
        ("POST", true) => {
            MockResponse::json(404, json!({ "code": 10003, "message": "Unknown Channel" }))
        }
        ("POST", false) => MockResponse::json(200, json!({ "id": MESSAGE, "channel_id": THREAD })),
        _ => MockResponse::new(404),
    }
}

#[test]
fn a_thread_alone_finds_the_webhook_of_its_own_channel() {
    let server = MockServer::start(channel_b_has_the_thread);
    let (w, c) = two_channels_world();
    let c = c.with_endpoints(Endpoints {
        api: server.url("/api/v10"),
    });
    // no channel is picked: the thread is tried in each channel until one knows it
    let mut p = payload(&[("thread", THREAD)]);
    p.prompt = Some("again".into());
    let out = w.with("dc", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(out.summary, "Posted to Discord");
    let posts: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|r| r.method == "POST")
        .collect();
    assert_eq!(
        posts.len(),
        2,
        "the refusal posted nothing, so the next channel was tried"
    );
    assert_eq!(posts[1].query_param("thread_id").as_deref(), Some(THREAD));
    // what was posted in is handed back, to be offered next time
    let place = out.place.unwrap();
    assert_eq!(place.value, THREAD);
    assert!(place.label.starts_with("Thread …"), "{}", place.label);
}

#[test]
fn a_thread_in_none_of_the_channels_is_explained_and_posted_nowhere() {
    let server = MockServer::start(|req| {
        if req.method == "POST" {
            MockResponse::json(404, json!({ "code": 10003, "message": "Unknown Channel" }))
        } else {
            MockResponse::json(200, json!({ "channel_id": TEXT, "guild_id": GUILD }))
        }
    });
    let (w, c) = two_channels_world();
    let c = c.with_endpoints(Endpoints {
        api: server.url("/api/v10"),
    });
    let mut p = payload(&[("channel", "hook0"), ("thread", THREAD)]);
    p.prompt = Some("hi".into());
    let e = w.with("dc", |ctx| c.send(ctx, &p)).unwrap_err();
    assert!(
        e.message
            .contains("not in any channel you added a webhook for"),
        "{}",
        e.message
    );
    assert!(!e.kind.is_retryable());
    // one webhook only: the sentence is about that channel
    let one = DiscordConnector::new(model::Discord {
        mode: DiscordMode::Webhook,
        shown_as: String::new(),
        hooks: vec![model::Hook {
            slot: 0,
            name: "bugs".into(),
            server: String::new(),
        }],
    })
    .with_endpoints(Endpoints {
        api: server.url("/api/v10"),
    });
    let e = w.with("dc", |ctx| one.send(ctx, &p)).unwrap_err();
    assert!(
        e.message.contains("not in the channel of this webhook"),
        "{}",
        e.message
    );
}

#[test]
fn a_post_started_in_a_forum_is_handed_back_by_its_name_for_next_time() {
    let server = MockServer::start(|req| {
        if req.path() == hook_path() && req.method == "GET" {
            return MockResponse::json(200, json!({ "channel_id": FORUM, "guild_id": GUILD }));
        }
        if req.path() == hook_path() && req.method == "POST" {
            return if req.json().get("thread_name").is_some() {
                MockResponse::json(200, json!({ "id": MESSAGE, "channel_id": THREAD }))
            } else {
                MockResponse::json(400, json!({ "code": 220003, "message": "needs a name" }))
            };
        }
        MockResponse::new(404)
    });
    let w = hook_world();
    let mut p = payload(&[("channel", "hook0"), ("title", "[PROJ-1] Crash on save")]);
    p.prompt = Some("details".into());
    let out = w
        .with("dc", |ctx| hooked(&server, "").send(ctx, &p))
        .unwrap();
    let place = out.place.unwrap();
    assert_eq!(place.value, THREAD);
    assert_eq!(place.label, "[PROJ-1] Crash on save");
}

#[test]
fn several_channels_and_nothing_picked_asks_for_the_channel() {
    let server = MockServer::start(webhook_text);
    let w = hook_world();
    let mut p = payload(&[]);
    p.prompt = Some("hi".into());
    let e = w
        .with("dc", |ctx| hooked(&server, "").send(ctx, &p))
        .unwrap_err();
    assert!(
        e.message.contains("Pick the channel to post in"),
        "{}",
        e.message
    );
    assert!(server.requests().is_empty());
}
