use super::*;
use crate::connections::tests::{fixture, save, Fixture};
use crate::session::tests::{fake_capture, NOW};
use crate::session::{prepare_with, Session};
use cargoframe_config::{Kind, Trigger};
use cargoframe_net::mock::{MockResponse, MockServer};
use serde_json::json;
use std::cell::RefCell;
use std::io::Read;
use std::sync::Mutex;

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 1, 2, 3];
const SECRET: &str = "TOP-SECRET-TOKEN-123";

/// Opens links without doing anything.
struct NoBrowser;
impl Browser for NoBrowser {
    fn open(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}
/// Reads what it is asked to paste while the files still exist, and says it was sent.
#[derive(Default)]
struct RecordingDeliver {
    seen: Mutex<Vec<Seen>>,
}

struct Seen {
    url: String,
    /// File name and content, in the order they would be pasted.
    files: Vec<(String, Vec<u8>)>,
    text: String,
    submit: bool,
}

impl Deliver for RecordingDeliver {
    fn deliver(
        &self,
        d: &cargoframe_connectors::Delivery,
        open: &dyn Fn(&str) -> Result<(), String>,
        _: &dyn Fn() -> bool,
    ) -> Result<cargoframe_connectors::Delivered, cargoframe_connectors::DeliverError> {
        open(&d.open_url).map_err(cargoframe_connectors::DeliverError::Open)?;
        let files = d
            .files
            .iter()
            .map(|p| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    fs::read(p).unwrap(),
                )
            })
            .collect();
        self.seen.lock().unwrap().push(Seen {
            url: d.open_url.clone(),
            files,
            text: d.text.clone(),
            submit: d.submit,
        });
        Ok(cargoframe_connectors::Delivered {
            window: "Claude".into(),
            files: d.files.len(),
            skipped_files: 0,
            text: true,
            submitted: d.submit,
            handed_over: false,
        })
    }
}
struct World {
    f: Fixture,
    server: MockServer,
    session: Session,
    out: tempfile::TempDir,
    /// Where captures keep their temporary copies.
    ws: Workspace,
    deliver: RecordingDeliver,
}

/// A project with a log file, a layout with a note, a files block and a sharing button that
/// posts JSON to the mock server, and a capture of it.
fn world(
    handler: impl Fn(&cargoframe_net::mock::Recorded) -> MockResponse + Send + Sync + 'static,
) -> World {
    let f = fixture();
    let server = MockServer::start(handler);
    save(
        &f.store,
        "api",
        json!({
            "name": "Hook", "kind": "custom", "base_url": server.base(),
            "auth": {"type": "bearer"},
            "send": {"method": "POST", "path": "/hook", "body": {"type": "json",
                "template": "{\"summary\":\"{{summary}}\",\"build\":\"{{build}}\",\"kind\":\"{{kind}}\",\"prompt\":\"{{prompt}}\"}"}},
            "result_url_path": "url"
        }),
    );
    f.connections
        .set_secret(&f.store, "api", "secret", SECRET)
        .unwrap();

    let project = f._dir.path().join("Foo");
    fs::create_dir_all(project.join("bin")).unwrap();
    fs::write(project.join("run.log"), "LOGLINE").unwrap();
    fs::write(project.join("other.log"), "OTHER").unwrap();
    let exe = project.join("bin").join("Foo");

    let layout: Layout = serde_json::from_value(json!({
        "id": "", "name": "L", "blocks": [
            {"id": "note", "rect": {"x":0,"y":0,"w":6,"h":2}, "type": "text",
             "title": "Note", "default_text": "hello"},
            {"id": "prompt", "rect": {"x":6,"y":0,"w":6,"h":2}, "type": "text",
             "title": "Prompt", "role": "prompt", "default_text": ""},
            {"id": "logs", "rect": {"x":0,"y":2,"w":6,"h":2}, "type": "files",
             "title": "Logs", "include": ["*.log"], "preselect": ["run.log"]},
            {"id": "send", "rect": {"x":6,"y":2,"w":6,"h":2}, "type": "sharing_button",
             "label": "Send", "connection_id": "api", "prompt_block_id": "prompt",
             "mappings": [
                {"field": "summary", "source": {"kind": "block", "value": "note"}},
                {"field": "build", "source": {"kind": "rule", "value": "version"}},
                {"field": "kind", "source": {"kind": "fixed", "value": "bug in {{app.title}}"}}
             ]}
        ]
    }))
    .unwrap();
    f.store.save_item(Kind::Layouts, "bug", &layout).unwrap();
    let trigger = Trigger {
        name: "Bug".into(),
        layout_id: Some("bug".into()),
        ..Trigger::default()
    };
    let captured = fake_capture(Some(&exe.display().to_string()));
    let mut session = prepare_with(
        &f.store,
        &f.connections,
        "bugtrig",
        &trigger,
        captured,
        NOW,
        7,
    )
    .unwrap();
    session
        .view
        .variables
        .insert("rule.version".into(), "1.2.3".into());
    session.png = PNG.to_vec();
    let out = tempfile::tempdir().unwrap();
    let ws = Workspace::new(out.path().join("captures"));
    World {
        f,
        server,
        session,
        out,
        ws,
        deliver: RecordingDeliver::default(),
    }
}

fn request() -> Request {
    Request {
        session_id: 7,
        button_id: "send".into(),
        texts: [
            ("note".to_string(), "it \"broke\"".to_string()),
            ("prompt".to_string(), "why?".to_string()),
        ]
        .into(),
        dropdowns: BTreeMap::new(),
        tags: BTreeMap::new(),
        deeper: BTreeMap::new(),
        files: [("logs".to_string(), vec!["run.log".to_string()])].into(),
        image_edited: false,
        still: false,
        clip_edit: None,
        send_blocks: None,
        save_here: false,
    }
}

fn services<'a>(w: &'a World, cancelled: &'a dyn Fn() -> bool) -> Services<'a> {
    Services {
        store: &w.f.store,
        connections: &w.f.connections,
        browser: &NoBrowser,
        deliver: &w.deliver,
        workspace: &w.ws,
        cancelled,
        now: NOW,
    }
}

fn go(w: &World, req: &Request, picks: Vec<Option<PickedFolder>>) -> (ShareResult, u32) {
    let never = || false;
    let svc = services(w, &never);
    let queue = RefCell::new(picks.into_iter());
    let asked = RefCell::new(0u32);
    let mut picker = |_n: u32| {
        *asked.borrow_mut() += 1;
        queue.borrow_mut().next().flatten()
    };
    let mut sleep = |_d: Duration| {};
    let r = run(&svc, &mut picker, &mut sleep, &w.session, req);
    let n = *asked.borrow();
    (r, n)
}

fn set_safe(w: &World, folder: Option<&str>, retries: u32) {
    let s = Safe {
        folder: folder.map(String::from),
        max_retries: retries,
        ..Safe::default()
    };
    w.f.store.save(&s).unwrap();
}

fn zip_names(path: &str) -> Vec<String> {
    let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().unwrap().to_string())
        .collect()
}

fn zip_text(path: &str, name: &str) -> String {
    let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

/// Point the test layout's button at a Claude connection.
fn use_claude(w: &mut World, settings: serde_json::Value) {
    let mut conn = json!({"name": "Claude", "kind": "claude"});
    for (k, v) in settings.as_object().unwrap() {
        conn[k] = v.clone();
    }
    save(&w.f.store, "claude", conn);
    let layout = w.session.view.layout.as_mut().unwrap();
    for b in &mut layout.blocks {
        if let BlockKind::SharingButton(btn) = &mut b.kind {
            btn.connection_id = Some("claude".into());
            btn.mappings.clear();
        }
    }
}

#[test]
fn a_share_to_claude_delivers_the_screenshot_the_files_and_the_message_in_one_go() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    // the browser link on every system (the default opens the desktop app on Windows)
    use_claude(&mut w, json!({"open_with": "browser"}));
    set_safe(&w, None, 0);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Sent { summary, close, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(summary, "Sent 2 files and the message to Claude");
    assert!(close);
    let seen = w.deliver.seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "one delivery, not a copy step per item");
    let s = &seen[0];
    assert_eq!(s.url, "https://claude.ai/new");
    assert!(s.submit);
    // The screenshot first, then the ticked file, each with the content it had.
    assert_eq!(
        s.files,
        vec![
            ("screenshot.png".to_string(), PNG.to_vec()),
            ("run.log".to_string(), b"LOGLINE".to_vec()),
        ]
    );
    assert!(s.text.starts_with("why?"), "{}", s.text);
    assert!(s.text.contains("Captured window: Editor"), "{}", s.text);
    assert!(s.text.contains("Attached files: run.log"), "{}", s.text);
    // Nothing went to the mock service: Claude is not a web request.
    assert!(w.server.requests_to("/hook").is_empty());
}

