use super::*;
use cargoframe_config::Dirs;
use cargoframe_layout::{Block, BlockKind, FilesBlock, Rect, TextBlock};

pub fn no_connections() -> Connections {
    Connections::new(
        std::sync::Arc::new(cargoframe_net::UreqHttp::new()),
        std::sync::Arc::new(cargoframe_connectors::MemorySecrets::new()),
    )
}

pub fn fake_capture(exe: Option<&str>) -> cap::Captured {
    cap::Captured {
        frame: cap::Frame::new(4, 3, vec![255; 4 * 3 * 4]).unwrap(),
        app: exe.map(|p| cap::FocusedApp {
            window: 1,
            exe_path: Some(p.into()),
            title: "Editor".into(),
            pid: Some(1),
            bounds: cap::PixelRect::new(0, 0, 4, 3),
        }),
        subject: cap::Subject::Window,
        notes: vec![],
    }
}

pub const NOW: Moment = Moment {
    year: 2026,
    month: 10,
    day: 23,
    hour: 8,
    minute: 0,
    second: 1,
};

#[test]
fn a_video_session_keeps_the_recording_in_its_own_folder_and_tells_the_window_about_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "Clip".into(),
        blocks: vec![Block {
            id: "clip".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 8,
                h: 8,
            },
            kind: BlockKind::Video(Default::default()),
        }],
    };
    store.save_item(Kind::Layouts, "clip", &layout).unwrap();
    let trigger = Trigger {
        name: "Clip".into(),
        media: cargoframe_config::Media::Video,
        layout_id: Some("clip".into()),
        ..Trigger::default()
    };
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let source = work.join("test-clip.mp4");
    std::fs::write(&source, b"not really a video").unwrap();
    let workspace = Workspace::new(dir.path().join("captures"));

    let s = prepare_video(
        &store,
        &no_connections(),
        &workspace,
        "cliptrig",
        &trigger,
        fake_capture(None),
        NOW,
        3,
        &source,
        ClipInfo {
            width: 1280,
            height: 720,
            duration_ms: 9_500,
            bytes: 18,
            has_audio: false,
            ends_before_ms: 500,
        },
    )
    .unwrap();

    // the capture is dated now, and the clip started its length (and the part not handed out
    // yet) before
    let now = crate::session::unix_ms();
    assert!(s.view.captured_at_ms <= now && now - s.view.captured_at_ms < 60_000);
    assert_eq!(
        s.view.video.as_ref().unwrap().started_at_ms,
        Some(s.view.captured_at_ms - 10_000)
    );
    let clip = s.clip.clone().expect("the session has the recording");
    assert_eq!(clip.path.file_name().unwrap(), CLIP_NAME);
    assert_eq!(std::fs::read(&clip.path).unwrap(), b"not really a video");
    assert!(!source.exists(), "the recording moved, it was not copied");
    // inside the capture's own folder, so it goes when the capture is closed
    assert_eq!(clip.path.parent().map(Path::to_path_buf), s.dir_path());
    let video = s.view.video.clone().expect("the window is told");
    assert_eq!(
        (video.width, video.height, video.duration_ms),
        (1280, 720, 9_500)
    );
    let json = serde_json::to_value(&s.view).unwrap();
    assert_eq!(json["video"]["duration_ms"], 9_500);
    assert!(json["video"]["started_at_ms"].as_u64().is_some());
    assert!(json["captured_at_ms"].as_u64().unwrap() > 0);
    assert_eq!(json["layout"]["blocks"][0]["type"], "video");

    let folder = s.dir_path().unwrap();
    drop(s);
    assert!(
        !folder.exists(),
        "closing the capture removes the recording"
    );
}

#[test]
fn a_screenshot_session_has_no_video() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    let s = prepare_with(
        &store,
        &no_connections(),
        "t",
        &Trigger::default(),
        fake_capture(None),
        NOW,
        1,
    )
    .unwrap();
    assert!(s.clip.is_none() && s.view.video.is_none());
    assert!(serde_json::to_value(&s.view).unwrap()["video"].is_null());
}

