//! One capture: take the screenshot, work out the context, build what the window shows.

use crate::connections::Connections;
use crate::registry::AppRegistry;
use cargoframe_bundle::{CaptureDir, Workspace};
use cargoframe_capture as cap;
use cargoframe_config::{CaptureMode, Kind, Store, Trigger};
use cargoframe_handoff::ChatList;
use cargoframe_layout::Layout;
use cargoframe_rules::{run_rules, FsFiles, Rule};
use cargoframe_window::files::{self, Limits, Scan};
use cargoframe_window::places;
use cargoframe_window::vars::{self, Moment};
use cargoframe_window::view::{
    self, AppView, ImageView, LayoutProblem, ProjectView, SessionView, VideoView,
};
use chrono::{Datelike, Local, Timelike};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The recording of a capture made by a video trigger, kept in the capture's own folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub bytes: u64,
    /// The recording has a sound track.
    pub has_audio: bool,
}

pub struct Session {
    /// The screenshot, PNG-encoded for speed. For a video capture it is the picture taken when
    /// the recording was stopped, used for the context (the program in focus) and as a poster.
    pub png: Vec<u8>,
    pub view: SessionView,
    /// The recording, for a capture made by a video trigger.
    pub clip: Option<Clip>,
    /// The picture as the person edited it (crop, drawings), sent by the window just before a
    /// share. Replaced each time; `None` while the picture is untouched.
    edited: Mutex<Option<Vec<u8>>>,
    /// For a recording: the mark the window drew on a see-through picture the size of the
    /// recording (a PNG), sent just before a share. `None` while there is no mark.
    clip_mark: Mutex<Option<Vec<u8>>>,
    /// The folder the ticked files are read from, as worked out at capture time: the highest
    /// folder any Files block looks in (the project folder unless a block looks higher). Every
    /// file path the window shows is relative to it. Kept here so a share never trusts a folder
    /// name sent back by the window.
    pub files_root: Option<String>,
    /// The folder this capture keeps copies in (the screenshot, the ticked files), made when a
    /// destination first needs real files. It is deleted when the capture is closed.
    dir: Mutex<Option<CaptureDir>>,
}

impl Session {
    pub fn new(png: Vec<u8>, view: SessionView) -> Self {
        let files_root = view.project.as_ref().map(|p| p.folder.clone());
        Self {
            png,
            view,
            clip: None,
            edited: Mutex::new(None),
            clip_mark: Mutex::new(None),
            files_root,
            dir: Mutex::new(None),
        }
    }

    /// This capture is a recording: tell the window about it.
    pub fn with_clip(mut self, clip: Clip) -> Self {
        self.view.video = Some(VideoView {
            width: clip.width,
            height: clip.height,
            duration_ms: clip.duration_ms,
            bytes: clip.bytes,
            audio: clip.has_audio,
        });
        self.clip = Some(clip);
        self
    }

    /// Read the files from another folder than the project folder (see `files_root`).
    pub fn with_files_root(mut self, root: Option<String>) -> Self {
        self.files_root = root;
        self
    }