#[test]
fn the_edited_picture_is_what_claude_gets() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    let edited = [0x89, b'P', b'N', b'G', 9, 9, 9, 9];
    w.session.set_edited(Some(edited.to_vec()));
    let mut req = request();
    req.image_edited = true;
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    assert_eq!(
        w.deliver.seen.lock().unwrap()[0].files[0].1,
        edited.to_vec()
    );
}

#[test]
fn the_capture_folder_goes_with_the_capture() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    assert!(
        w.session.dir_path().is_none(),
        "no folder until a destination needs files"
    );
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let dir = w.session.dir_path().expect("the share made the folder");
    assert!(dir.join("screenshot.png").is_file());
    assert!(dir.join("files").join("run.log").is_file());
    // The window closes: the capture is dropped, and so is its folder.
    let World { session, ws, .. } = w;
    drop(session);
    assert!(!dir.exists(), "the folder outlived its capture");
    assert_eq!(ws.sweep(&[], Duration::ZERO).removed, 0);
}

#[test]
fn claude_gets_the_files_as_they_were_when_it_was_shared_not_a_zip() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({"send_automatically": false}));
    set_safe(&w, None, 0);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Sent { summary, .. } = r else {
        panic!("{r:?}")
    };
    assert!(summary.starts_with("Pasted"), "{summary}");
    assert!(!w.deliver.seen.lock().unwrap()[0].submit);
}

#[test]
fn claude_has_a_limit_on_files_per_message() {
    assert!(check_file_count(0, true).is_ok());
    assert!(check_file_count(MAX_FILES - 1, true).is_ok());
    let err = check_file_count(MAX_FILES + 2, true).unwrap_err();
    assert!(
        err.contains("at most 20") && err.contains("untick 3"),
        "{err}"
    );
    // Without the screenshot there is room for one more file.
    assert!(check_file_count(MAX_FILES, false).is_ok());
    let err = check_file_count(MAX_FILES + 1, false).unwrap_err();
    assert!(
        !err.contains("screenshot") && err.contains("untick 1"),
        "{err}"
    );
}

/// Set the picture and file rules of the test layout's button.
fn set_rules(w: &mut World, rules: SendRules) {
    let layout = w.session.view.layout.as_mut().unwrap();
    for b in &mut layout.blocks {
        if let BlockKind::SharingButton(btn) = &mut b.kind {
            btn.send = rules;
        }
    }
}

fn project_file(w: &World, name: &str) -> PathBuf {
    w.f._dir.path().join("Foo").join(name)
}

#[test]
fn a_button_can_leave_the_picture_out() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    set_rules(
        &mut w,
        SendRules {
            screenshot: false,
            ..SendRules::default()
        },
    );
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Sent { summary, notes, .. } = r else {
        panic!("{r:?}")
    };
    assert_eq!(summary, "Sent 1 file and the message to Claude");
    let seen = w.deliver.seen.lock().unwrap();
    assert_eq!(
        seen[0].files,
        vec![("run.log".to_string(), b"LOGLINE".to_vec())]
    );
    assert!(
        seen[0].text.contains("Attached files: run.log"),
        "{}",
        seen[0].text
    );
    assert!(!notes.iter().any(|n| n.contains("screenshot")), "{notes:?}");
}

#[test]
fn a_button_can_leave_the_files_out_even_when_some_are_ticked() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    set_rules(
        &mut w,
        SendRules {
            files: FilesHow::None,
            ..SendRules::default()
        },
    );
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let seen = w.deliver.seen.lock().unwrap();
    let names: Vec<&str> = seen[0].files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["screenshot.png"]);
    assert!(!seen[0].text.contains("Attached files"), "{}", seen[0].text);
}

/// The boxes the window had ticked, as the request carries them.
fn ticked(ids: &[&str]) -> Option<Vec<String>> {
    Some(ids.iter().map(|s| s.to_string()).collect())
}

#[test]
fn clearing_the_files_box_leaves_the_ticked_files_out() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    let req = Request {
        send_blocks: ticked(&["prompt"]),
        ..request()
    };
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Sent { summary, .. } = r else {
        panic!("{r:?}")
    };
    // the layout has no screenshot block, so the capture still goes where the picture would
    assert_eq!(summary, "Sent 1 file and the message to Claude");
    let seen = w.deliver.seen.lock().unwrap();
    let names: Vec<&str> = seen[0].files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["screenshot.png"], "the files box was cleared");
    assert!(!seen[0].text.contains("Attached files"), "{}", seen[0].text);
}

#[test]
fn a_cleared_prompt_box_is_refused_for_claude_and_nothing_is_sent() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    let req = Request {
        send_blocks: ticked(&["logs"]),
        ..request()
    };
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}")
    };
    assert_eq!(
        reasons,
        vec!["\u{201c}Prompt\u{201d} is needed by this button".to_string()]
    );
    assert!(w.deliver.seen.lock().unwrap().is_empty());
}

#[test]
fn a_box_for_something_the_button_cannot_send_is_refused() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    // the note is mapped to nothing on a Claude button
    let req = Request {
        send_blocks: ticked(&["prompt", "note"]),
        ..request()
    };
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}")
    };
    assert_eq!(
        reasons,
        vec!["This button has no field for \u{201c}Note\u{201d}".to_string()]
    );
    // and a box for a block that is not in the layout
    let req = Request {
        send_blocks: ticked(&["prompt", "ghost"]),
        ..request()
    };
    assert!(matches!(
        go(&w, &req, vec![]).0,
        ShareResult::Blocked { .. }
    ));
}

#[test]
fn clearing_the_box_of_a_mapped_block_leaves_its_field_unfilled_and_the_service_says_so() {
    let w = world(|_| MockResponse::json(200, json!({"url": "https://x/1"})));
    set_safe(&w, None, 0);
    // The test layout maps the note to `summary`, and the Custom API's request uses
    // `{{summary}}`: with the note's box cleared nothing fills it, and the request is
    // refused by name instead of being sent with a hole. The share is kept.
    let req = Request {
        send_blocks: ticked(&["prompt"]),
        ..request()
    };
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Failed { message, saved, .. } = r else {
        panic!("{r:?}")
    };
    assert_eq!(message, "The request uses {{summary}} but nothing fills it");
    assert!(saved.is_some(), "the share is kept when it could not go");
    assert!(w.server.requests().is_empty(), "nothing was sent");
    // the same click with the box ticked sends the note, JSON-escaped
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let bodies = w.server.requests();
    let body = String::from_utf8_lossy(&bodies.last().unwrap().body).to_string();
    assert!(body.contains("it \\\"broke\\\""), "{body}");
}

#[test]
fn a_zip_kept_after_leaving_the_picture_out_has_no_picture() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    set_safe(&w, Some(&w.out.path().display().to_string()), 0);
    // The test layout posts to the mock server (which refuses), so the share is kept.
    set_rules(
        &mut w,
        SendRules {
            screenshot: false,
            ..SendRules::default()
        },
    );
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Failed {
        saved: Some(saved), ..
    } = r
    else {
        panic!("{r:?}")
    };
    let names = zip_names(&saved.path);
    assert!(!names.iter().any(|n| n == "screenshot.png"), "{names:?}");
    assert!(names.iter().any(|n| n == "share.json"), "{names:?}");
}

#[test]
fn text_files_can_go_into_the_message_instead_of_being_attached() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    set_rules(
        &mut w,
        SendRules {
            files: FilesHow::Inline,
            ..SendRules::default()
        },
    );
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Sent { summary, notes, .. } = r else {
        panic!("{r:?}")
    };
    assert_eq!(summary, "Sent 1 file and the message to Claude");
    let seen = w.deliver.seen.lock().unwrap();
    let names: Vec<&str> = seen[0].files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["screenshot.png"], "the log is text now, not a file");
    assert!(
        seen[0].text.contains("File run.log:\n```\nLOGLINE\n```"),
        "{}",
        seen[0].text
    );
    assert!(!seen[0].text.contains("Attached files"), "{}", seen[0].text);
    assert!(
        notes
            .iter()
            .any(|n| n.contains("1 file was written into the message as text")),
        "{notes:?}"
    );
}

