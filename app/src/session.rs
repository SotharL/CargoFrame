//! One capture: take the screenshot, work out the context, build what the window shows.

use crate::connections::Connections;
use crate::registry::AppRegistry;
use cargoframe_capture as cap;
use cargoframe_config::{CaptureMode, Kind, Store, Trigger};
use cargoframe_handoff::ChatList;
use cargoframe_layout::Layout;
use cargoframe_rules::{run_all, FsFiles, Rule};
use cargoframe_window::files::{self, Limits, Scan};
use cargoframe_window::vars::{self, Moment};
use cargoframe_window::view::{self, AppView, ImageView, LayoutProblem, ProjectView, SessionView};
use chrono::{Datelike, Local, Timelike};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct Session {
    /// The screenshot, PNG-encoded for speed.
    pub png: Vec<u8>,
    pub view: SessionView,
    /// The picture as the person edited it (crop, drawings), sent by the window just before a
    /// share. Replaced each time; `None` while the picture is untouched.
    edited: Mutex<Option<Vec<u8>>>,
    /// The folder the project files are read from, as resolved at capture time. Kept here so a
    /// share never trusts a folder name sent back by the window.
    pub project_folder: Option<String>,
}

impl Session {
    pub fn new(png: Vec<u8>, view: SessionView) -> Self {
        let project_folder = view.project.as_ref().map(|p| p.folder.clone());
        Self {
            png,
            view,
            edited: Mutex::new(None),
            project_folder,
        }
    }

    pub fn set_edited(&self, png: Option<Vec<u8>>) {
        *self.edited.lock().unwrap_or_else(|e| e.into_inner()) = png;
    }

    /// The picture to send: the edited one if there is one, else the capture.
    pub fn image_to_send(&self) -> Vec<u8> {
        self.edited
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_else(|| self.png.clone())
    }
}

/// The capture the window is showing. One at a time: a new capture replaces it.
pub struct Sessions {
    current: Mutex<Option<Arc<Session>>>,
    next: AtomicU64,
}

impl Sessions {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
            next: AtomicU64::new(1),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    pub fn set(&self, session: Session) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(session));
    }

    pub fn current(&self) -> Option<Arc<Session>> {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn clear(&self) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("trigger \"{0}\" does not exist")]
    NoTrigger(String),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Capture(String),
}

fn mode_of(mode: CaptureMode) -> cap::Mode {
    match mode {
        CaptureMode::ActiveWindow => cap::Mode::ActiveWindow,
        CaptureMode::Display => cap::Mode::Display,
        CaptureMode::AllDisplays => cap::Mode::AllDisplays,
    }
}

fn now() -> Moment {
    let t = Local::now();
    Moment {
        year: t.year(),
        month: t.month() as u8,
        day: t.day() as u8,
        hour: t.hour() as u8,
        minute: t.minute() as u8,
        second: t.second() as u8,
    }
}

/// Capture for real and prepare the session.
pub fn capture_and_prepare(
    store: &Store,
    connections: &Connections,
    trigger_id: &str,
    id: u64,
) -> Result<Session, PrepareError> {
    let trigger = store
        .load_item::<Trigger>(Kind::Triggers, trigger_id)
        .map_err(|e| PrepareError::Config(e.to_string()))?
        .ok_or_else(|| PrepareError::NoTrigger(trigger_id.to_string()))?;
    let started = std::time::Instant::now();
    let captured =
        cap::capture(mode_of(trigger.capture)).map_err(|e| PrepareError::Capture(e.to_string()))?;
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "captured"
    );
    let session = prepare_with(store, connections, trigger_id, &trigger, captured, now(), id)?;
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "session ready"
    );
    Ok(session)
}