#[test]
fn a_session_combines_capture_layout_variables_and_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    // a project: <root>/Foo/bin/Foo with a log in the project folder
    let project = dir.path().join("Foo");
    std::fs::create_dir_all(project.join("bin")).unwrap();
    std::fs::write(project.join("run.log"), "log").unwrap();
    let exe = project.join("bin").join("Foo");

    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "L".into(),
        blocks: vec![
            Block {
                id: "note".into(),
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 6,
                    h: 2,
                },
                kind: BlockKind::Text(TextBlock {
                    title: "Note".into(),
                    placement: Default::default(),
                    role: Default::default(),
                    default_text: "{{project.name}} at {{capture.date}}".into(),
                    formatting: true,
                    one_line: false,
                }),
            },
            Block {
                id: "logs".into(),
                rect: Rect {
                    x: 6,
                    y: 0,
                    w: 6,
                    h: 2,
                },
                kind: BlockKind::Files(FilesBlock {
                    title: "Logs".into(),
                    include: vec!["*.log".into()],
                    preselect: vec!["*.log".into()],
                    look_in: Default::default(),
                }),
            },
        ],
    };
    store.save_item(Kind::Layouts, "bug", &layout).unwrap();
    let trigger = Trigger {
        name: "Bug".into(),
        layout_id: Some("bug".into()),
        ..Trigger::default()
    };

    let s = prepare_with(
        &store,
        &no_connections(),
        "bugtrig",
        &trigger,
        fake_capture(Some(&exe.display().to_string())),
        NOW,
        7,
    )
    .unwrap();
    assert_eq!(s.view.session_id, 7);
    assert_eq!(s.view.texts["note"].value, "Foo at 2026-10-23");
    assert_eq!(s.view.files["logs"].entries.len(), 1);
    assert!(s.view.files["logs"].entries[0].selected);
    assert_eq!(s.view.project.as_ref().unwrap().name, "Foo");
    assert_eq!(s.view.layout.as_ref().unwrap().id, "bug");
    assert_eq!(s.view.image.width, 4);
    assert_eq!(&s.png[1..4], b"PNG");
}

/// A capture of a project "Foo" with `run.log` ("LOGLINE") and `other.log`, and a Files block
/// offering `*.log` with `preselect` ticked by default.
fn log_capture(dir: &tempfile::TempDir, preselect: &[&str]) -> (Session, PathBuf) {
    let store = Store::new(Dirs::under(dir.path()));
    let project = dir.path().join("Foo");
    std::fs::create_dir_all(project.join("bin")).unwrap();
    std::fs::write(project.join("run.log"), "LOGLINE").unwrap();
    std::fs::write(project.join("other.log"), "OTHER").unwrap();
    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "L".into(),
        blocks: vec![Block {
            id: "logs".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 2,
            },
            kind: BlockKind::Files(FilesBlock {
                title: "Logs".into(),
                include: vec!["*.log".into()],
                preselect: preselect.iter().map(|p| p.to_string()).collect(),
                look_in: Default::default(),
            }),
        }],
    };
    store.save_item(Kind::Layouts, "l", &layout).unwrap();
    let trigger = Trigger {
        layout_id: Some("l".into()),
        ..Trigger::default()
    };
    let exe = project.join("bin").join("Foo");
    let session = prepare_with(
        &store,
        &no_connections(),
        "t",
        &trigger,
        fake_capture(Some(&exe.display().to_string())),
        NOW,
        3,
    )
    .unwrap();
    (session, project)
}

const BIG: u64 = 1024 * 1024;

/// Project `dev/foo` with its program at `dev/foo/bin/release/foo`, and logs in three places.
fn deep_capture(dir: &tempfile::TempDir, blocks: Vec<Block>) -> Session {
    let store = Store::new(Dirs::under(dir.path()));
    let w = |p: &str, body: &str| {
        let path = dir.path().join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    w("dev/foo/bin/release/foo", "");
    w("dev/foo/bin/release/logs/run.log", "RUN");
    w("dev/foo/logs/app.log", "APP");
    w("dev/logs/shared.log", "SHARED");
    w("dev/foo/readme.log", "README");
    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "L".into(),
        blocks,
    };
    store.save_item(Kind::Layouts, "l", &layout).unwrap();
    let trigger = Trigger {
        layout_id: Some("l".into()),
        ..Trigger::default()
    };
    let exe = dir.path().join("dev/foo/bin/release/foo");
    prepare_with(
        &store,
        &no_connections(),
        "t",
        &trigger,
        fake_capture(Some(&exe.display().to_string())),
        NOW,
        5,
    )
    .unwrap()
}