#[test]
fn a_file_that_is_too_big_or_not_text_is_attached_when_inline_was_asked() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    set_rules(
        &mut w,
        SendRules {
            files: FilesHow::Inline,
            inline_limit_kb: 1,
            ..SendRules::default()
        },
    );
    // Too big for 1 KB.
    fs::write(project_file(&w, "run.log"), "x".repeat(2048)).unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Sent { notes, .. } = r else {
        panic!("{r:?}")
    };
    {
        let seen = w.deliver.seen.lock().unwrap();
        let names: Vec<&str> = seen[0].files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["screenshot.png", "run.log"]);
        assert!(!seen[0].text.contains("File run.log"), "{}", seen[0].text);
        assert!(seen[0].text.contains("Attached files: run.log"));
    }
    assert!(
        notes
            .iter()
            .any(|n| n.contains("past the 1 KB limit") && n.contains("attached instead")),
        "{notes:?}"
    );
}

#[test]
fn a_file_that_is_not_text_stays_a_file_when_inline_was_asked() {
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    set_rules(
        &mut w,
        SendRules {
            files: FilesHow::Inline,
            ..SendRules::default()
        },
    );
    fs::write(project_file(&w, "run.log"), [0u8, 1, 2, 255]).unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Sent { notes, .. } = r else {
        panic!("{r:?}")
    };
    let seen = w.deliver.seen.lock().unwrap();
    let names: Vec<&str> = seen[0].files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["screenshot.png", "run.log"]);
    assert!(
        notes.iter().any(|n| n.contains("not plain text")),
        "{notes:?}"
    );
}

#[test]
fn the_inline_limit_is_shared_by_all_the_files_together() {
    let mut files = LocalCopies::default();
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in [
        ("a.txt", "x".repeat(600)),
        ("b.txt", "y".repeat(600)),
        ("c.txt", "z".repeat(100)),
    ] {
        let p = dir.path().join(name);
        fs::write(&p, body).unwrap();
        files.files.push((name.to_string(), p));
    }
    let inline = move_text_inline(&mut files, 1024);
    // The first fits, the second would pass the limit, the small third still fits after it.
    let names: Vec<&str> = inline.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["a.txt", "c.txt"]);
    assert_eq!(files.files.len(), 1);
    assert_eq!(files.files[0].0, "b.txt");
}

#[test]
fn what_counts_as_text() {
    assert_eq!(as_text(b"hello").as_deref(), Some("hello"));
    assert_eq!(as_text("\u{feff}bom".as_bytes()).as_deref(), Some("bom"));
    assert_eq!(as_text("héllo".as_bytes()).as_deref(), Some("héllo"));
    assert!(as_text(&[104, 0, 105]).is_none(), "a NUL byte is binary");
    assert!(as_text(&[0xff, 0xfe, 0x41]).is_none(), "not UTF-8");
}

#[test]
fn picking_a_chat_works_without_mapping_the_chat_field() {
    const CHAT: &str = "0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0";
    let mut w = world(|_| MockResponse::json(500, json!({})));
    use_claude(&mut w, json!({}));
    set_safe(&w, None, 0);
    let layout = w.session.view.layout.as_mut().unwrap();
    layout.blocks.push(cargoframe_layout::Block {
        id: "chat".into(),
        rect: cargoframe_layout::Rect::new(0, 4, 6, 1),
        kind: BlockKind::Dropdown(cargoframe_layout::DropdownBlock::new(
            "Chat",
            DropdownSource::ClaudeChats,
        )),
    });
    let mut req = request();
    req.dropdowns.insert("chat".into(), Some(CHAT.into()));
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let seen = w.deliver.seen.lock().unwrap();
    assert!(
        seen[0].url.ends_with(&format!("/chat/{CHAT}")),
        "{}",
        seen[0].url
    );
}

/// The layout of `world`, with a dropdown that goes on to a second list mapped to `where`.
fn with_a_path(w: &mut World) {
    // the connection's body names the two fields
    save(
        &w.f.store,
        "api",
        json!({
            "name": "Hook", "kind": "custom", "base_url": w.server.base(),
            "auth": {"type": "bearer"},
            "send": {"method": "POST", "path": "/hook", "body": {"type": "json",
                "template": "{\"where\":\"{{where}}\",\"inside_field\":\"{{inside_field}}\"}"}},
            "result_url_path": "url"
        }),
    );
    let layout = w.session.view.layout.as_mut().unwrap();
    let mut path = cargoframe_layout::DropdownBlock::new(
        "Where to post",
        DropdownSource::WebApi {
            connection_id: "api".into(),
            options_id: "places".into(),
        },
    );
    path.deeper = Some(cargoframe_layout::Deeper {
        list: "inside".into(),
        field: "inside_field".into(),
    });
    layout.blocks.push(cargoframe_layout::Block {
        id: "where".into(),
        rect: cargoframe_layout::Rect::new(0, 4, 6, 2),
        kind: BlockKind::Dropdown(path),
    });
    let button = layout
        .blocks
        .iter_mut()
        .find_map(|b| match &mut b.kind {
            BlockKind::SharingButton(s) => Some(s),
            _ => None,
        })
        .unwrap();
    button.mappings.push(cargoframe_layout::InfoMapping {
        field: "where".into(),
        source: ValueSource::Block("where".into()),
    });
}

#[test]
fn the_second_step_of_a_path_dropdown_fills_its_own_field_without_a_mapping() {
    let mut w = world(|_| MockResponse::json(201, json!({"url": "https://x.test/p/9"})));
    with_a_path(&mut w);
    set_safe(&w, None, 0);
    let mut req = request();
    req.dropdowns.insert("where".into(), Some("A".into()));
    req.deeper.insert("where".into(), Some("B".into()));
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let body = w.server.requests_to("/hook")[0].json();
    assert_eq!(body["where"], "A");
    assert_eq!(body["inside_field"], "B");
}

#[test]
fn a_path_dropdown_with_nothing_chosen_inside_sends_no_second_field() {
    let mut w = world(|_| MockResponse::json(201, json!({"url": "https://x.test/p/9"})));
    with_a_path(&mut w);
    set_safe(&w, None, 0);
    // This connection's body needs `inside_field`, so a share with nothing inside the first
    // choice is refused by the connection itself ("nothing fills it"): the field is not
    // sent empty, and nothing reaches the server.
    for inside in [None, Some(String::new()), Some("  ".to_string())] {
        let mut req = request();
        req.dropdowns.insert("where".into(), Some("A".into()));
        req.deeper.insert("where".into(), inside);
        let (r, _) = go(&w, &req, vec![]);
        let ShareResult::Failed { message, .. } = r else {
            panic!("{r:?}");
        };
        assert!(message.contains("nothing fills it"), "{message}");
    }
    assert!(w.server.requests_to("/hook").is_empty());
}

#[test]
fn the_window_may_only_ask_for_lists_the_layout_names() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    with_a_path(&mut w);
    let layout = w.session.view.layout.as_ref().unwrap();
    // the dropdown's own list, and the one inside it for a given choice
    assert_eq!(
        options_asked(layout, "where", None),
        Some(("api".into(), "places".into()))
    );
    assert_eq!(
        options_asked(layout, "where", Some(" 123 ")),
        Some(("api".into(), "inside:123".into()))
    );
    // nothing inside an empty choice, a block that is not a dropdown, or one that goes nowhere
    assert_eq!(options_asked(layout, "where", Some("")), None);
    assert_eq!(options_asked(layout, "note", None), None);
    assert_eq!(options_asked(layout, "missing", None), None);
    let plain = cargoframe_layout::DropdownBlock::new(
        "P",
        DropdownSource::WebApi {
            connection_id: "api".into(),
            options_id: "places".into(),
        },
    );
    let mut layout = layout.clone();
    layout.blocks.push(cargoframe_layout::Block {
        id: "plain".into(),
        rect: cargoframe_layout::Rect::new(0, 6, 6, 1),
        kind: BlockKind::Dropdown(plain),
    });
    assert_eq!(options_asked(&layout, "plain", Some("123")), None);
}

