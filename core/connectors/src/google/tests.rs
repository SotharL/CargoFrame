use super::*;
use crate::oauth::Tokens;
use crate::payload::Field;
use crate::secrets::SecretStore;
use crate::testkit::{ApproveBrowser, World};
use cargoframe_net::mock::{MockResponse, MockServer};
use cargoframe_safe::FailureKind;

fn connector(server: &MockServer, settings: model::Google) -> GoogleConnector {
    GoogleConnector::new(settings).with_endpoints(
        Endpoints {
            auth: "https://accounts.example/auth".into(),
            token: server.url("/token"),
            revoke: server.url("/revoke"),
            api: server.base(),
            sheets: server.base(),
        },
        Duration::from_secs(5),
    )
}

fn narrow() -> model::Google {
    model::Google {
        client_id: "cid.apps".into(),
        sheets: false,
        full_drive: false,
    }
}

fn signed_in() -> World {
    let w = World::new().secret("g", "client_secret", "CSEC");
    w.secrets
        .set(
            "g/oauth",
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

fn payload() -> Payload {
    Payload {
        title: "App".into(),
        image: Some(Attachment {
            name: "screenshot.png".into(),
            mime: "image/png".into(),
            data: vec![0x89, 1, 2],
        }),
        files_zip: Some(Attachment {
            name: "files.zip".into(),
            mime: "application/zip".into(),
            data: vec![b'P', b'K'],
        }),
        vars: [
            ("capture.date".to_string(), "2026-10-23".to_string()),
            ("capture.time".to_string(), "08:00:01".to_string()),
        ]
        .into(),
        ..Payload::default()
    }
}

/// A Drive that already has the CargoFrame folder (or none), and accepts uploads and rows.
fn drive(has_folder: bool) -> MockServer {
    MockServer::start(move |req| match (req.method.as_str(), req.path()) {
        ("GET", "/drive/v3/files") => {
            if has_folder {
                MockResponse::json(200, json!({"files":[{"id":"FOLDER1"}]}))
            } else {
                MockResponse::json(200, json!({"files":[]}))
            }
        }
        ("POST", "/drive/v3/files") => MockResponse::json(200, json!({"id":"NEWFOLDER"})),
        ("POST", "/upload/drive/v3/files") => {
            let name = if req.text().contains("screenshot.png") {
                "shot"
            } else {
                "zip"
            };
            MockResponse::json(
                200,
                json!({"id":name,"webViewLink":format!("https://drive.google.com/file/d/{name}/view")}),
            )
        }
        ("POST", p) if p.contains(":append") => {
            MockResponse::json(200, json!({"updates":{"updatedRows":1}}))
        }
        ("GET", "/drive/v3/about") => MockResponse::json(
            200,
            json!({"user":{"displayName":"Jane","emailAddress":"jane@example.com"}}),
        ),
        _ => MockResponse::new(404),
    })
}

#[test]
fn ids_are_taken_from_links_and_checked() {
    assert_eq!(
        id_from_link(
            "https://drive.google.com/drive/folders/1AbC_d-9?usp=sharing",
            "folders"
        )
        .as_deref(),
        Some("1AbC_d-9")
    );
    assert_eq!(
        id_from_link("1AbC_d-9", "folders").as_deref(),
        Some("1AbC_d-9")
    );
    assert_eq!(
        id_from_link(
            "https://docs.google.com/spreadsheets/d/SHEET123/edit#gid=0",
            "d"
        )
        .as_deref(),
        Some("SHEET123")
    );
    for bad in [
        "",
        "https://example.com/x",
        "a b",
        "../x",
        "https://drive.google.com/drive/folders/",
        "a/b",
    ] {
        assert_eq!(id_from_link(bad, "folders"), None, "{bad}");
    }
}

#[test]
fn scopes_follow_the_options() {
    let s = |sheets, full| {
        GoogleConnector::new(model::Google {
            client_id: "x".into(),
            sheets,
            full_drive: full,
        })
        .scopes()
    };
    assert_eq!(
        s(false, false),
        vec!["https://www.googleapis.com/auth/drive.file"]
    );
    assert_eq!(s(true, false).len(), 2);
    assert_eq!(
        s(false, true),
        vec!["https://www.googleapis.com/auth/drive"]
    );
}

#[test]
fn uploads_go_to_the_existing_cargoframe_folder() {
    let server = drive(true);
    let w = signed_in();
    let out = w
        .with("g", |ctx| {
            connector(&server, narrow()).send(ctx, &payload())
        })
        .unwrap();
    assert_eq!(out.summary, "Uploaded to Google Drive");
    assert_eq!(
        out.url.as_deref(),
        Some("https://drive.google.com/file/d/shot/view")
    );

    let uploads = server.requests_to("/upload/drive/v3/files");
    assert_eq!(uploads.len(), 2);
    for u in &uploads {
        assert_eq!(u.header("authorization"), Some("Bearer AT"));
        assert!(u
            .header("content-type")
            .unwrap()
            .starts_with("multipart/related; boundary="));
        let text = String::from_utf8_lossy(&u.body).into_owned();
        assert!(text.contains(r#""parents":["FOLDER1"]"#), "{text}");
        assert_eq!(u.query_param("uploadType").as_deref(), Some("multipart"));
    }
    assert!(String::from_utf8_lossy(&uploads[0].body).contains(r#""name":"screenshot.png""#));
    assert!(String::from_utf8_lossy(&uploads[1].body).contains(r#""name":"files.zip""#));
    assert!(server
        .requests_to("/drive/v3/files")
        .iter()
        .all(|r| r.method == "GET"));
}

#[test]
fn the_folder_is_created_when_missing_and_found_once() {
    let server = drive(false);
    let w = signed_in();
    w.with("g", |ctx| {
        connector(&server, narrow()).send(ctx, &payload())
    })
    .unwrap();
    let create = server
        .requests_to("/drive/v3/files")
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(
        create.json(),
        json!({"name":"CargoFrame","mimeType":"application/vnd.google-apps.folder"})
    );
    let text =
        String::from_utf8_lossy(&server.requests_to("/upload/drive/v3/files")[0].body).into_owned();
    assert!(text.contains("NEWFOLDER"));
    assert_eq!(
        w.progress.get("google.folder").as_deref(),
        Some("NEWFOLDER")
    );
}

#[test]
fn a_named_folder_is_used_as_given() {
    let server = drive(true);
    let w = signed_in();
    let mut p = payload();
    p.fields.push(Field::text(
        "folder",
        "https://drive.google.com/drive/folders/MYFOLDER?usp=sharing",
    ));
    let c = connector(
        &server,
        model::Google {
            full_drive: true,
            ..narrow()
        },
    );
    w.with("g", |ctx| c.send(ctx, &p)).unwrap();
    let text =
        String::from_utf8_lossy(&server.requests_to("/upload/drive/v3/files")[0].body).into_owned();
    assert!(text.contains("MYFOLDER"));
    assert!(
        server.requests_to("/drive/v3/files").is_empty(),
        "no folder lookup is needed"
    );

    p.fields.retain(|f| f.key != "folder");
    p.fields.push(Field::text("folder", "not a folder!"));
    assert!(w
        .with("g", |ctx| c.send(ctx, &p))
        .unwrap_err()
        .message
        .contains("not a Drive folder"));
}

#[test]
fn without_full_drive_a_refused_folder_explains_why() {
    let server = MockServer::start(|req| {
        if req.path() == "/upload/drive/v3/files" {
            MockResponse::json(404, json!({"error":{"message":"File not found: OTHER."}}))
        } else {
            MockResponse::new(404)
        }
    });
    let w = signed_in();
    let mut p = payload();
    p.fields.push(Field::text("folder", "OTHER"));
    let err = w
        .with("g", |ctx| connector(&server, narrow()).send(ctx, &p))
        .unwrap_err();
    assert!(err.message.contains("File not found"));
    assert!(err.message.contains("full Drive option"), "{}", err.message);
}

#[test]
fn a_retry_uploads_only_what_is_missing() {
    let zip_fails = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let z = zip_fails.clone();
    let server = MockServer::start(move |req| match req.path() {
        "/drive/v3/files" => MockResponse::json(200, json!({"files":[{"id":"F"}]})),
        "/upload/drive/v3/files" => {
            if req.text().contains("files.zip")
                && z.swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                MockResponse::new(500)
            } else {
                MockResponse::json(
                    200,
                    json!({"id":"x","webViewLink":"https://drive.google.com/file/d/x/view"}),
                )
            }
        }
        _ => MockResponse::new(404),
    });
    let w = signed_in();
    let c = connector(&server, narrow());
    let err = w.with("g", |ctx| c.send(ctx, &payload())).unwrap_err();
    assert_eq!(err.kind, FailureKind::Server(500));
    w.with("g", |ctx| c.send(ctx, &payload())).unwrap();
    let uploads = server.requests_to("/upload/drive/v3/files");
    let shots = uploads
        .iter()
        .filter(|r| String::from_utf8_lossy(&r.body).contains("screenshot.png"))
        .count();
    let zips = uploads
        .iter()
        .filter(|r| String::from_utf8_lossy(&r.body).contains("files.zip"))
        .count();
    assert_eq!((shots, zips), (1, 2));
}

#[test]
fn a_spreadsheet_row_holds_the_date_the_columns_and_the_links() {
    let server = drive(true);
    let w = signed_in();
    let c = connector(
        &server,
        model::Google {
            sheets: true,
            ..narrow()
        },
    );
    let mut p = payload();
    p.fields = vec![
        Field::text(
            "spreadsheet",
            "https://docs.google.com/spreadsheets/d/SHEET9/edit",
        ),
        Field::text("sheet", "Bugs"),
        Field::text("col1", "Crash"),
        Field::text("col3", "High"),
    ];
    let out = w.with("g", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(out.summary, "Uploaded to Google Drive and added a row");
    let append = server
        .requests()
        .into_iter()
        .find(|r| r.path().contains(":append"))
        .unwrap();
    assert_eq!(
        append.path(),
        "/v4/spreadsheets/SHEET9/values/Bugs%21A1:append"
    );
    assert_eq!(
        append.query_param("valueInputOption").as_deref(),
        Some("USER_ENTERED")
    );
    assert_eq!(
        append.json(),
        json!({"values":[["2026-10-23 08:00:01","Crash","","High","","",
            "https://drive.google.com/file/d/shot/view","https://drive.google.com/file/d/zip/view"]]})
    );
}

#[test]
fn a_row_alone_needs_no_upload_and_the_option_must_be_on() {
    let server = drive(true);
    let w = signed_in();
    let p = Payload {
        fields: vec![
            Field::text("spreadsheet", "SHEET9"),
            Field::text("col1", "x"),
        ],
        ..Payload::default()
    };
    let c = connector(
        &server,
        model::Google {
            sheets: true,
            ..narrow()
        },
    );
    let out = w.with("g", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(out.summary, "Added a row to the spreadsheet");
    assert!(server.requests_to("/upload/drive/v3/files").is_empty());

    let err = w
        .with("g", |ctx| connector(&server, narrow()).send(ctx, &p))
        .unwrap_err();
    assert!(
        err.message.contains("spreadsheet option"),
        "{}",
        err.message
    );
    let err = w
        .with("g", |ctx| {
            connector(&server, narrow()).send(ctx, &Payload::default())
        })
        .unwrap_err();
    assert!(err.message.contains("nothing to send"));
}

#[test]
fn a_failed_row_says_the_files_are_already_up() {
    let server = MockServer::start(|req| match req.path() {
        "/drive/v3/files" => MockResponse::json(200, json!({"files":[{"id":"F"}]})),
        "/upload/drive/v3/files" => MockResponse::json(
            200,
            json!({"id":"x","webViewLink":"https://drive.google.com/file/d/x/view"}),
        ),
        _ => MockResponse::json(
            403,
            json!({"error":{"message":"The caller does not have permission"}}),
        ),
    });
    let w = signed_in();
    let c = connector(
        &server,
        model::Google {
            sheets: true,
            ..narrow()
        },
    );
    let mut p = payload();
    p.fields = vec![Field::text("spreadsheet", "SHEET9")];
    let err = w.with("g", |ctx| c.send(ctx, &p)).unwrap_err();
    assert_eq!(err.kind, FailureKind::AuthExpired);
    assert!(
        err.message
            .starts_with("The files were uploaded, but could not add the row"),
        "{}",
        err.message
    );
}

#[test]
fn the_folder_list_and_the_test_line() {
    let server = MockServer::start(|req| match req.path() {
        "/drive/v3/files" => MockResponse::json(
            200,
            json!({"files":[{"id":"1","name":"Bugs"},{"id":"2","name":"Ideas"}]}),
        ),
        "/drive/v3/about" => {
            MockResponse::json(200, json!({"user":{"emailAddress":"jane@example.com"}}))
        }
        _ => MockResponse::new(404),
    });
    let w = signed_in();
    let c = connector(&server, narrow());
    let folders = w.with("g", |ctx| c.options(ctx, "folders")).unwrap();
    assert_eq!(
        folders.iter().map(|f| f.label.as_str()).collect::<Vec<_>>(),
        vec!["Bugs", "Ideas"]
    );
    let q = server.requests_to("/drive/v3/files")[0]
        .query_param("q")
        .unwrap();
    assert!(
        q.contains("mimeType = 'application/vnd.google-apps.folder'")
            && q.contains("trashed = false")
    );
    assert_eq!(
        w.with("g", |ctx| c.test(ctx)).unwrap(),
        "Signed in to Google as jane@example.com"
    );
    assert!(w.with("g", |ctx| c.options(ctx, "nope")).is_err());
}

#[test]
fn connecting_needs_the_client_secret_then_signs_in_with_the_right_permissions() {
    let server = MockServer::start(|req| match req.path() {
        "/token" => MockResponse::json(
            200,
            json!({"access_token":"AT","refresh_token":"RT","expires_in":3600}),
        ),
        "/drive/v3/about" => {
            MockResponse::json(200, json!({"user":{"emailAddress":"jane@example.com"}}))
        }
        "/revoke" => MockResponse::ok(),
        _ => MockResponse::new(404),
    });
    let c = connector(
        &server,
        model::Google {
            sheets: true,
            ..narrow()
        },
    );
    let browser = ApproveBrowser::new();

    let bare = World::new();
    let err = bare
        .with_browser("g", &browser, |ctx| c.connect(ctx))
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::ConnectorUnavailable);
    assert!(
        browser.opened.lock().unwrap().is_empty(),
        "no sign-in without the secret"
    );

    let w = World::new().secret("g", "client_secret", "CSEC");
    assert_eq!(w.with("g", |ctx| c.status(ctx)), Status::Disconnected);
    let msg = w.with_browser("g", &browser, |ctx| c.connect(ctx)).unwrap();
    assert_eq!(msg, "Signed in to Google as jane@example.com");
    let url = browser.opened.lock().unwrap()[0].clone();
    assert!(url.contains("access_type=offline") && url.contains("prompt=consent"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("drive.file") && url.contains("spreadsheets"));
    let token = &server.requests_to("/token")[0];
    assert_eq!(token.form_value("client_secret").as_deref(), Some("CSEC"));
    assert!(token.form_value("code_verifier").is_some());
    assert_eq!(w.with("g", |ctx| c.status(ctx)), Status::Connected);

    w.with("g", |ctx| c.disconnect(ctx));
    assert_eq!(w.with("g", |ctx| c.status(ctx)), Status::Disconnected);
}