fn files_block(id: &str, y: u32, look_in: cargoframe_layout::LookIn) -> Block {
    Block {
        id: id.into(),
        rect: Rect {
            x: 0,
            y,
            w: 6,
            h: 2,
        },
        kind: BlockKind::Files(FilesBlock {
            title: id.into(),
            include: vec!["*.log".into()],
            preselect: vec!["*.log".into()],
            look_in,
        }),
    }
}

fn paths(s: &Session, block: &str) -> Vec<String> {
    s.view.files[block]
        .entries
        .iter()
        .map(|e| e.path.clone())
        .collect()
}

#[test]
fn files_are_found_in_folders_above_the_program_with_one_name_each() {
    use cargoframe_layout::{LookFolder, LookIn};
    let dir = tempfile::tempdir().unwrap();
    let s = deep_capture(
        &dir,
        vec![
            // The default: the project folder, as it always was.
            files_block("project", 0, LookIn::default()),
            files_block(
                "logs",
                2,
                LookIn {
                    folders: vec![LookFolder::Program, LookFolder::Above2, LookFolder::Above3],
                    subfolder: "logs".into(),
                },
            ),
        ],
    );
    // Paths are relative to the highest folder looked in: dev.
    assert_eq!(
        s.files_root.as_deref(),
        Some(dir.path().join("dev").display().to_string().as_str())
    );
    assert_eq!(paths(&s, "project"), ["foo/readme.log"]);
    assert_eq!(
        paths(&s, "logs"),
        [
            "foo/bin/release/logs/run.log",
            "foo/logs/app.log",
            "logs/shared.log"
        ]
    );
    let places = &s.view.files["logs"].places;
    assert_eq!(places.len(), 3);
    assert!(
        places.iter().all(|p| p.exists && p.found == 1),
        "{places:?}"
    );
    assert!(s.view.files["logs"].problem.is_none());
}

#[test]
fn with_only_the_default_the_paths_stay_relative_to_the_project_folder() {
    use cargoframe_layout::LookIn;
    let dir = tempfile::tempdir().unwrap();
    let s = deep_capture(&dir, vec![files_block("project", 0, LookIn::default())]);
    assert_eq!(
        s.files_root.as_deref(),
        Some(
            dir.path()
                .join("dev")
                .join("foo")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(paths(&s, "project"), ["readme.log"]);
}

#[test]
fn a_choice_that_cannot_be_found_says_why_and_the_rest_still_work() {
    use cargoframe_layout::{LookFolder, LookIn};
    let dir = tempfile::tempdir().unwrap();
    // Above3 from dev/foo/bin/release is dev, which exists; a missing subfolder is skipped.
    let s = deep_capture(
        &dir,
        vec![files_block(
            "logs",
            0,
            LookIn {
                folders: vec![LookFolder::Project, LookFolder::Above3],
                subfolder: "nothing-here".into(),
            },
        )],
    );
    assert!(s.view.files["logs"].entries.is_empty());
    assert_eq!(
        s.view.files["logs"].problem.as_deref(),
        Some("None of the folders to look in exist")
    );
    assert!(s.view.files["logs"].places.iter().all(|p| !p.exists));
}

#[test]
fn staging_copies_only_the_files_ticked_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = log_capture(&dir, &["run.log"]);
    let ws = Workspace::new(dir.path().join("captures"));
    assert_eq!(s.preselected(), vec!["run.log".to_string()]);
    assert_eq!(s.stage_preselected(&ws, BIG, BIG, &|| true), 1);
    let folder = s.dir_path().unwrap();
    assert_eq!(
        std::fs::read_to_string(folder.join("files").join("run.log")).unwrap(),
        "LOGLINE"
    );
    assert!(!folder.join("files").join("other.log").exists());
}

#[test]
fn a_staged_copy_is_a_snapshot_of_the_moment_of_the_capture() {
    let dir = tempfile::tempdir().unwrap();
    let (s, project) = log_capture(&dir, &["run.log"]);
    let ws = Workspace::new(dir.path().join("captures"));
    s.stage_preselected(&ws, BIG, BIG, &|| true);
    std::fs::write(project.join("run.log"), "LOGLINE and much more").unwrap();
    // Staging again (a share does) keeps the first copy.
    s.with_dir(&ws, |d| {
        d.stage(&project, "run.log", BIG).unwrap();
    })
    .unwrap();
    let folder = s.dir_path().unwrap();
    assert_eq!(
        std::fs::read_to_string(folder.join("files").join("run.log")).unwrap(),
        "LOGLINE"
    );
}

#[test]
fn nothing_ticked_means_no_folder_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = log_capture(&dir, &[]);
    let ws = Workspace::new(dir.path().join("captures"));
    assert!(s.preselected().is_empty());
    assert_eq!(s.stage_preselected(&ws, BIG, BIG, &|| true), 0);
    assert!(s.dir_path().is_none());
    assert!(!ws.root().exists());
}