#[test]
fn a_share_sends_the_mapped_fields_the_prompt_and_the_secret_only_as_a_header() {
    let w = world(|_| MockResponse::json(201, json!({"url": "https://x.test/p/9"})));
    set_safe(&w, None, 0);
    let (r, asked) = go(&w, &request(), vec![]);
    let ShareResult::Sent {
        summary,
        url,
        close,
        ..
    } = r
    else {
        panic!("{r:?}");
    };
    assert!(summary.contains("201"), "{summary}");
    assert_eq!(url.as_deref(), Some("https://x.test/p/9"));
    assert!(close);
    assert_eq!(asked, 0);
    let sent = w.server.requests_to("/hook");
    assert_eq!(sent.len(), 1);
    let body = sent[0].json();
    assert_eq!(body["summary"], "it \"broke\"");
    assert_eq!(body["build"], "1.2.3");
    assert_eq!(body["kind"], "bug in Editor");
    assert_eq!(body["prompt"], "why?");
    assert_eq!(
        sent[0].header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert!(!String::from_utf8_lossy(&sent[0].body).contains(SECRET));
}

#[test]
fn the_edited_picture_is_sent_only_when_the_window_says_it_was_edited() {
    let w = world(|_| MockResponse::json(200, json!({})));
    set_safe(&w, None, 0);
    // Switch the body to multipart so the picture is part of the request.
    save(
        &w.f.store,
        "api",
        json!({
            "name": "Hook", "kind": "custom", "base_url": w.server.base(),
            "auth": {"type": "bearer"},
            "send": {"method": "POST", "path": "/up", "body": {"type": "multipart",
                "fields": [{"name": "title", "value": "{{title}}"}],
                "image_field": "shot", "files_field": "zip"}}
        }),
    );
    w.f.connections.invalidate("api");
    let edited = [0x89, b'P', b'N', b'G', 9, 9, 9, 9];
    w.session.set_edited(Some(edited.to_vec()));

    let mut req = request();
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    req.image_edited = true;
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");

    let sent = w.server.requests_to("/up");
    assert_eq!(sent.len(), 2);
    let has = |body: &[u8], needle: &[u8]| body.windows(needle.len()).any(|x| x == needle);
    assert!(has(&sent[0].body, PNG) && !has(&sent[0].body, &edited));
    assert!(has(&sent[1].body, &edited) && !has(&sent[1].body, PNG));
    // The ticked file travels as a zip that names it, with the title as a form field.
    assert!(has(&sent[0].body, b"filename=\"files.zip\""));
    assert!(has(&sent[0].body, b"run.log"));
    assert!(!has(&sent[0].body, b"other.log"));
    assert!(has(&sent[0].body, b"Editor"));
}

#[test]
fn blocked_shares_say_why_and_send_nothing() {
    let w = world(|_| MockResponse::json(200, json!({})));
    set_safe(&w, None, 0);

    // a stale capture
    let mut req = request();
    req.session_id = 99;
    let (r, _) = go(&w, &req, vec![]);
    assert!(
        matches!(&r, ShareResult::Blocked { reasons } if reasons[0].contains("no longer open")),
        "{r:?}"
    );

    // a file the capture did not offer (other.log matches *.log but is only offered if
    // scanned; "../../etc/passwd" never is)
    let mut req = request();
    req.files
        .insert("logs".into(), vec!["../../etc/passwd".into()]);
    let (r, _) = go(&w, &req, vec![]);
    assert!(
        matches!(&r, ShareResult::Blocked { reasons } if reasons[0].contains("not one of the files")),
        "{r:?}"
    );

    // a button that is not in the layout
    let mut req = request();
    req.button_id = "nope".into();
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Blocked { .. }));

    // a required field with no value
    let t = cargoframe_config::InfoType {
        name: "Summary".into(),
        connection_id: "api".into(),
        field: "summary".into(),
        required: true,
        ..Default::default()
    };
    w.f.store.save_item(Kind::InfoTypes, "summary", &t).unwrap();
    let mut req = request();
    req.texts.insert("note".into(), "  ".into());
    let (r, _) = go(&w, &req, vec![]);
    assert!(
        matches!(&r, ShareResult::Blocked { reasons } if reasons[0].contains("Summary")),
        "{r:?}"
    );
    // and one that nothing maps
    let t = cargoframe_config::InfoType {
        name: "Owner".into(),
        connection_id: "api".into(),
        field: "owner".into(),
        required: true,
        ..Default::default()
    };
    w.f.store.save_item(Kind::InfoTypes, "owner", &t).unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    assert!(
        matches!(&r, ShareResult::Blocked { reasons } if reasons.iter().any(|x| x.contains("Owner") && x.contains("map it"))),
        "{r:?}"
    );

    assert!(
        w.server.requests().is_empty(),
        "nothing may be sent when blocked"
    );
}

#[test]
fn a_default_template_fills_an_empty_field() {
    let w = world(|_| MockResponse::json(200, json!({})));
    set_safe(&w, None, 0);
    let t = cargoframe_config::InfoType {
        name: "Summary".into(),
        connection_id: "api".into(),
        field: "summary".into(),
        default_template: "Problem in {{app.title}}".into(),
        ..Default::default()
    };
    w.f.store.save_item(Kind::InfoTypes, "summary", &t).unwrap();
    let mut req = request();
    req.texts.insert("note".into(), "".into());
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let body = w.server.requests_to("/hook")[0].json();
    assert_eq!(body["summary"], "Problem in Editor");
}

#[test]
fn a_field_fill_adds_values_when_its_conditions_hold_and_only_in_its_layouts() {
    use cargoframe_config::{Act, Action, FillStep, Test, When};
    let w = world(|_| MockResponse::json(200, json!({})));
    set_safe(&w, None, 0);
    let add = |text: &str| Action {
        act: Act::Add,
        text: text.into(),
    };
    let when = |subject: &str, test, text: &str| {
        Some(When {
            subject: subject.into(),
            test,
            text: text.into(),
        })
    };
    let mut t = cargoframe_config::InfoType {
        name: "Kind".into(),
        connection_id: "api".into(),
        field: "kind".into(),
        kind: InfoKind::MultiChoice,
        steps: vec![
            FillStep {
                when: None,
                then: vec![add("crash")],
            },
            FillStep {
                when: when("{{app.title}}", Test::Found, ""),
                then: vec![add("in-{{app.title}}"), add("crash")],
            },
            FillStep {
                when: when("app.title", Test::Is, "Nope"),
                then: vec![Action {
                    act: Act::Set,
                    text: "never".into(),
                }],
            },
            FillStep {
                when: when("", Test::Contains, "in-editor"),
                then: vec![add("seen")],
            },
        ],
        ..Default::default()
    };
    w.f.store.save_item(Kind::InfoTypes, "kind", &t).unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    // the button's own value first, then each step that holds, a value given once
    let body = w.server.requests_to("/hook")[0].json();
    assert_eq!(body["kind"], "bug in Editor, crash, in-Editor, seen");

    // only in another layout: this share leaves the field alone
    t.layouts = vec!["elsewhere".into()];
    w.f.store.save_item(Kind::InfoTypes, "kind", &t).unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let body = w.server.requests_to("/hook")[1].json();
    assert_eq!(body["kind"], "bug in Editor");
}

#[test]
fn a_tags_block_sends_its_tags_in_its_field_cleaned_and_at_most_twenty_five() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    set_safe(&w, None, 0);
    let layout = w.session.view.layout.as_mut().unwrap();
    layout.blocks.push(cargoframe_layout::Block {
        id: "labels".into(),
        rect: cargoframe_layout::Rect {
            x: 0,
            y: 4,
            w: 6,
            h: 1,
        },
        kind: BlockKind::Tags(cargoframe_layout::TagsBlock::new("Labels", &["crash"])),
    });
    for b in &mut layout.blocks {
        if let BlockKind::SharingButton(btn) = &mut b.kind {
            for m in &mut btn.mappings {
                if m.field == "kind" {
                    m.source = ValueSource::Block("labels".into());
                }
            }
        }
    }
    let mut req = request();
    let mut sent: Vec<String> = vec!["gpu, driver".into(), " crash ".into(), "Crash".into()];
    sent.extend((0..40).map(|n| format!("t{n}")));
    req.tags.insert("labels".into(), sent);
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    let body = w.server.requests_to("/hook")[0].json();
    let kind = body["kind"].as_str().unwrap().to_string();
    let tags: Vec<&str> = kind.split(", ").collect();
    // a comma inside a tag makes two; each once; 25 at most
    assert_eq!(&tags[..4], ["gpu", "driver", "crash", "t0"]);
    assert_eq!(tags.len(), cargoframe_layout::MAX_TAGS);
}