    /// Use this capture's folder (made now if it does not exist yet).
    pub fn with_dir<R>(
        &self,
        workspace: &Workspace,
        f: impl FnOnce(&CaptureDir) -> R,
    ) -> std::io::Result<R> {
        let mut guard = self.dir.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(workspace.create(self.view.session_id)?);
        }
        Ok(f(guard.as_ref().expect("the folder was just made")))
    }

    /// The files ticked by default, once each, in the order the window lists them.
    pub fn preselected(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        let Some(layout) = self.view.layout.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for block in &layout.blocks {
            let Some(state) = self.view.files.get(&block.id) else {
                continue;
            };
            for entry in state.entries.iter().filter(|e| e.selected) {
                if seen.insert(entry.path.clone()) {
                    out.push(entry.path.clone());
                }
            }
        }
        out
    }

    /// Copy the files that are ticked by default into this capture's folder, as they are right
    /// now: a log that keeps growing is what it was at the moment of the capture, and a share
    /// has less to do when the button is pressed. Meant for a background thread: it holds the
    /// folder's lock while it works, so a share that starts meanwhile waits for it.
    ///
    /// Best effort: a file that cannot be copied is left for the share to try again (and report).
    /// Nothing is copied beyond `max_file` bytes for one file or `max_total` for all of them, and
    /// it stops as soon as `still_open` says the window moved on. Returns how many were copied.
    pub fn stage_preselected(
        &self,
        workspace: &Workspace,
        max_file: u64,
        max_total: u64,
        still_open: &dyn Fn() -> bool,
    ) -> usize {
        let wanted = self.preselected();
        let (Some(root), false) = (self.files_root.as_deref(), wanted.is_empty()) else {
            return 0;
        };
        let sizes: std::collections::BTreeMap<&str, u64> = self
            .view
            .files
            .values()
            .flat_map(|f| f.entries.iter().map(|e| (e.path.as_str(), e.size)))
            .collect();
        let copied = self.with_dir(workspace, |dir| {
            let mut count = 0;
            let mut total = 0u64;
            for rel in &wanted {
                if !still_open() {
                    break;
                }
                let size = sizes.get(rel.as_str()).copied().unwrap_or(0);
                if size > max_file || total + size > max_total {
                    continue;
                }
                if dir.stage(Path::new(root), rel, max_file).is_ok() {
                    count += 1;
                    total += size;
                }
            }
            count
        });
        copied.unwrap_or(0)
    }

    /// Where this capture's folder is, if it has one.
    pub fn dir_path(&self) -> Option<PathBuf> {
        self.dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|d| d.path().to_path_buf())
    }

    pub fn set_edited(&self, png: Option<Vec<u8>>) {
        *self.edited.lock().unwrap_or_else(|e| e.into_inner()) = png;
    }

    pub fn set_clip_mark(&self, png: Option<Vec<u8>>) {
        *self.clip_mark.lock().unwrap_or_else(|e| e.into_inner()) = png;
    }

    /// The mark the window drew for the recording, if there is one.
    pub fn clip_mark(&self) -> Option<Vec<u8>> {
        self.clip_mark
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
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

/// How many CargoFrame windows can be open at once.
pub const MAX_WINDOWS: usize = 10;

/// The label of the window number `n` (1 to [`MAX_WINDOWS`]): `capture`, `capture-2`, `capture-3`...
pub fn window_label(n: usize) -> String {
    if n <= 1 {
        "capture".to_string()
    } else {
        format!("capture-{n}")
    }
}

/// Whether `label` is the label of a CargoFrame (capture) window.
pub fn is_capture_label(label: &str) -> bool {
    label == "capture"
        || label
            .strip_prefix("capture-")
            .and_then(|n| n.parse::<usize>().ok())
            .is_some_and(|n| (2..=MAX_WINDOWS).contains(&n))
}

/// The captures that are open, one per CargoFrame window, by the window's label. Up to
/// [`MAX_WINDOWS`] at a time; a window that is closed gives its place back.
pub struct Sessions {
    open: Mutex<BTreeMap<String, Arc<Session>>>,
    next: AtomicU64,
}

/// Every one of the windows is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllWindowsOpen;

impl Sessions {
    pub fn new() -> Self {
        Self {
            open: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Arc<Session>>> {
        self.open.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn next_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// Give `session` the first window that has no capture, and say which. Fails when all
    /// [`MAX_WINDOWS`] have one.
    pub fn place(&self, session: Session) -> Result<(String, Arc<Session>), AllWindowsOpen> {
        let mut open = self.lock();
        let label = (1..=MAX_WINDOWS)
            .map(window_label)
            .find(|l| !open.contains_key(l))
            .ok_or(AllWindowsOpen)?;
        let session = Arc::new(session);
        open.insert(label.clone(), session.clone());
        Ok((label, session))
    }

    /// The capture the window `label` shows.
    pub fn for_label(&self, label: &str) -> Option<Arc<Session>> {
        self.lock().get(label).cloned()
    }

    /// The capture with this number, whichever window shows it.
    pub fn by_id(&self, id: u64) -> Option<Arc<Session>> {
        self.lock()
            .values()
            .find(|s| s.view.session_id == id)
            .cloned()
    }

    /// The window `label` was closed: its capture is no longer open.
    pub fn clear(&self, label: &str) -> Option<Arc<Session>> {
        self.lock().remove(label)
    }

    pub fn count(&self) -> usize {
        self.lock().len()
    }

    pub fn is_full(&self) -> bool {
        self.count() >= MAX_WINDOWS
    }

    /// The capture taken last, whichever window shows it.
    pub fn latest(&self) -> Option<Arc<Session>> {
        self.lock()
            .values()
            .max_by_key(|s| s.view.session_id)
            .cloned()
    }

    /// Every open capture.
    pub fn all(&self) -> Vec<Arc<Session>> {
        self.lock().values().cloned().collect()
    }
}

impl Default for Sessions {
    fn default() -> Self {
        Self::new()
    }
}

/// The largest edited picture the window may hand over.
pub const MAX_EDITED_PNG: usize = 64 * 1024 * 1024;

/// Accept only what is plainly a PNG of a sane size.
pub fn check_png(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_EDITED_PNG {
        return Err("The edited picture is too large".into());
    }
    if bytes.len() < 8 || bytes[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err("The edited picture is not a PNG".into());
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("trigger \"{0}\" does not exist")]
    NoTrigger(String),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Capture(String),
    #[error("{0}")]
    Clip(String),
}

fn mode_of(mode: CaptureMode) -> cap::Mode {
    match mode {
        CaptureMode::ActiveWindow => cap::Mode::ActiveWindow,
        CaptureMode::Display => cap::Mode::Display,
        CaptureMode::AllDisplays => cap::Mode::AllDisplays,
    }
}

/// The name of the recording inside a capture's own folder.
pub const CLIP_NAME: &str = "recording.mp4";

/// What a finished recording is, as far as the window needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub bytes: u64,
    pub has_audio: bool,
}

/// A picture taken now, when a recording has just been stopped: what the program in focus is
/// gives the capture its context, and the picture is the poster.
pub fn capture_for_video(trigger: &Trigger) -> Result<cap::Captured, PrepareError> {
    cap::capture(mode_of(trigger.capture)).map_err(|e| PrepareError::Capture(e.to_string()))
}

/// A capture made by a video trigger: everything `prepare_with` does, and the recording moved
/// into the capture's own folder (so it goes when the capture is closed, and a recording started
/// meanwhile cannot take it away).
#[allow(clippy::too_many_arguments)]
pub fn prepare_video(
    store: &Store,
    connections: &Connections,
    workspace: &Workspace,
    trigger_id: &str,
    trigger: &Trigger,
    captured: cap::Captured,
    now: Moment,
    id: u64,
    clip: &Path,
    info: ClipInfo,
) -> Result<Session, PrepareError> {
    let session = prepare_with(store, connections, trigger_id, trigger, captured, now, id)?;
    let moved = session
        .with_dir(workspace, |dir| {
            let target = dir.path().join(CLIP_NAME);
            move_file(clip, &target).map(|()| target)
        })
        .map_err(|e| {
            PrepareError::Clip(format!(
                "CargoFrame could not make a folder for the recording: {e}"
            ))
        })?
        .map_err(|e| PrepareError::Clip(format!("CargoFrame could not keep the recording: {e}")))?;
    Ok(session.with_clip(Clip {
        path: moved,
        width: info.width,
        height: info.height,
        duration_ms: info.duration_ms,
        bytes: info.bytes,
        has_audio: info.has_audio,
    }))
}

/// Move a file: a rename where it can be, a copy and a delete across drives.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

pub fn now() -> Moment {
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

/// Put the values another program sent (`cargoframe open`) into the window's text boxes and
/// dropdowns. Only a layout marked *Other programs can fill this* takes them; what cannot be
/// used is said in a note in the window, never silently dropped.
pub fn apply_incoming(view: &mut SessionView, values: &[(String, String)]) {
    if values.is_empty() {
        return;
    }
    let Some(layout) = view.layout.clone() else {
        return;
    };
    if !layout.accepts_input {
        view.notes.push(
            "Another program sent values, but this layout does not accept them: turn on \"Other programs can fill this\" in the layout".into(),
        );
        return;
    }
    let mut unknown = Vec::new();
    for (id, value) in values {
        match layout.block(id).map(|b| &b.kind) {
            Some(cargoframe_layout::BlockKind::Text(_)) => {
                view.texts.insert(
                    id.clone(),
                    view::TextState {
                        value: value.clone(),
                        unresolved: Vec::new(),
                    },
                );
            }
            Some(cargoframe_layout::BlockKind::Dropdown(_)) => {
                if let Some(d) = view.dropdowns.get_mut(id) {
                    d.selected = Some(value.clone());
                }
            }
            _ => unknown.push(id.as_str()),
        }
    }
    if !unknown.is_empty() {
        view.notes.push(format!(
            "Another program sent values for {}, which this layout has no text box or dropdown for",
            unknown.join(", ")
        ));
    }
}

/// Does a press of this trigger record a video? Its layout decides: a layout with a Video block
/// records (a press starts, the next one stops), any other takes a screenshot. A trigger with no
/// layout, or one that cannot be read, takes a screenshot.
pub fn records(store: &Store, trigger: &Trigger) -> bool {
    trigger
        .layout_id
        .as_deref()
        .and_then(|id| store.load_item::<Layout>(Kind::Layouts, id).ok().flatten())
        .is_some_and(|l| {
            l.blocks
                .iter()
                .any(|b| matches!(b.kind, cargoframe_layout::BlockKind::Video(_)))
        })
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
    let session = prepare_with(
        store,
        connections,
        trigger_id,
        &trigger,
        captured,
        now(),
        id,
    )?;
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

    // Data from files: read the values from the project folder, then offer them to the window as
    // `{{rule.<key>}}`. A value that is missing only matters if the layout uses it. Files the
    // rules add are ticked in the Files blocks.
    let mut variables = builtins.values.clone();
    let mut notes = captured.notes.clone();
    let mut added_files: Vec<String> = Vec::new();
    if let Ok(loaded) = store.load_all::<Rule>(Kind::Rules) {
        let mut report = run_rules(
            loaded.items.iter().map(|(id, r)| (id.as_str(), r)),
            project.as_ref().map(|p| Path::new(&p.folder)),
            &FsFiles,
        );
        // values made from other values, once every file was read
        let mut known = variables.clone();
        for (key, outcome) in &report.values {
            if let Some(v) = &outcome.value {
                known.insert(format!("rule.{key}"), v.clone());
            }
        }
        cargoframe_rules::combine_values(
            loaded.items.iter().map(|(id, r)| (id.as_str(), r)),
            &mut report,
            &mut known,
        );
        let used = layout
            .as_ref()
            .map(vars::referenced_rules)
            .unwrap_or_default();
        for (key, outcome) in &report.values {
            if let Some(v) = &outcome.value {
                variables.insert(format!("rule.{key}"), v.clone());
            }
            if let (true, Some(problem)) = (used.contains(key), &outcome.problem) {
                let name = report
                    .owners
                    .get(key)
                    .and_then(|id| {
                        loaded
                            .items
                            .iter()
                            .find(|(rid, _)| rid == id)
                            .map(|(rid, r)| (rid, r))
                    })
                    .and_then(|(rid, r)| r.steps(rid).into_iter().find(|s| &s.key == key))
                    .map(|s| s.name)
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| key.clone());
                let tail = if outcome.used_fallback {
                    " (using its fallback value)"
                } else {
                    ""
                };
                notes.push(format!(
                    "From a file, \u{201c}{}\u{201d}: {problem}{tail}",
                    name.trim()
                ));
            }
        }
        added_files = report.files;
    }

    // Where the Files blocks look. File paths are relative to the highest folder any of them
    // looks in; with the default that is the project folder.
    let project_dir = project.as_ref().map(|p| p.folder.as_str());
    let files_root = layout
        .as_ref()
        .ok()
        .and_then(|l| places::files_root(l, project_dir, exe.as_deref()))
        .or_else(|| project_dir.map(PathBuf::from));
    let scan = |f: &cargoframe_layout::FilesBlock| -> Scan {
        let Some(root) = &files_root else {
            return Scan {
                problem: Some("No project folder could be found for this capture".into()),
                ..Scan::default()
            };
        };
        let found = places::places_for(&f.look_in, project_dir, exe.as_deref());
        if found.places.is_empty() {
            let why = found
                .missing
                .iter()
                .map(|(which, why)| format!("{}: {why}", places::label(*which)))
                .collect::<Vec<_>>()
                .join("; ");
            return Scan {
                problem: Some(format!("No folder to look in ({why})")),
                ..Scan::default()
            };
        }
        let look: Vec<files::Where> = found
            .places
            .iter()
            .map(|p| files::Where {
                label: places::label(p.which).to_string(),
                dir: PathBuf::from(&p.dir),
            })
            .collect();
        // the files rules add are offered and ticked, whatever the block's own patterns say
        let mut include = f.include.clone();
        let mut preselect = f.preselect.clone();
        for file in &added_files {
            if !include.contains(file) {
                include.push(file.clone());
            }
            if !preselect.contains(file) {
                preselect.push(file.clone());
            }
        }
        files::scan_places(root, &look, &include, &preselect, Limits::default())
    };
    let subject = match captured.subject {
        cap::Subject::Window => "window",
        cap::Subject::Display => "display",
        cap::Subject::AllDisplays => "all_displays",
    };
    let save_folder = store
        .load_or_default::<cargoframe_config::Safe>()
        .ok()
        .and_then(|s| s.folder);
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
        local_folder: save_folder.as_deref(),
        scan: &scan,
    };
    let view = view::build(&inputs);
    let png = captured
        .frame
        .to_png()
        .map_err(|e| PrepareError::Capture(e.to_string()))?;
    Ok(Session::new(png, view).with_files_root(files_root.map(|r| r.display().to_string())))
}

#[cfg(test)]
pub(crate) mod tests {
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
            },
        )
        .unwrap();

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
}