#[test]
fn staging_leaves_out_what_is_too_big_and_stops_at_the_total() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = log_capture(&dir, &["*.log"]);
    let ws = Workspace::new(dir.path().join("captures"));
    // run.log is 7 bytes and other.log is 5: one file may be at most 6, so only other.log.
    assert_eq!(s.stage_preselected(&ws, 6, BIG, &|| true), 1);
    let folder = s.dir_path().unwrap();
    assert!(folder.join("files").join("other.log").is_file());
    assert!(!folder.join("files").join("run.log").exists());

    let dir = tempfile::tempdir().unwrap();
    let (s, _) = log_capture(&dir, &["*.log"]);
    let ws = Workspace::new(dir.path().join("captures"));
    // Together they are 12 bytes: with room for 10, the first fits and the second does not.
    assert_eq!(s.stage_preselected(&ws, BIG, 10, &|| true), 1);
}

#[test]
fn staging_stops_once_the_window_has_moved_on() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = log_capture(&dir, &["*.log"]);
    let ws = Workspace::new(dir.path().join("captures"));
    assert_eq!(s.stage_preselected(&ws, BIG, BIG, &|| false), 0);
    let folder = s.dir_path().unwrap();
    assert!(!folder.join("files").exists());
}

#[test]
fn data_rule_values_reach_the_window_and_only_used_failures_are_reported() {
    use cargoframe_rules::{Format, Rule};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    let project = dir.path().join("Foo");
    std::fs::create_dir_all(project.join("bin")).unwrap();
    std::fs::write(project.join("version.json"), r#"{"build": 42}"#).unwrap();
    let exe = project.join("bin").join("Foo");
    let rule = |file: &str, query: &str, fallback: Option<&str>| Rule {
        name: format!("{query} rule"),
        file: file.into(),
        format: Format::Json,
        query: query.into(),
        fallback: fallback.map(str::to_string),
        ..Rule::default()
    };
    store
        .save_item(Kind::Rules, "build", &rule("version.json", "build", None))
        .unwrap();
    store
        .save_item(Kind::Rules, "gone", &rule("missing.json", "x", None))
        .unwrap();
    store
        .save_item(Kind::Rules, "soft", &rule("missing.json", "y", Some("n/a")))
        .unwrap();
    store
        .save_item(Kind::Rules, "unused", &rule("missing.json", "z", None))
        .unwrap();
    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "L".into(),
        blocks: vec![Block {
            id: "note".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 2,
            },
            kind: BlockKind::Text(TextBlock {
                title: "Note".into(),
                placement: Default::default(),
                role: Default::default(),
                default_text: "b={{rule.build}} g={{rule.gone}} s={{rule.soft}}".into(),
                formatting: true,
                one_line: false,
            }),
        }],
    };
    store.save_item(Kind::Layouts, "l", &layout).unwrap();
    let trigger = Trigger {
        layout_id: Some("l".into()),
        ..Trigger::default()
    };
    let s = prepare_with(
        &store,
        &no_connections(),
        "t",
        &trigger,
        fake_capture(Some(&exe.display().to_string())),
        NOW,
        1,
    )
    .unwrap();
    // The value is read, the fallback fills in, and the failed rule stays unresolved.
    assert_eq!(s.view.variables["rule.build"], "42");
    assert_eq!(s.view.texts["note"].value, "b=42 g={{rule.gone}} s=n/a");
    assert_eq!(s.view.texts["note"].unresolved, ["rule.gone"]);
    // Only rules the layout uses are reported; the fallback one says so.
    assert_eq!(s.view.notes.len(), 2, "{:?}", s.view.notes);
    assert!(s
        .view
        .notes
        .iter()
        .any(|n| n.contains("x rule") && n.contains("no file matches")));
    assert!(s
        .view
        .notes
        .iter()
        .any(|n| n.contains("y rule") && n.contains("fallback")));
    assert!(!s.view.notes.iter().any(|n| n.contains("z rule")));
}