#[test]
fn a_failed_share_retries_then_keeps_a_zip_in_the_safe_folder_without_secrets() {
    let w = world(|_| MockResponse::json(503, json!({"message": "down"})));
    let safe_dir = w.out.path().join("safe");
    fs::create_dir_all(&safe_dir).unwrap();
    set_safe(&w, Some(&safe_dir.display().to_string()), 2);
    let (r, asked) = go(&w, &request(), vec![]);
    let ShareResult::Failed {
        label,
        attempts,
        saved,
        reconnect,
        ..
    } = r
    else {
        panic!("{r:?}");
    };
    assert_eq!(label, "Service error");
    assert_eq!(attempts, 3, "one try and two retries");
    assert!(!reconnect);
    assert_eq!(asked, 0, "the safe folder worked, nobody is asked");
    assert_eq!(w.server.requests_to("/hook").len(), 3);

    let saved = saved.unwrap();
    assert_eq!(saved.place, "safe_folder");
    assert!(
        saved.path.starts_with(&safe_dir.display().to_string()),
        "{}",
        saved.path
    );
    assert!(
        saved.path.contains("CargoFrame_2026-10-23_080001_hook"),
        "{}",
        saved.path
    );
    let names = zip_names(&saved.path);
    for want in [
        "screenshot.png",
        "share.json",
        "error.json",
        "files/run.log",
        "manifest.json",
    ] {
        assert!(names.contains(&want.to_string()), "{names:?}");
    }
    assert!(!names.iter().any(|n| n.contains("other.log")));
    let share = zip_text(&saved.path, "share.json");
    assert!(
        share.contains("it \\\"broke\\\"") && share.contains("why?"),
        "{share}"
    );
    assert!(zip_text(&saved.path, "error.json").contains("Service error"));
    // The sign-in secret is nowhere in the kept copy, and no half-written file remains.
    let raw = fs::read(&saved.path).unwrap();
    assert!(!raw.windows(SECRET.len()).any(|x| x == SECRET.as_bytes()));
    let left: Vec<_> = fs::read_dir(&safe_dir).unwrap().flatten().collect();
    assert_eq!(left.len(), 1);
    // the temporary zip was cleaned up
    assert_eq!(
        fs::read_dir(w.f.store.dirs().data().join("tmp"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn a_rejected_sign_in_is_not_retried_and_offers_to_reconnect() {
    let w = world(|_| MockResponse::json(401, json!({"message": "bad token"})));
    let safe_dir = w.out.path().join("safe");
    fs::create_dir_all(&safe_dir).unwrap();
    set_safe(&w, Some(&safe_dir.display().to_string()), 3);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Failed {
        reconnect,
        attempts,
        label,
        ..
    } = r
    else {
        panic!("{r:?}");
    };
    assert!(reconnect);
    assert_eq!(attempts, 1);
    assert_eq!(label, "Sign-in expired");
    let info = w.f.connections.info(&w.f.store, "api").unwrap();
    assert_eq!(info.state, cargoframe_layout::ConnectionState::Expired);
}

#[test]
fn a_503_then_a_200_succeeds_after_one_retry() {
    let calls = std::sync::atomic::AtomicU32::new(0);
    let w = world(move |_| {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            MockResponse::new(503)
        } else {
            MockResponse::json(200, json!({}))
        }
    });
    set_safe(&w, None, 3);
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
    assert_eq!(w.server.requests_to("/hook").len(), 2);
}

#[test]
fn with_no_safe_folder_the_person_is_asked_and_the_choice_can_be_remembered() {
    let w = world(|_| MockResponse::new(400));
    set_safe(&w, None, 0);
    let chosen = w.out.path().join("chosen");
    fs::create_dir_all(&chosen).unwrap();
    let pick = PickedFolder {
        dir: chosen.display().to_string(),
        remember: true,
    };
    let (r, asked) = go(&w, &request(), vec![Some(pick)]);
    let ShareResult::Failed { saved, label, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(label, "Rejected by the service");
    assert_eq!(asked, 1);
    let saved = saved.unwrap();
    assert_eq!(saved.place, "picked_folder");
    assert!(Path::new(&saved.path).starts_with(&chosen));
    let safe = w.f.store.load_or_default::<Safe>().unwrap();
    assert_eq!(
        safe.folder.as_deref(),
        Some(chosen.display().to_string().as_str())
    );
}

#[test]
fn when_nothing_else_works_the_zip_is_kept_in_the_data_folder() {
    let w = world(|_| MockResponse::new(400));
    let missing = w.out.path().join("not-there");
    set_safe(&w, Some(&missing.display().to_string()), 0);
    let (r, asked) = go(&w, &request(), vec![None, None, None]);
    let ShareResult::Failed { saved, .. } = r else {
        panic!("{r:?}");
    };
    assert!(asked >= 1);
    let saved = saved.unwrap();
    assert_eq!(saved.place, "kept_locally");
    assert!(Path::new(&saved.path).starts_with(w.f.store.dirs().data().join("kept")));
    assert!(saved.note.unwrap().contains("safe folder"));
    assert!(Path::new(&saved.path).is_file());
}

fn disconnect(w: &World) {
    w.f.connections
        .set_secret(&w.f.store, "api", "secret", "")
        .unwrap();
}

/// Every entry of the zip as text (the picture too, lossily): for "this appears nowhere".
fn all_zip_text(path: &str) -> String {
    let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut out = String::new();
    for i in 0..z.len() {
        let mut bytes = Vec::new();
        z.by_index(i).unwrap().read_to_end(&mut bytes).unwrap();
        out.push_str(&String::from_utf8_lossy(&bytes));
        out.push('\n');
    }
    out
}

#[test]
fn a_connection_that_is_not_signed_in_saves_a_zip_and_sends_nothing() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let (r, asked) = go(&w, &request(), vec![]);
    let ShareResult::SavedLocally { path, close, .. } = r else {
        panic!("{r:?}");
    };
    assert!(
        close,
        "the layout's close-after-sharing setting still applies"
    );
    assert_eq!(
        asked, 0,
        "the person is never asked where: the setting says"
    );
    assert!(Path::new(&path).starts_with(&dir), "{path}");
    assert!(path.ends_with("_local.zip"), "{path}");
    let mut names = zip_names(&path);
    names.sort();
    assert!(names.contains(&"screenshot.png".to_string()), "{names:?}");
    assert!(names.contains(&"share.json".to_string()));
    assert!(names.contains(&"files/run.log".to_string()));
    assert!(
        !names.contains(&"files/other.log".to_string()),
        "only ticked files"
    );
    assert!(!names.iter().any(|n| n == "error.json"), "nothing failed");
    let share: serde_json::Value = serde_json::from_str(&zip_text(&path, "share.json")).unwrap();
    assert_eq!(share["destination"], "local");
    assert_eq!(share["prompt"], "why?");
    assert_eq!(share["fields"][0]["value"], "it \"broke\"");
    assert!(
        w.server.requests().is_empty(),
        "nothing leaves the computer"
    );
    assert!(!all_zip_text(&path).contains(SECRET));
    // No leftovers next to the zip.
    let files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(files.len(), 1);
}

/// Turn the capture of a test world into a video capture whose recording holds `bytes`.
fn make_video(w: &mut World, bytes: &[u8]) {
    let path = w.out.path().join("recording.mp4");
    fs::write(&path, bytes).unwrap();
    let view = w.session.view.clone();
    w.session = std::mem::replace(&mut w.session, Session::new(Vec::new(), view)).with_clip(
        crate::session::Clip {
            path,
            width: 1280,
            height: 720,
            duration_ms: 9_000,
            bytes: bytes.len() as u64,
            has_audio: false,
        },
    );
}

#[test]
fn a_video_capture_saved_locally_keeps_the_recording_where_the_picture_would_be() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"MP4 BYTES");
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let names = zip_names(&path);
    assert!(names.contains(&"recording.mp4".to_string()), "{names:?}");
    assert!(!names.contains(&"screenshot.png".to_string()), "{names:?}");
    assert!(zip_text(&path, "recording.mp4").starts_with("MP4 BYTES"));
    assert!(zip_text(&path, "manifest.json").contains("\"video\""));
}

#[test]
fn a_video_capture_leaves_the_recording_out_when_the_button_leaves_the_picture_out() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"MP4 BYTES");
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    set_rules(
        &mut w,
        SendRules {
            screenshot: false,
            ..SendRules::default()
        },
    );
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let names = zip_names(&path);
    assert!(!names.contains(&"recording.mp4".to_string()), "{names:?}");
    assert!(!names.contains(&"screenshot.png".to_string()), "{names:?}");
}

/// A recording capture saved locally, in a layout that has a screenshot block and a video block.
fn recording_with_a_picture_block(w: &mut World) -> PathBuf {
    make_video(w, b"MP4 BYTES");
    let layout = w.session.view.layout.as_mut().unwrap();
    for (id, kind, x) in [
        ("shot", json!({"type": "screenshot"}), 0),
        ("clip", json!({"type": "video"}), 6),
    ] {
        let mut block = json!({"id": id, "rect": {"x": x, "y": 4, "w": 6, "h": 4}});
        block
            .as_object_mut()
            .unwrap()
            .extend(kind.as_object().unwrap().clone());
        layout.blocks.push(serde_json::from_value(block).unwrap());
    }
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(w, Some(&dir.display().to_string()), 0);
    disconnect(w);
    dir
}

