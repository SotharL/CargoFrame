use super::*;
use crate::oauth::Tokens;
use crate::payload::{Attachment, Field};
use crate::secrets::SecretStore;
use crate::testkit::{ApproveBrowser, World};
use cargoframe_net::mock::{MockResponse, MockServer};
use serde_json::json;
use std::time::Duration;

const APP: &str = "3f2a1b4c-5d6e-4f70-8a9b-0c1d2e3f4a5b";
const TEAM: &str = "11111111-2222-3333-4444-555555555555";
const CHANNEL: &str = "19:abc123@thread.tacv2";

fn connector(server: &MockServer) -> TeamsConnector {
    TeamsConnector::new(model::Teams {
        client_id: APP.into(),
        tenant: "contoso.onmicrosoft.com".into(),
    })
    .with_endpoints(
        Endpoints {
            login: server.url("/login"),
            graph: server.url("/graph"),
        },
        Duration::from_secs(5),
    )
}

/// A person who has already signed in.
fn signed_in() -> World {
    let w = World::new();
    w.secrets
        .set(
            "teams/oauth",
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

fn image() -> Attachment {
    Attachment {
        name: "screenshot.png".into(),
        mime: "image/png".into(),
        data: vec![0x89, b'P', b'N', b'G', 7, 7],
    }
}

fn payload(fields: &[(&str, &str)]) -> Payload {
    Payload {
        title: "My App - crash".into(),
        fields: fields.iter().map(|(k, v)| Field::text(k, v)).collect(),
        ..Payload::default()
    }
}

fn created() -> MockResponse {
    MockResponse::json(
        201,
        json!({"id": "1700000000000", "webUrl": "https://teams.microsoft.com/l/message/x/1700000000000"}),
    )
}

#[test]
fn targets_are_read_from_the_list_values_and_bare_chat_ids() {
    assert_eq!(
        parse_target("chat:19:abc@thread.v2").unwrap(),
        Target::Chat("19:abc@thread.v2".into())
    );
    assert_eq!(
        parse_target("19:one_two@unq.gbl.spaces").unwrap(),
        Target::Chat("19:one_two@unq.gbl.spaces".into())
    );
    assert_eq!(
        parse_target(&format!("channel:{TEAM}/{CHANNEL}")).unwrap(),
        Target::Channel {
            team: TEAM.into(),
            channel: CHANNEL.into()
        }
    );
    for bad in [
        "",
        "general",
        "chat:",
        "chat:abc",
        "channel:nope/19:x@thread.tacv2",
        &format!("channel:{TEAM}/general"),
        &format!("channel:{TEAM}"),
        // a bare channel id is not enough: it needs its team
        CHANNEL,
    ] {
        assert!(parse_target(bad).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn a_copied_channel_message_link_names_the_team_channel_and_thread() {
    let link = format!(
        "https://teams.microsoft.com/l/message/19%3Aabc123%40thread.tacv2/1700000000123?tenantId=t&groupId={TEAM}&parentMessageId=1700000000123&teamName=Eng&channelName=General&createdTime=1700000000123"
    );
    assert_eq!(
        parse_thread(&link).unwrap(),
        ThreadRef {
            team: TEAM.into(),
            channel: CHANNEL.into(),
            message: "1700000000123".into()
        }
    );
    // a reply's link points at the reply but names its thread
    let reply = format!(
        "https://teams.microsoft.com/l/message/19%3Aabc123%40thread.tacv2/1700000009999?groupId={TEAM}&parentMessageId=1700000000123"
    );
    assert_eq!(parse_thread(&reply).unwrap().message, "1700000000123");
    // a root message's link may carry no parent
    let root = format!(
        "https://teams.cloud.microsoft/l/message/19%3Aabc123%40thread.tacv2/1700000000123?groupId={TEAM}"
    );
    assert_eq!(parse_thread(&root).unwrap().message, "1700000000123");

    for bad in [
        "".to_string(),
        "1700000000123".to_string(),
        "http://teams.microsoft.com/l/message/x/1".to_string(),
        format!("https://evil.example/l/message/19%3Aabc123%40thread.tacv2/1?groupId={TEAM}"),
        format!("https://teams.microsoft.com/l/chat/19%3Aabc123%40thread.v2/1?groupId={TEAM}"),
        // a chat message (no channel id), a missing team, a message that is not a number
        format!("https://teams.microsoft.com/l/message/19%3Aabc%40unq.gbl.spaces/1?groupId={TEAM}"),
        "https://teams.microsoft.com/l/message/19%3Aabc123%40thread.tacv2/1700000000123"
            .to_string(),
        format!(
            "https://teams.microsoft.com/l/message/19%3Aabc123%40thread.tacv2/abc?groupId={TEAM}"
        ),
    ] {
        let err = parse_thread(&bad).unwrap_err();
        assert!(err.contains("Copy link"), "{bad:?}: {err}");
    }
}

#[test]
fn text_is_escaped_so_it_can_never_become_markup_or_a_mention() {
    let p = payload(&[
        (
            "message",
            "<at id=\"0\">Everyone</at> & <script>x</script>\nsecond 'line'",
        ),
        ("title", "  <b>big</b>  \n title "),
    ]);
    let c = compose(&p, true, false);
    assert!(!c.html.contains("<at"), "{}", c.html);
    assert!(!c.html.contains("<script"), "{}", c.html);
    assert!(c
        .html
        .starts_with("<p><b>&lt;b&gt;big&lt;/b&gt; title</b></p>"));
    assert!(c.html.contains("&amp; &lt;script&gt;"));
    assert!(c.html.contains("<br>second &#39;line&#39;"));
    assert!(c
        .html
        .ends_with("<p><img src=\"../hostedContents/1/$value\" alt=\"Screenshot\"></p>"));
    assert_eq!(c.subject, None);
}

#[test]
fn the_message_falls_back_to_the_prompt_then_the_window_title() {
    let mut p = payload(&[]);
    assert_eq!(compose(&p, false, true).html, "<p>My App - crash</p>");
    p.prompt = Some("  from the prompt  ".into());
    assert_eq!(compose(&p, false, true).html, "<p>from the prompt</p>");
    // a title typed by the person replaces the window title as the fallback text
    let p = payload(&[("title", "Headline")]);
    let c = compose(&p, false, true);
    assert_eq!(c.html, "");
    assert_eq!(c.subject.as_deref(), Some("Headline"));
}

#[test]
fn a_long_message_is_shortened_and_the_person_is_told() {
    let long = "x".repeat(MAX_TEXT_CHARS + 50);
    let c = compose(&payload(&[("message", &long)]), false, false);
    assert!(c.shortened);
    assert!(c.html.chars().count() < MAX_TEXT_CHARS + 20);
    assert!(c.html.contains('…'));
}

#[test]
fn a_chat_post_carries_the_text_and_the_screenshot_inside_the_message() {
    let server = MockServer::start(|req| match req.path() {
        "/graph/chats/19%3Aabc%40thread.v2/messages" => created(),
        _ => MockResponse::new(404),
    });
    let w = signed_in();
    let mut p = payload(&[
        ("target", "chat:19:abc@thread.v2"),
        ("message", "It crashed"),
    ]);
    p.image = Some(image());
    let out = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to Teams");
    assert_eq!(
        out.url.as_deref(),
        Some("https://teams.microsoft.com/l/message/x/1700000000000")
    );
    assert!(out.notes.is_empty());

    let req = &server.requests()[0];
    assert_eq!(req.header("authorization"), Some("Bearer AT"));
    let body = req.json();
    assert_eq!(body["body"]["contentType"], "html");
    let html = body["body"]["content"].as_str().unwrap();
    assert!(html.contains("<p>It crashed</p>"));
    assert!(html.contains("../hostedContents/1/$value"));
    assert_eq!(
        body["hostedContents"][0]["@microsoft.graph.temporaryId"],
        "1"
    );
    assert_eq!(body["hostedContents"][0]["contentType"], "image/png");
    assert_eq!(
        STANDARD
            .decode(body["hostedContents"][0]["contentBytes"].as_str().unwrap())
            .unwrap(),
        image().data
    );
    // a chat message has no headline
    assert!(body.get("subject").is_none());
}

#[test]
fn a_new_channel_post_gets_the_title_as_its_headline() {
    let server = MockServer::start(|req| {
        if req.path().ends_with("/messages") {
            created()
        } else {
            MockResponse::new(404)
        }
    });
    let w = signed_in();
    let p = payload(&[
        ("target", &format!("channel:{TEAM}/{CHANNEL}")),
        ("title", "Crash on save"),
        ("message", "details"),
    ]);
    w.with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let req = &server.requests()[0];
    assert_eq!(
        req.path(),
        format!("/graph/teams/{TEAM}/channels/19%3Aabc123%40thread.tacv2/messages")
    );
    let body = req.json();
    assert_eq!(body["subject"], "Crash on save");
    assert_eq!(body["body"]["content"], "<p>details</p>");
    assert!(body.get("hostedContents").is_none());
}

#[test]
fn a_thread_reply_goes_under_the_first_message_with_a_bold_title() {
    let server = MockServer::start(|req| {
        if req.path().ends_with("/replies") {
            created()
        } else {
            MockResponse::new(404)
        }
    });
    let w = signed_in();
    let link = format!(
        "https://teams.microsoft.com/l/message/19%3Aabc123%40thread.tacv2/1700000009999?groupId={TEAM}&parentMessageId=1700000000123"
    );
    // the link alone is enough
    let p = payload(&[
        ("thread", &link),
        ("title", "Update"),
        ("message", "still broken"),
    ]);
    let out = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted in the Teams thread");
    let req = &server.requests()[0];
    assert_eq!(
        req.path(),
        format!("/graph/teams/{TEAM}/channels/19%3Aabc123%40thread.tacv2/messages/1700000000123/replies")
    );
    let body = req.json();
    assert!(body.get("subject").is_none());
    assert_eq!(
        body["body"]["content"],
        "<p><b>Update</b></p><p>still broken</p>"
    );

    // the same channel chosen in the list is fine too
    let p = payload(&[
        ("thread", &link),
        ("target", &format!("channel:{TEAM}/{CHANNEL}")),
    ]);
    w.with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn contradicting_choices_are_refused_before_anything_is_sent() {
    let server = MockServer::start(|_| created());
    let w = signed_in();
    let link = format!(
        "https://teams.microsoft.com/l/message/19%3Aabc123%40thread.tacv2/1700000000123?groupId={TEAM}"
    );
    let other = format!("channel:{TEAM}/19:other@thread.tacv2");
    for (fields, want) in [
        (
            vec![("thread", link.as_str()), ("target", other.as_str())],
            "different channel",
        ),
        (
            vec![
                ("thread", link.as_str()),
                ("target", "chat:19:abc@thread.v2"),
            ],
            "no threads",
        ),
        (vec![("message", "hi")], "needs a chat or a channel"),
        (vec![("target", "general")], "not a Teams chat or channel"),
        (vec![("target", "chat:19:abc@thread.v2")], "nothing to send"),
    ] {
        let mut p = payload(&fields);
        // no window title either, so the last case has nothing to say
        p.title.clear();
        let err = w
            .with("teams", |ctx| connector(&server).send(ctx, &p))
            .unwrap_err();
        assert!(err.message.contains(want), "{fields:?}: {}", err.message);
    }
    assert!(server.requests().is_empty());
}

#[test]
fn ticked_files_are_left_out_and_the_person_is_told() {
    let server = MockServer::start(|_| created());
    let w = signed_in();
    let mut p = payload(&[("target", "chat:19:abc@thread.v2")]);
    p.files_zip = Some(Attachment {
        name: "files.zip".into(),
        mime: "application/zip".into(),
        data: vec![1, 2, 3],
    });
    let out = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert!(out.notes[0].contains("OneDrive"), "{:?}", out.notes);
    // the zip is nowhere in the message
    assert!(!server.requests()[0].text().contains("files.zip"));
}

#[test]
fn refusals_say_what_to_do_and_keep_their_class() {
    let w = signed_in();
    let p = payload(&[("target", "chat:19:abc@thread.v2"), ("message", "hi")]);
    for (status, want, kind) in [
        (401, "press Sign in", FailureKind::AuthExpired),
        (403, "not a member", FailureKind::AuthExpired),
        (
            404,
            "cannot find that chat or channel",
            FailureKind::Rejected(404),
        ),
        (413, "too large", FailureKind::Rejected(413)),
    ] {
        let server = MockServer::start(move |_| {
            MockResponse::json(
                status,
                json!({"error": {"code": "Nope", "message": "Graph says no"}}),
            )
        });
        let err = w
            .with("teams", |ctx| connector(&server).send(ctx, &p))
            .unwrap_err();
        assert_eq!(err.kind, kind, "{status}");
        assert!(err.message.contains(want), "{status}: {}", err.message);
    }
    // being throttled is safe to try again, and the wait Microsoft asks for is kept
    let server = MockServer::start(|_| {
        MockResponse::json(429, json!({"error": {"message": "slow down"}}))
            .header("Retry-After", "7")
    });
    let err = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(matches!(err.kind, FailureKind::Throttled(429)));
    assert!(err.kind.is_retryable());
    assert_eq!(err.retry_after, Some(Duration::from_secs(7)));
}

#[test]
fn a_post_whose_outcome_is_unknown_is_never_tried_again() {
    let w = signed_in();
    let p = payload(&[("target", "chat:19:abc@thread.v2"), ("message", "hi")]);

    let server = MockServer::start(|_| MockResponse::new(503));
    let err = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(!err.kind.is_retryable(), "{:?}", err.kind);
    assert!(err.message.contains("look in Teams"), "{}", err.message);

    // the answer never arrives
    let server = MockServer::start(|_| created().delay(Duration::from_millis(900)));
    let w = World::new().timeout(Duration::from_millis(200)).secret(
        "teams",
        "oauth",
        &serde_json::to_string(&Tokens {
            access_token: "AT".into(),
            refresh_token: Some("RT".into()),
            expires_at: 9_999_999,
        })
        .unwrap(),
    );
    let err = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(!err.kind.is_retryable(), "{:?}", err.kind);
    assert!(err.message.contains("look in Teams"), "{}", err.message);
}

#[test]
fn not_being_signed_in_is_said_plainly() {
    let server = MockServer::start(|_| created());
    let w = World::new();
    let p = payload(&[("target", "chat:19:abc@thread.v2"), ("message", "hi")]);
    let err = w
        .with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(err.message.contains("Sign in"), "{}", err.message);
    assert!(server.requests().is_empty());
    assert_eq!(
        w.with("teams", |ctx| connector(&server).status(ctx)),
        Status::Disconnected
    );
}

#[test]
fn an_old_token_is_renewed_before_the_post() {
    let server = MockServer::start(|req| match req.path() {
        "/login/contoso.onmicrosoft.com/oauth2/v2.0/token" => MockResponse::json(
            200,
            json!({"access_token": "AT-NEW", "refresh_token": "RT-NEW", "expires_in": 3600}),
        ),
        p if p.ends_with("/messages") => created(),
        _ => MockResponse::new(404),
    });
    let w = World::new().secret(
        "teams",
        "oauth",
        &serde_json::to_string(&Tokens {
            access_token: "AT-OLD".into(),
            refresh_token: Some("RT".into()),
            expires_at: 10,
        })
        .unwrap(),
    );
    let p = payload(&[("target", "chat:19:abc@thread.v2"), ("message", "hi")]);
    w.with("teams", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let renew = &server.requests_to("/login/contoso.onmicrosoft.com/oauth2/v2.0/token")[0];
    assert_eq!(
        renew.form_value("grant_type").as_deref(),
        Some("refresh_token")
    );
    assert_eq!(renew.form_value("refresh_token").as_deref(), Some("RT"));
    assert_eq!(renew.form_value("client_id").as_deref(), Some(APP));
    assert_eq!(renew.form_value("client_secret"), None);
    let post = server.requests_to("/graph/chats/19%3Aabc%40thread.v2/messages");
    assert_eq!(post[0].header("authorization"), Some("Bearer AT-NEW"));
}

#[test]
fn connecting_signs_in_through_the_browser_as_a_public_client_and_names_the_account() {
    let server = MockServer::start(|req| match req.path() {
        "/login/contoso.onmicrosoft.com/oauth2/v2.0/token" => MockResponse::json(
            200,
            json!({"access_token": "AT9", "refresh_token": "RT9", "expires_in": 3600}),
        ),
        "/graph/me" => MockResponse::json(
            200,
            json!({"id": "u1", "displayName": "Jane Doe", "userPrincipalName": "jane@contoso.com"}),
        ),
        _ => MockResponse::new(404),
    });
    let w = World::new();
    let c = connector(&server);
    assert_eq!(w.with("teams", |ctx| c.status(ctx)), Status::Disconnected);
    let browser = ApproveBrowser::new();
    let msg = w
        .with_browser("teams", &browser, |ctx| c.connect(ctx))
        .unwrap();
    assert_eq!(msg, "Signed in to Microsoft as Jane Doe (jane@contoso.com)");
    assert_eq!(w.with("teams", |ctx| c.status(ctx)), Status::Connected);

    let url = browser.opened.lock().unwrap()[0].clone();
    assert!(
        url.starts_with(&server.url("/login/contoso.onmicrosoft.com/oauth2/v2.0/authorize?")),
        "{url}"
    );
    assert!(url.contains(&format!("client_id={APP}")));
    assert!(
        url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A"),
        "{url}"
    );
    assert!(url.contains("code_challenge_method=S256"));
    for scope in SCOPES {
        assert!(url.contains(scope), "{scope} is missing from {url}");
    }
    // a public client: the verifier proves who asks, there is no secret
    let token = &server.requests_to("/login/contoso.onmicrosoft.com/oauth2/v2.0/token")[0];
    assert!(token.form_value("code_verifier").is_some());
    assert_eq!(token.form_value("client_secret"), None);
    assert_eq!(token.header("authorization"), None);

    w.with("teams", |ctx| c.disconnect(ctx));
    assert_eq!(w.with("teams", |ctx| c.status(ctx)), Status::Disconnected);
}

#[test]
fn connecting_without_the_app_id_says_where_to_find_it() {
    let w = World::new();
    let c = TeamsConnector::new(model::Teams::default());
    let err = w.with("teams", |ctx| c.connect(ctx)).unwrap_err();
    assert!(
        err.message.contains("Application (client) ID"),
        "{}",
        err.message
    );
}

fn lists_server() -> MockServer {
    MockServer::start(|req| match req.path() {
        "/graph/me" => MockResponse::json(200, json!({"id": "me-id", "displayName": "Me"})),
        "/graph/me/chats" => MockResponse::json(
            200,
            json!({
                "value": [
                    {"id": "19:one@unq.gbl.spaces", "chatType": "oneOnOne", "topic": null,
                     "members": [{"userId": "me-id", "displayName": "Me"}, {"userId": "bob", "displayName": "Bob Stone"}]},
                    {"id": "19:grp@thread.v2", "chatType": "group", "topic": "Release crew",
                     "members": []},
                    {"id": "19:many@thread.v2", "chatType": "group", "topic": "",
                     "members": [
                        {"userId": "me-id", "displayName": "Me"},
                        {"userId": "a", "displayName": "Ann"}, {"userId": "b", "displayName": "Ben"},
                        {"userId": "c", "displayName": "Cy"}, {"userId": "d", "displayName": "Di"},
                        {"userId": "e", "displayName": "Ed"}]},
                    {"id": "19:meet@thread.v2", "chatType": "meeting", "topic": "Sprint review", "members": []}
                ],
                // a link that leads elsewhere must never receive the token
                "@odata.nextLink": "https://evil.example/me/chats?page=2"
            }),
        ),
        "/graph/me/joinedTeams" => MockResponse::json(
            200,
            json!({"value": [
                {"id": "22222222-2222-2222-2222-222222222222", "displayName": "Zeta"},
                {"id": TEAM, "displayName": "Engineering"}
            ]}),
        ),
        p if p == format!("/graph/teams/{TEAM}/channels") => MockResponse::json(
            200,
            json!({"value": [
                {"id": "19:b@thread.tacv2", "displayName": "Random", "membershipType": "standard"},
                {"id": CHANNEL, "displayName": "General", "membershipType": "standard"},
                {"id": "19:p@thread.skype", "displayName": "Leads", "membershipType": "private"},
                {"id": "19:s@thread.tacv2", "displayName": "Partner", "membershipType": "shared"}
            ]}),
        ),
        "/graph/teams/22222222-2222-2222-2222-222222222222/channels" => {
            MockResponse::json(200, json!({"value": []}))
        }
        _ => MockResponse::new(404),
    })
}

#[test]
fn the_list_holds_channels_by_team_then_chats_by_kind_with_readable_names() {
    let server = lists_server();
    let w = signed_in();
    let items = w
        .with("teams", |ctx| connector(&server).options(ctx, "targets"))
        .unwrap();
    let shown: Vec<(String, String, String)> = items
        .iter()
        .map(|i| {
            (
                i.group.clone().unwrap_or_default(),
                i.label.clone(),
                i.value.clone(),
            )
        })
        .collect();
    let gen = format!("channel:{TEAM}/{CHANNEL}");
    assert_eq!(
        shown,
        vec![
            ("Team: Engineering".to_string(), "#General".to_string(), gen),
            (
                "Team: Engineering".to_string(),
                "#Leads (private)".to_string(),
                format!("channel:{TEAM}/19:p@thread.skype")
            ),
            (
                "Team: Engineering".to_string(),
                "#Random".to_string(),
                format!("channel:{TEAM}/19:b@thread.tacv2")
            ),
            (
                "Direct chats".to_string(),
                "Bob Stone".to_string(),
                "chat:19:one@unq.gbl.spaces".to_string()
            ),
            (
                "Group chats".to_string(),
                "Ann, Ben, Cy, +2 more".to_string(),
                "chat:19:many@thread.v2".to_string()
            ),
            (
                "Group chats".to_string(),
                "Release crew".to_string(),
                "chat:19:grp@thread.v2".to_string()
            ),
            (
                "Meeting chats".to_string(),
                "Sprint review".to_string(),
                "chat:19:meet@thread.v2".to_string()
            ),
        ]
    );
    // every value can be sent back as a target
    for i in &items {
        assert!(parse_target(&i.value).is_ok(), "{}", i.value);
    }
    // the bearer token only ever went to the mock Graph, never to the foreign page link
    assert!(server
        .requests()
        .iter()
        .all(|r| r.header("authorization") == Some("Bearer AT")));
    assert!(server.requests_to("/me/chats").is_empty());
    assert!(server
        .requests()
        .iter()
        .any(
            |r| r.path() == "/graph/me/chats" && r.query().contains("%24expand=members")
                || r.query().contains("$expand=members")
        ));
}

#[test]
fn one_list_failing_does_not_hide_the_other() {
    // an organisation that withholds the channel permission still has its chats
    let server = MockServer::start(|req| match req.path() {
        "/graph/me" => MockResponse::json(200, json!({"id": "me-id"})),
        "/graph/me/chats" => MockResponse::json(
            200,
            json!({"value": [{"id": "19:grp@thread.v2", "chatType": "group", "topic": "Crew"}]}),
        ),
        _ => MockResponse::json(403, json!({"error": {"message": "Forbidden"}})),
    });
    let w = signed_in();
    let items = w
        .with("teams", |ctx| connector(&server).options(ctx, "targets"))
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Crew");

    // both failing is an error the person can read
    let server =
        MockServer::start(|_| MockResponse::json(403, json!({"error": {"message": "Forbidden"}})));
    let err = w
        .with("teams", |ctx| connector(&server).options(ctx, "targets"))
        .unwrap_err();
    assert!(
        err.message.contains("press Sign in again"),
        "{}",
        err.message
    );
    assert!(w
        .with("teams", |ctx| connector(&server).options(ctx, "nope"))
        .is_err());
}

#[test]
fn the_settings_are_checked_before_they_are_saved() {
    use crate::model::{check, Connection, Settings};
    let conn = |client_id: &str, tenant: &str| Connection {
        name: "Teams".into(),
        enabled: true,
        settings: Settings::Teams(model::Teams {
            client_id: client_id.into(),
            tenant: tenant.into(),
        }),
    };
    assert!(check(&conn(APP, "organizations")).is_ok());
    assert!(check(&conn(&format!(" {APP} "), "contoso.onmicrosoft.com")).is_ok());
    assert!(check(&conn(APP, TEAM)).is_ok());
    for (id, tenant) in [
        ("", "organizations"),
        ("not-a-guid", "organizations"),
        (APP, ""),
        (APP, "common"),
        (APP, "consumers"),
        (APP, "evil.example/path"),
        (APP, "a b"),
    ] {
        assert!(check(&conn(id, tenant)).is_err(), "{id:?} {tenant:?}");
    }
    assert_eq!(model::Kind::Teams.secret_names(), &["oauth"]);
    assert!(model::Kind::Teams.typed_secrets().is_empty());
}

#[test]
fn the_message_is_written_as_teams_html_and_stays_escaped() {
    let p = payload(&[(
        "message",
        "**Crash** on *save* <b>x</b>\n- one\n- two\n> seen\n__urgent__ `a&b`",
    )]);
    assert_eq!(
        compose(&p, false, false).html,
        "<p><b>Crash</b> on <i>save</i> &lt;b&gt;x&lt;/b&gt;</p><ul><li>one</li><li>two</li></ul>\
         <blockquote>seen</blockquote><p><u>urgent</u> <code>a&amp;b</code></p>"
    );
}