#[test]
fn a_file_found_by_its_name_gives_its_values_and_is_ticked_in_the_files() {
    use cargoframe_rules::{Find, Rule, ValueStep, Which};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    let project = dir.path().join("Foo");
    std::fs::create_dir_all(project.join("bin")).unwrap();
    std::fs::create_dir_all(project.join("diag")).unwrap();
    std::fs::write(project.join("diag/system_info.txt"), "GPU: RTX 4070\n").unwrap();
    let rule = Rule {
        name: "Machine".into(),
        file: "system_info".into(),
        add_to_files: true,
        values: vec![ValueStep {
            key: "gpu".into(),
            name: "GPU".into(),
            find: Find::ValueOf { name: "GPU".into() },
            which: Which::First,
            fallback: None,
        }],
        ..Rule::default()
    };
    store.save_item(Kind::Rules, "machine", &rule).unwrap();
    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "L".into(),
        blocks: vec![Block {
            id: "logs".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 2,
            },
            // the block itself only offers logs: the rule adds its file anyway
            kind: BlockKind::Files(FilesBlock {
                title: "Logs".into(),
                include: vec!["*.log".into()],
                preselect: vec![],
                look_in: Default::default(),
            }),
        }],
    };
    store.save_item(Kind::Layouts, "l", &layout).unwrap();
    let trigger = Trigger {
        layout_id: Some("l".into()),
        ..Trigger::default()
    };
    let exe = project.join("bin").join("Foo");
    let s = prepare_with(
        &store,
        &no_connections(),
        "t",
        &trigger,
        fake_capture(Some(&exe.display().to_string())),
        NOW,
        4,
    )
    .unwrap();
    assert_eq!(s.view.variables["rule.gpu"], "RTX 4070");
    assert_eq!(s.preselected(), vec!["diag/system_info.txt".to_string()]);
}

#[test]
fn no_program_means_no_project_and_files_say_why() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    let layout = Layout {
        accepts_input: false,
        columns: cargoframe_layout::GRID_COLUMNS,
        id: String::new(),
        name: "L".into(),
        blocks: vec![Block {
            id: "logs".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 2,
            },
            kind: BlockKind::Files(FilesBlock {
                title: "Logs".into(),
                include: vec!["*".into()],
                preselect: vec![],
                look_in: Default::default(),
            }),
        }],
    };
    store.save_item(Kind::Layouts, "l", &layout).unwrap();
    let trigger = Trigger {
        layout_id: Some("l".into()),
        ..Trigger::default()
    };
    let s = prepare_with(
        &store,
        &no_connections(),
        "t",
        &trigger,
        fake_capture(None),
        NOW,
        1,
    )
    .unwrap();
    assert!(s.view.project.is_none());
    assert!(s.view.files["logs"]
        .problem
        .as_deref()
        .unwrap()
        .contains("No project folder"));
}