#[test]
fn a_picture_taken_from_the_recording_goes_with_it() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    recording_with_a_picture_block(&mut w);
    let mut req = request();
    req.still = true;
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let names = zip_names(&path);
    assert!(names.contains(&"recording.mp4".to_string()), "{names:?}");
    assert!(names.contains(&"screenshot.png".to_string()), "{names:?}");
}

#[test]
fn an_empty_screenshot_block_sends_only_the_recording() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    recording_with_a_picture_block(&mut w);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let names = zip_names(&path);
    assert!(names.contains(&"recording.mp4".to_string()), "{names:?}");
    assert!(!names.contains(&"screenshot.png".to_string()), "{names:?}");
}

#[test]
fn each_picture_block_goes_by_its_own_box() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    recording_with_a_picture_block(&mut w);
    let names_for = |w: &World, ticked: &[&str]| {
        let mut req = request();
        req.still = true;
        req.send_blocks = Some(ticked.iter().map(|b| b.to_string()).collect());
        let (r, _) = go(w, &req, vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        zip_names(&path)
    };
    let only_the_picture = names_for(&w, &["shot"]);
    assert!(
        only_the_picture.contains(&"screenshot.png".to_string()),
        "{only_the_picture:?}"
    );
    assert!(
        !only_the_picture.contains(&"recording.mp4".to_string()),
        "{only_the_picture:?}"
    );
    let only_the_recording = names_for(&w, &["clip"]);
    assert!(
        only_the_recording.contains(&"recording.mp4".to_string()),
        "{only_the_recording:?}"
    );
    assert!(
        !only_the_recording.contains(&"screenshot.png".to_string()),
        "{only_the_recording:?}"
    );
}

#[test]
fn a_video_is_not_sent_to_a_destination_that_cannot_take_one_and_the_reason_is_given() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"MP4 BYTES");
    set_safe(&w, None, 0);
    // the test world's own destination is a Custom API connection
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert!(
        reasons.iter().any(|m| m.contains("cannot take a video")),
        "{reasons:?}"
    );
    assert!(w.server.requests().is_empty(), "nothing was sent");

    use_claude(&mut w, json!({}));
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|m| m.contains("Claude cannot watch a video")),
        "{reasons:?}"
    );
    assert!(w.deliver.seen.lock().unwrap().is_empty());
}

/// A real clip `secs` seconds long, or `None` when ffmpeg is not installed here.
fn real_clip(path: &Path, secs: u32) -> Option<()> {
    let made = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error"])
        .args(["-f", "lavfi", "-i"])
        .arg(format!("testsrc=duration={secs}:size=320x240:rate=15"))
        .args(["-pix_fmt", "yuv420p", "-g", "15"])
        .arg(path)
        .status()
        .ok()?;
    made.success().then_some(())
}

/// How long a clip is, in seconds, as ffprobe says (`None` when ffprobe is not installed).
fn seconds_of(path: &Path) -> Option<f64> {
    let out = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration"])
        .args(["-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[test]
fn a_video_capture_cut_in_the_window_sends_only_the_part_that_was_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source.mp4");
    if real_clip(&source, 6).is_none() || seconds_of(&source).is_none() {
        eprintln!("skipped: ffmpeg or ffprobe is not installed");
        return;
    }
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"placeholder");
    let mut clip = w.session.clip.clone().unwrap();
    clip.path = source;
    clip.duration_ms = 6_000;
    w.session.clip = Some(clip);
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let mut req = request();
    req.clip_edit = Some(crate::video::ClipEdit {
        start_ms: 1_000,
        end_ms: Some(3_000),
        crop: None,
        mark: false,
        ..Default::default()
    });
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let mut z = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    z.by_name("recording.mp4")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let sent = tmp.path().join("sent.mp4");
    fs::write(&sent, &bytes).unwrap();
    let length = seconds_of(&sent).expect("the zip holds a video");
    assert!(
        (1.5..2.8).contains(&length),
        "2 seconds were kept of 6, the zip holds {length} s"
    );
    // the cut is made in the capture's own folder; the original is not touched
    assert!(
        seconds_of(&w.session.clip.as_ref().unwrap().path).unwrap() > 5.5,
        "the recording is as it was"
    );
}

#[test]
fn a_cut_that_keeps_everything_sends_the_recording_as_it_is() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"MP4 BYTES");
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    for trim in [
        crate::video::ClipEdit {
            start_ms: 0,
            end_ms: None,
            crop: None,
            mark: false,
            ..Default::default()
        },
        crate::video::ClipEdit {
            start_ms: 0,
            end_ms: Some(9_000),
            crop: None,
            mark: false,
            ..Default::default()
        },
    ] {
        let mut req = request();
        req.clip_edit = Some(trim);
        let (r, _) = go(&w, &req, vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        // no ffmpeg is needed, and nothing is changed
        assert!(zip_text(&path, "recording.mp4").starts_with("MP4 BYTES"));
    }
}

#[test]
fn a_cut_that_cannot_be_made_stops_the_share_and_says_why() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    // not a video: the cut cannot be made, with or without ffmpeg on this computer
    make_video(&mut w, b"MP4 BYTES");
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let mut req = request();
    req.clip_edit = Some(crate::video::ClipEdit {
        start_ms: 1_000,
        end_ms: Some(2_000),
        crop: None,
        mark: false,
        ..Default::default()
    });
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(
        reasons[0].contains("recording") || reasons[0].contains("ffmpeg"),
        "{reasons:?}"
    );
    assert!(
        fs::read_dir(&dir).unwrap().next().is_none(),
        "no zip was made"
    );
}

#[test]
fn changing_nothing_is_told_from_cutting_or_cropping_something() {
    use crate::video::ClipEdit;
    use cargoframe_video::Crop;
    let t = |start_ms, end_ms| ClipEdit {
        start_ms,
        end_ms,
        crop: None,
        mark: false,
        ..Default::default()
    };
    assert!(t(0, None).changes_nothing(5_000));
    assert!(t(0, Some(5_000)).changes_nothing(5_000));
    assert!(t(0, Some(6_000)).changes_nothing(5_000));
    assert!(!t(100, None).changes_nothing(5_000));
    assert!(!t(0, Some(4_900)).changes_nothing(5_000));
    let cropped = ClipEdit {
        crop: Some(Crop {
            x: 0,
            y: 0,
            w: 64,
            h: 64,
        }),
        ..t(0, None)
    };
    assert!(!cropped.changes_nothing(5_000));
    let marked = ClipEdit {
        mark: true,
        ..t(0, None)
    };
    assert!(!marked.changes_nothing(5_000));
    // keys shown need an export; a crop told to follow without a crop changes nothing
    let keyed = ClipEdit {
        keys: Some(crate::video::KeysAt { x: 0, y: 0 }),
        ..t(0, None)
    };
    assert!(!keyed.changes_nothing(5_000));
    let follow = ClipEdit {
        follow: true,
        ..t(0, None)
    };
    assert!(follow.changes_nothing(5_000));
}

#[test]
fn a_video_capture_cropped_in_the_window_sends_the_cropped_picture_for_the_whole_time() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source.mp4");
    if real_clip(&source, 4).is_none() || seconds_of(&source).is_none() {
        eprintln!("skipped: ffmpeg or ffprobe is not installed");
        return;
    }
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"placeholder");
    let mut clip = w.session.clip.clone().unwrap();
    clip.path = source;
    clip.duration_ms = 4_000;
    w.session.clip = Some(clip);
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let mut req = request();
    // odd numbers, as a mouse gives them: the picture still comes out even
    req.clip_edit = Some(crate::video::ClipEdit {
        start_ms: 0,
        end_ms: None,
        crop: Some(cargoframe_video::Crop {
            x: 41,
            y: 21,
            w: 161,
            h: 101,
        }),
        mark: false,
        ..Default::default()
    });
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let mut z = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    z.by_name("recording.mp4")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let sent = tmp.path().join("sent.mp4");
    fs::write(&sent, &bytes).unwrap();
    let probed = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=width,height"])
        .args(["-of", "csv=s=x:p=0"])
        .arg(&sent)
        .output()
        .unwrap();
    let size = String::from_utf8_lossy(&probed.stdout).trim().to_string();
    assert_eq!(size, "160x100", "the crop, made even");
    let length = seconds_of(&sent).unwrap();
    assert!(length > 3.0, "the whole time is kept: {length} s");
}

/// The first picture of a video as raw RGB, with its width, via ffmpeg.
fn first_frame(path: &Path, width: usize) -> Option<Vec<u8>> {
    let out = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .ok()?;
    (out.stdout.len() >= width * 3).then_some(out.stdout)
}