/// Everything after the capture; separate so it can be tested without a screen.
pub fn prepare_with(
    store: &Store,
    connections: &Connections,
    trigger_id: &str,
    trigger: &Trigger,
    captured: cap::Captured,
    now: Moment,
    id: u64,
) -> Result<Session, PrepareError> {
    let exe = captured
        .app
        .as_ref()
        .and_then(|a| a.exe_path.as_ref())
        .map(|p| p.display().to_string());
    let title = captured.app.as_ref().map(|a| a.title.clone());
    let builtins = vars::builtins(exe.as_deref(), title.as_deref(), now);

    let project = builtins.project.as_ref().map(|r| ProjectView {
        folder: r.folder.clone(),
        name: builtins
            .values
            .get("project.name")
            .cloned()
            .unwrap_or_default(),
        how: vars::describe(&r.how),
    });
    let app = captured.app.as_ref().map(|a| AppView {
        title: a.title.clone(),
        exe_name: builtins.values.get("app.exe").cloned(),
        exe_path: exe.clone(),
    });

    let layout: Result<Layout, LayoutProblem> = match &trigger.layout_id {
        None => Err(LayoutProblem::NotSet),
        Some(layout_id) => match store.load_item::<Layout>(Kind::Layouts, layout_id) {
            Ok(Some(mut l)) => {
                l.id = layout_id.clone();
                Ok(l)
            }
            Ok(None) => Err(LayoutProblem::Missing(layout_id.clone())),
            Err(e) => Err(LayoutProblem::Unreadable(e.to_string())),
        },
    };
    let chats = store
        .load_or_default::<ChatList>()
        .map(ChatList::normalized)
        .unwrap_or_default();
    let registry = AppRegistry::from_store(store, connections);

    // Data rules: read their values from the project folder, then offer them to the window as
    // `{{rule.<id>}}`. A rule that fails only matters if the layout uses it.
    let mut variables = builtins.values.clone();
    let mut notes = captured.notes.clone();
    if let Ok(loaded) = store.load_all::<Rule>(Kind::Rules) {
        let outcomes = run_all(
            loaded.items.iter().map(|(id, r)| (id.as_str(), r)),
            project.as_ref().map(|p| Path::new(&p.folder)),
            &FsFiles,
        );
        let used = layout
            .as_ref()
            .map(vars::referenced_rules)
            .unwrap_or_default();
        for (id, rule) in &loaded.items {
            let Some(outcome) = outcomes.get(id) else {
                continue;
            };
            if let Some(v) = &outcome.value {
                variables.insert(format!("rule.{id}"), v.clone());
            }
            if let (true, Some(problem)) = (used.contains(id), &outcome.problem) {
                let name = if rule.name.trim().is_empty() {
                    id.as_str()
                } else {
                    rule.name.trim()
                };
                let tail = if outcome.used_fallback {
                    " (using its fallback value)"
                } else {
                    ""
                };
                notes.push(format!("Data rule \u{201c}{name}\u{201d}: {problem}{tail}"));
            }
        }
    }

    let scan = |f: &cargoframe_layout::FilesBlock| -> Scan {
        match &project {
            Some(p) => files::scan(
                Path::new(&p.folder),
                &f.include,
                &f.preselect,
                Limits::default(),
            ),
            None => Scan {
                problem: Some("No project folder could be found for this capture".into()),
                ..Scan::default()
            },
        }
    };
    let subject = match captured.subject {
        cap::Subject::Window => "window",
        cap::Subject::Display => "display",
        cap::Subject::AllDisplays => "all_displays",
    };
    let inputs = view::Inputs {
        session_id: id,
        trigger_id,
        trigger_name: &trigger.name,
        layout: layout.as_ref().map_err(Clone::clone),
        subject,
        notes,
        app,
        project: project.clone(),
        image: ImageView {
            width: captured.frame.width(),
            height: captured.frame.height(),
        },
        variables: &variables,
        chats: &chats,
        registry: &registry,
        scan: &scan,
    };
    let view = view::build(&inputs);
    let png = captured
        .frame
        .to_png()
        .map_err(|e| PrepareError::Capture(e.to_string()))?;
    Ok(Session::new(png, view))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_config::Dirs;
    use cargoframe_layout::{Block, BlockKind, FilesBlock, Rect, TextBlock};

    fn no_connections() -> Connections {
        Connections::new(
            std::sync::Arc::new(cargoframe_net::UreqHttp::new()),
            std::sync::Arc::new(cargoframe_connectors::MemorySecrets::new()),
        )
    }

    fn fake_capture(exe: Option<&str>) -> cap::Captured {
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

    const NOW: Moment = Moment {
        year: 2026,
        month: 10,
        day: 23,
        hour: 8,
        minute: 0,
        second: 1,
    };

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
    fn no_program_means_no_project_and_files_say_why() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Dirs::under(dir.path()));
        let layout = Layout {
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
                }),
            }],
        };
        store.save_item(Kind::Layouts, "l", &layout).unwrap();
        let trigger = Trigger {
            layout_id: Some("l".into()),
            ..Trigger::default()
        };
        let s = prepare_with(&store, &no_connections(), "t", &trigger, fake_capture(None), NOW, 1).unwrap();
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
        let s = prepare_with(&store, &no_connections(), "t", &none, fake_capture(None), NOW, 1).unwrap();
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
        let s = prepare_with(&store, &no_connections(), "t", &gone, fake_capture(None), NOW, 1).unwrap();
        assert!(s
            .view
            .layout_problem
            .as_deref()
            .unwrap()
            .contains("no longer exists"));
    }

    #[test]
    fn sessions_hold_one_capture_at_a_time() {
        let sessions = Sessions::new();
        assert!(sessions.current().is_none());
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Dirs::under(dir.path()));
        let s = prepare_with(
            &store,
            &no_connections(),
            "t",
            &Trigger::default(),
            fake_capture(None),
            NOW,
            sessions.next_id(),
        )
        .unwrap();
        sessions.set(s);
        assert_eq!(sessions.current().unwrap().view.session_id, 1);
        assert_eq!(sessions.next_id(), 2);
        sessions.clear();
        assert!(sessions.current().is_none());
    }
}