#[test]
fn trigger_without_layout_or_with_a_missing_one_shows_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    let none = Trigger::default();
    let s = prepare_with(
        &store,
        &no_connections(),
        "t",
        &none,
        fake_capture(None),
        NOW,
        1,
    )
    .unwrap();
    assert!(s.view.layout.is_none());
    assert!(s
        .view
        .layout_problem
        .as_deref()
        .unwrap()
        .contains("no layout"));
    let gone = Trigger {
        layout_id: Some("gone".into()),
        ..Trigger::default()
    };
    let s = prepare_with(
        &store,
        &no_connections(),
        "t",
        &gone,
        fake_capture(None),
        NOW,
        1,
    )
    .unwrap();
    assert!(s
        .view
        .layout_problem
        .as_deref()
        .unwrap()
        .contains("no longer exists"));
}

#[test]
fn only_png_pictures_of_a_sane_size_are_accepted() {
    assert!(check_png(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]).is_ok());
    assert!(check_png(b"GIF89a....").is_err());
    assert!(check_png(&[0x89, b'P']).is_err());
    assert!(check_png(&[]).is_err());
    let mut big = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    big.resize(MAX_EDITED_PNG + 1, 0);
    assert!(check_png(&big).is_err());
}

#[test]
fn the_edited_picture_replaces_the_capture_only_while_set() {
    let s = Session::new(vec![1, 2], {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(cargoframe_config::Dirs::under(dir.path()));
        prepare_with(
            &store,
            &no_connections(),
            "t",
            &Trigger::default(),
            fake_capture(None),
            NOW,
            1,
        )
        .unwrap()
        .view
    });
    assert_eq!(s.image_to_send(), vec![1, 2]);
    s.set_edited(Some(vec![9]));
    assert_eq!(s.image_to_send(), vec![9]);
    s.set_edited(None);
    assert_eq!(s.image_to_send(), vec![1, 2]);
}

fn a_session(sessions: &Sessions) -> Session {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(Dirs::under(dir.path()));
    prepare_with(
        &store,
        &no_connections(),
        "t",
        &Trigger::default(),
        fake_capture(None),
        NOW,
        sessions.next_id(),
    )
    .unwrap()
}

#[test]
fn each_window_holds_its_own_capture() {
    let sessions = Sessions::new();
    assert!(sessions.for_label("capture").is_none());
    let (first, a) = sessions.place(a_session(&sessions)).unwrap();
    let (second, b) = sessions.place(a_session(&sessions)).unwrap();
    assert_eq!((first.as_str(), second.as_str()), ("capture", "capture-2"));
    assert_eq!(a.view.session_id, 1);
    assert_eq!(b.view.session_id, 2);
    assert_eq!(sessions.for_label("capture-2").unwrap().view.session_id, 2);
    assert_eq!(sessions.by_id(1).unwrap().view.session_id, 1);
    assert_eq!(sessions.latest().unwrap().view.session_id, 2);
    assert_eq!(sessions.count(), 2);
    // closing the first window leaves the second alone, and its place is used next
    assert!(sessions.clear("capture").is_some());
    assert!(sessions.for_label("capture").is_none());
    assert!(sessions.by_id(1).is_none());
    assert_eq!(sessions.for_label("capture-2").unwrap().view.session_id, 2);
    let (third, _) = sessions.place(a_session(&sessions)).unwrap();
    assert_eq!(third, "capture");
    assert!(sessions.clear("capture-9").is_none());
}

#[test]
fn there_is_room_for_ten_windows_and_the_eleventh_is_refused() {
    let sessions = Sessions::new();
    let mut labels = Vec::new();
    for _ in 0..MAX_WINDOWS {
        assert!(!sessions.is_full());
        labels.push(sessions.place(a_session(&sessions)).unwrap().0);
    }
    assert!(sessions.is_full());
    assert_eq!(sessions.all().len(), 10);
    assert_eq!(labels[0], "capture");
    assert_eq!(labels[9], "capture-10");
    assert!(sessions.place(a_session(&sessions)).is_err());
    // a closed window gives its place back
    sessions.clear("capture-4");
    assert!(!sessions.is_full());
    assert_eq!(sessions.place(a_session(&sessions)).unwrap().0, "capture-4");
}