#[test]
fn a_video_capture_with_a_mark_sends_the_recording_with_the_mark_on_it() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source.mp4");
    if real_clip(&source, 3).is_none() || seconds_of(&source).is_none() {
        eprintln!("skipped: ffmpeg or ffprobe is not installed");
        return;
    }
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"placeholder");
    let mut clip = w.session.clip.clone().unwrap();
    clip.path = source;
    clip.duration_ms = 3_000;
    w.session.clip = Some(clip);
    // the mark the window draws: see-through, with a red square in the top-left corner
    let (mw, mh) = (320u32, 240u32);
    let mut rgba = vec![0u8; (mw * mh * 4) as usize];
    for y in 0..80u32 {
        for x in 0..80u32 {
            let i = ((y * mw + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
    }
    let png = cargoframe_capture::Frame::new(mw, mh, rgba)
        .unwrap()
        .to_png()
        .unwrap();
    w.session.set_clip_mark(Some(png));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let mut req = request();
    req.clip_edit = Some(crate::video::ClipEdit {
        start_ms: 0,
        end_ms: None,
        crop: None,
        mark: true,
        ..Default::default()
    });
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let mut z = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    z.by_name("recording.mp4")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let sent = tmp.path().join("sent.mp4");
    fs::write(&sent, &bytes).unwrap();
    let marked = first_frame(&sent, mw as usize).expect("a picture");
    let at = |x: usize, y: usize| {
        let i = (y * mw as usize + x) * 3;
        (marked[i], marked[i + 1], marked[i + 2])
    };
    let (r, g, b) = at(20, 20);
    assert!(
        r > 180 && g < 90 && b < 90,
        "the corner of the picture is red where the mark is: {:?}",
        at(20, 20)
    );
    // the same clip without the mark is not red there
    let plain = first_frame(&w.session.clip.as_ref().unwrap().path, mw as usize).unwrap();
    let i = (20 * mw as usize + 20) * 3;
    assert!(
        !(plain[i] > 180 && plain[i + 1] < 90 && plain[i + 2] < 90),
        "the recording itself is not red there"
    );
}

#[test]
fn a_video_capture_shows_the_keys_pressed_where_the_window_placed_them() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source.mp4");
    if real_clip(&source, 3).is_none() || seconds_of(&source).is_none() {
        eprintln!("skipped: ffmpeg or ffprobe is not installed");
        return;
    }
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"placeholder");
    let mut clip = w.session.clip.clone().unwrap();
    clip.path = source;
    clip.duration_ms = 3_000;
    w.session.clip = Some(clip);
    w.session.view.video.as_mut().unwrap().keys = vec![cargoframe_window::view::KeyLine {
        from_ms: 0,
        to_ms: 2_000,
        text: "Ctrl+S".into(),
    }];
    // what the window draws: a blank picture and a green one for the line, the same size
    let picture = |rgba: [u8; 4]| {
        cargoframe_capture::Frame::new(40, 20, rgba.repeat(40 * 20))
            .unwrap()
            .to_png()
            .unwrap()
    };
    let body = |pics: &[Vec<u8>]| {
        let mut b = Vec::new();
        for p in pics {
            b.extend((p.len() as u32).to_be_bytes());
            b.extend(p);
        }
        b
    };
    let pictures = vec![picture([0, 0, 0, 0]), picture([0, 255, 0, 255])];
    assert_eq!(
        crate::commands::split_frames(&body(&pictures)).unwrap(),
        pictures
    );
    w.session.set_clip_keys(Some(pictures));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let mut req = request();
    req.clip_edit = Some(crate::video::ClipEdit {
        keys: Some(crate::video::KeysAt { x: 10, y: 30 }),
        ..Default::default()
    });
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    let mut z = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    z.by_name("recording.mp4")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let sent = tmp.path().join("sent.mp4");
    fs::write(&sent, &bytes).unwrap();
    let frame = first_frame(&sent, 320).expect("a picture");
    let at = |x: usize, y: usize| {
        let i = (y * 320 + x) * 3;
        (frame[i], frame[i + 1], frame[i + 2])
    };
    let (r, g, b) = at(25, 40);
    assert!(
        g > 180 && r < 90 && b < 90,
        "the keys are drawn there: {:?}",
        at(25, 40)
    );
    // without the pictures, the share stops and says why
    w.session.set_clip_keys(None);
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert_eq!(reasons, ["The keys pressed were not drawn: try again"]);
}

#[test]
fn a_mark_that_was_not_uploaded_stops_the_share_and_says_so() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"MP4 BYTES");
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let mut req = request();
    req.clip_edit = Some(crate::video::ClipEdit {
        start_ms: 0,
        end_ms: None,
        crop: None,
        mark: true,
        ..Default::default()
    });
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert_eq!(reasons, ["The mark was not drawn: try again"]);
    assert!(
        fs::read_dir(&dir).unwrap().next().is_none(),
        "no zip was made"
    );
}

#[test]
fn a_recording_over_the_limit_is_refused_before_anything_is_read() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    make_video(&mut w, b"MP4 BYTES");
    // a sparse file: the size is what counts, nothing is written
    let big = w.out.path().join("big.mp4");
    let f = fs::File::create(&big).unwrap();
    f.set_len(MAX_VIDEO_BYTES + 1).unwrap();
    drop(f);
    let mut clip = w.session.clip.clone().unwrap();
    clip.path = big;
    w.session.clip = Some(clip);
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert!(reasons[0].contains("record a shorter one"), "{reasons:?}");
}

#[test]
fn a_window_with_nowhere_to_post_saves_a_zip_instead_of_sending() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    let mut req = request();
    req.save_here = true;
    let (r, _) = go(&w, &req, vec![]);
    assert!(matches!(r, ShareResult::SavedLocally { .. }), "{r:?}");
    assert!(
        w.server.requests().is_empty(),
        "nothing is sent to the service"
    );
}

#[test]
fn a_button_with_no_connection_chosen_saves_a_zip_too() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    if let Some(Some(BlockKind::SharingButton(b))) = w.session.view.layout.as_mut().map(|l| {
        l.blocks
            .iter_mut()
            .map(|b| &mut b.kind)
            .find(|k| matches!(k, BlockKind::SharingButton(_)))
    }) {
        b.connection_id = None;
    }
    let (r, _) = go(&w, &request(), vec![]);
    assert!(matches!(r, ShareResult::SavedLocally { .. }), "{r:?}");
    assert!(w.server.requests().is_empty());
}

#[test]
fn saving_locally_needs_the_folder_for_saved_zips() {
    let w = world(|_| MockResponse::json(200, json!({})));
    disconnect(&w);
    for folder in [None, Some("   ")] {
        set_safe(&w, folder, 0);
        let (r, asked) = go(
            &w,
            &request(),
            vec![Some(PickedFolder {
                dir: w.out.path().display().to_string(),
                remember: true,
            })],
        );
        assert_eq!(
            r,
            ShareResult::Blocked {
                reasons: vec![NEEDS_SAVE_FOLDER.to_string()]
            }
        );
        assert_eq!(asked, 0, "no folder picker for a local save");
    }
    assert!(!w.f.store.dirs().data().join("kept").exists());
    assert!(w.server.requests().is_empty());
}

#[test]
fn a_folder_that_is_gone_is_reported_not_created_or_replaced() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let missing = w.out.path().join("not-there");
    set_safe(&w, Some(&missing.display().to_string()), 0);
    disconnect(&w);
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Blocked { reasons } = r else {
        panic!("{r:?}");
    };
    assert!(
        reasons[0].contains("not-there") && reasons[0].contains("missing"),
        "{reasons:?}"
    );
    assert!(!missing.exists());
    assert!(!w.f.store.dirs().data().join("kept").exists());
}

/// Adds a Zip name block to the capture, holding `typed` (its value in the window).
fn add_zip_name(w: &mut World, typed: &str) -> String {
    let layout = w.session.view.layout.as_mut().unwrap();
    layout.blocks.push(
        serde_json::from_value(json!({
            "id": "zipname", "rect": {"x":0,"y":4,"w":6,"h":1}, "type": "text",
            "title": "Zip name", "role": "zip_name", "default_text": ""
        }))
        .unwrap(),
    );
    typed.to_string()
}

fn request_with_zip_name(typed: &str) -> Request {
    let mut req = request();
    req.texts.insert("zipname".into(), typed.into());
    req
}

#[test]
fn the_zip_name_block_names_a_zip_saved_locally() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let typed = add_zip_name(&mut w, "Bug in login");
    let (r, _) = go(&w, &request_with_zip_name(&typed), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(Path::new(&path), dir.join("Bug in login.zip"));
    // The same name twice keeps both.
    let (r, _) = go(&w, &request_with_zip_name(&typed), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(Path::new(&path), dir.join("Bug in login-2.zip"));
}

#[test]
fn an_empty_or_unusable_zip_name_keeps_the_default_name() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    add_zip_name(&mut w, "");
    for typed in ["", "   ", "..."] {
        let (r, _) = go(&w, &request_with_zip_name(typed), vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        assert!(
            path.contains("CargoFrame_2026-10-23_080001_local"),
            "{path}"
        );
    }
}

#[test]
fn a_zip_name_cannot_write_outside_the_folder() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    add_zip_name(&mut w, "");
    let (r, _) = go(&w, &request_with_zip_name("../../escape"), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(Path::new(&path).parent(), Some(dir.as_path()), "{path}");
    assert_eq!(Path::new(&path).file_name().unwrap(), "escape.zip");
}

#[test]
fn only_the_first_zip_name_block_counts() {
    let mut w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    add_zip_name(&mut w, "");
    w.session.view.layout.as_mut().unwrap().blocks.push(
        serde_json::from_value(json!({
            "id": "zipname2", "rect": {"x":6,"y":4,"w":6,"h":1}, "type": "text",
            "title": "Other", "role": "zip_name", "default_text": ""
        }))
        .unwrap(),
    );
    let mut req = request_with_zip_name("first");
    req.texts.insert("zipname2".into(), "second".into());
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(Path::new(&path).file_name().unwrap(), "first.zip");
}

#[test]
fn a_failed_share_keeps_its_copy_under_the_typed_zip_name() {
    let mut w = world(|_| MockResponse::json(503, json!({"message": "down"})));
    let safe_dir = w.out.path().join("safe");
    fs::create_dir_all(&safe_dir).unwrap();
    set_safe(&w, Some(&safe_dir.display().to_string()), 0);
    add_zip_name(&mut w, "");
    let (r, _) = go(&w, &request_with_zip_name("Crash on save"), vec![]);
    let ShareResult::Failed { saved, .. } = r else {
        panic!("{r:?}");
    };
    let saved = saved.unwrap();
    assert_eq!(saved.place, "safe_folder");
    assert_eq!(
        Path::new(&saved.path).file_name().unwrap(),
        "Crash on save.zip"
    );
}

/// The project folder of the world's capture.
fn project_of(w: &World) -> PathBuf {
    PathBuf::from(w.session.files_root.clone().unwrap())
}

#[test]
fn a_saved_zip_holds_the_files_as_they_were_at_the_capture() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    // The capture copies what is ticked by default, then the log keeps growing.
    assert_eq!(
        w.session.stage_preselected(
            &w.ws,
            MAX_STAGED_FILE_BYTES,
            MAX_STAGED_TOTAL_BYTES,
            &|| true
        ),
        1
    );
    fs::write(project_of(&w).join("run.log"), "LOGLINE and then some").unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(zip_text(&path, "files/run.log"), "LOGLINE");
}

#[test]
fn a_file_added_by_hand_is_copied_at_once_listed_and_sent_and_a_log_viewer_reads_offered_files() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let elsewhere = w.out.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let picked = elsewhere.join("notes.txt");
    fs::write(&picked, "first\nsecond").unwrap();
    let e = w.session.add_file(&w.ws, "logs", &picked).unwrap();
    assert_eq!(
        (e.path.as_str(), e.size, e.selected),
        ("added/notes.txt", 12, true)
    );
    // the same name again gets a number; a block that is not there is refused
    let again = w.session.add_file(&w.ws, "logs", &picked).unwrap();
    assert_eq!(again.path, "added/notes (2).txt");
    assert!(w.session.add_file(&w.ws, "nope", &picked).is_err());
    assert!(w.session.add_file(&w.ws, "logs", &elsewhere).is_err());
    // changed or gone after it was added: the copy is what goes
    fs::write(&picked, "changed").unwrap();
    let mut req = request();
    req.files.insert(
        "logs".into(),
        vec!["run.log".into(), "added/notes.txt".into()],
    );
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(zip_text(&path, "files/added/notes.txt"), "first\nsecond");
    assert_eq!(zip_text(&path, "files/run.log"), "LOGLINE");

    // a log viewer reads what the capture offers, and nothing else
    let log = w.session.read_offered("run.log").unwrap();
    assert_eq!((log.text.as_str(), log.cut), ("LOGLINE", false));
    assert_eq!(
        w.session.read_offered("added/notes.txt").unwrap().text,
        "first\nsecond"
    );
    assert!(w.session.read_offered("../secret.txt").is_err());
    assert!(w.session.read_offered("missing.log").is_err());
}

#[test]
fn the_end_of_a_large_file_is_read_from_its_first_whole_line() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("big.log");
    fs::write(&f, "aaaa\nbbbb\ncccc\n").unwrap();
    let t = crate::session::read_end(&f, 8).unwrap();
    assert_eq!((t.text.as_str(), t.cut, t.size), ("cccc\n", true, 15));
    assert!(!crate::session::read_end(&f, 100).unwrap().cut);
}

#[test]
fn a_file_ticked_later_is_copied_when_the_button_is_pressed() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    w.session.stage_preselected(
        &w.ws,
        MAX_STAGED_FILE_BYTES,
        MAX_STAGED_TOTAL_BYTES,
        &|| true,
    );
    let mut req = request();
    req.files
        .insert("logs".into(), vec!["run.log".into(), "other.log".into()]);
    let (r, _) = go(&w, &req, vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(zip_text(&path, "files/run.log"), "LOGLINE");
    assert_eq!(zip_text(&path, "files/other.log"), "OTHER");
}

#[test]
fn nothing_copied_ahead_still_zips_the_current_file() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    fs::write(project_of(&w).join("run.log"), "NEWER").unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::SavedLocally { path, .. } = r else {
        panic!("{r:?}");
    };
    // The share copies it when the button is pressed, so it is that moment's content.
    assert_eq!(zip_text(&path, "files/run.log"), "NEWER");
}

#[test]
fn a_zip_kept_after_a_failed_send_holds_the_capture_time_files() {
    let w = world(|_| MockResponse::json(503, json!({"message": "down"})));
    let safe_dir = w.out.path().join("safe");
    fs::create_dir_all(&safe_dir).unwrap();
    set_safe(&w, Some(&safe_dir.display().to_string()), 0);
    w.session.stage_preselected(
        &w.ws,
        MAX_STAGED_FILE_BYTES,
        MAX_STAGED_TOTAL_BYTES,
        &|| true,
    );
    fs::write(project_of(&w).join("run.log"), "LOGLINE and then some").unwrap();
    let (r, _) = go(&w, &request(), vec![]);
    let ShareResult::Failed { saved, .. } = r else {
        panic!("{r:?}");
    };
    assert_eq!(zip_text(&saved.unwrap().path, "files/run.log"), "LOGLINE");
}

#[test]
fn a_share_waits_for_the_copying_that_started_with_the_capture() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let started = std::time::Instant::now();
    let waited = std::thread::scope(|scope| {
        let holder = scope.spawn(|| {
            // Stands in for a slow copy: holds the capture's folder for a while.
            w.session
                .with_dir(&w.ws, |_| std::thread::sleep(Duration::from_millis(400)))
                .unwrap();
        });
        std::thread::sleep(Duration::from_millis(100));
        let (r, _) = go(&w, &request(), vec![]);
        assert!(matches!(r, ShareResult::SavedLocally { .. }), "{r:?}");
        let waited = started.elapsed();
        holder.join().unwrap();
        waited
    });
    assert!(
        waited >= Duration::from_millis(350),
        "the share did not wait: {waited:?}"
    );
}

#[test]
fn what_is_copied_ahead_is_capped_like_a_zip() {
    assert_eq!(
        MAX_STAGED_TOTAL_BYTES,
        cargoframe_bundle::Limits::default().max_total_bytes
    );
}

#[test]
fn two_local_saves_in_the_same_second_keep_both() {
    let w = world(|_| MockResponse::json(200, json!({})));
    let dir = w.out.path().join("zips");
    fs::create_dir_all(&dir).unwrap();
    set_safe(&w, Some(&dir.display().to_string()), 0);
    disconnect(&w);
    let (a, _) = go(&w, &request(), vec![]);
    let (b, _) = go(&w, &request(), vec![]);
    let (ShareResult::SavedLocally { path: a, .. }, ShareResult::SavedLocally { path: b, .. }) =
        (a, b)
    else {
        panic!("both saved");
    };
    assert_ne!(a, b);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
}