#[test]
fn only_the_windows_of_a_capture_have_a_capture_label() {
    assert_eq!(window_label(1), "capture");
    assert_eq!(window_label(2), "capture-2");
    assert!(is_capture_label("capture"));
    assert!(is_capture_label("capture-2"));
    assert!(is_capture_label("capture-10"));
    for not in [
        "capture-1",
        "capture-11",
        "capture-0",
        "capture-x",
        "capture2",
        "settings",
        "",
        "Capture",
    ] {
        assert!(!is_capture_label(not), "{not}");
    }
}

#[test]
fn values_from_another_program_fill_text_boxes_and_dropdowns_of_a_layout_that_accepts_them() {
    use cargoframe_layout::{DropdownBlock, DropdownSource};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(cargoframe_config::Dirs::under(dir.path()));
    let rect = Rect {
        x: 0,
        y: 0,
        w: 6,
        h: 2,
    };
    let mut layout = Layout::empty("", "L");
    layout.blocks = vec![
        Block {
            id: "summary".into(),
            rect,
            kind: BlockKind::Text(TextBlock {
                title: "Summary".into(),
                placement: Default::default(),
                role: Default::default(),
                default_text: "{{app.title}}".into(),
                formatting: true,
                one_line: false,
            }),
        },
        Block {
            id: "chat".into(),
            rect,
            kind: BlockKind::Dropdown(DropdownBlock::new("Chat", DropdownSource::ClaudeChats)),
        },
    ];
    let prepared = |l: &Layout| {
        store.save_item(Kind::Layouts, "l", l).unwrap();
        let trigger = Trigger {
            layout_id: Some("l".into()),
            ..Trigger::default()
        };
        prepare_with(
            &store,
            &no_connections(),
            "t",
            &trigger,
            fake_capture(None),
            NOW,
            1,
        )
        .unwrap()
    };
    let values = vec![
        ("summary".to_string(), "It crashed".to_string()),
        ("chat".to_string(), "new".to_string()),
        ("nope".to_string(), "x".to_string()),
    ];

    // not accepted: nothing is filled, and the window says why
    let mut s = prepared(&layout);
    let before = s.view.texts["summary"].value.clone();
    apply_incoming(&mut s.view, &values);
    assert_eq!(s.view.texts["summary"].value, before);
    assert!(s.view.notes.iter().any(|n| n.contains("does not accept")));

    layout.accepts_input = true;
    let mut s = prepared(&layout);
    apply_incoming(&mut s.view, &values);
    assert_eq!(s.view.texts["summary"].value, "It crashed");
    assert_eq!(s.view.dropdowns["chat"].selected.as_deref(), Some("new"));
    assert!(s.view.notes.iter().any(|n| n.contains("nope")));
}

#[test]
fn the_layout_says_whether_a_trigger_records() {
    use cargoframe_layout::VideoBlock;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(cargoframe_config::Dirs::under(dir.path()));
    let rect = Rect {
        x: 0,
        y: 0,
        w: 6,
        h: 4,
    };
    let mut shot = Layout::empty("", "Shot");
    shot.blocks = vec![Block {
        id: "s".into(),
        rect,
        kind: BlockKind::Screenshot,
    }];
    let mut both = shot.clone();
    both.blocks.push(Block {
        id: "v".into(),
        rect: Rect { y: 4, ..rect },
        kind: BlockKind::Video(VideoBlock::default()),
    });
    store.save_item(Kind::Layouts, "shot", &shot).unwrap();
    store.save_item(Kind::Layouts, "both", &both).unwrap();
    let with = |id: Option<&str>, media| Trigger {
        layout_id: id.map(String::from),
        media,
        ..Trigger::default()
    };
    use cargoframe_config::Media;
    assert!(!records(&store, &with(Some("shot"), Media::Screenshot)));
    // a screenshot and a video in one layout: a press records, and the screenshot is taken from it
    assert!(records(&store, &with(Some("both"), Media::Screenshot)));
    // the old "Makes" setting is no longer what decides
    assert!(!records(&store, &with(Some("shot"), Media::Video)));
    assert!(!records(&store, &with(None, Media::Video)));
    assert!(!records(&store, &with(Some("gone"), Media::Video)));
}
