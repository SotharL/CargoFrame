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
    /// The texts the window drew over the recording, by name (`keys`, `clock`).
    clip_layers: Mutex<BTreeMap<String, ClipLayer>>,
    /// The folder the ticked files are read from, as worked out at capture time: the highest
    /// folder any Files block looks in (the project folder unless a block looks higher). Every
    /// file path the window shows is relative to it. Kept here so a share never trusts a folder
    /// name sent back by the window.
    pub files_root: Option<String>,
    /// The folder this capture keeps copies in (the screenshot, the ticked files), made when a
    /// destination first needs real files. It is deleted when the capture is closed.
    dir: Mutex<Option<CaptureDir>>,
    /// Files the person added to a Files block by hand, by block id. Each is copied into the
    /// capture's folder at once, under `added/<name>`: everything that reads a ticked file finds
    /// that copy first, so the file is never read from where it was chosen again.
    added: Mutex<BTreeMap<String, Vec<files::FileEntry>>>,
}

/// The name a file added by hand has in the window and in a zip.
pub const ADDED: &str = "added";
/// The largest file that can be added by hand.
pub const MAX_ADDED_BYTES: u64 = 50 * 1024 * 1024;
/// The most a log viewer reads of a file: its end, where the latest lines are.
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

/// The end of a file: all of it when it is at most `max` bytes, else its last `max` bytes from
/// the first whole line. Text that is not UTF-8 is shown as best it can be.
pub fn read_end(path: &Path, max: u64) -> std::io::Result<LogText> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let size = f.metadata()?.len();
    let cut = size > max;
    if cut {
        f.seek(SeekFrom::Start(size - max))?;
    }
    let mut bytes = Vec::with_capacity(size.min(max) as usize);
    f.take(max).read_to_end(&mut bytes)?;
    if cut {
        // the first line is only part of one
        if let Some(i) = bytes.iter().position(|&b| b == b'\n') {
            bytes.drain(..=i);
        }
    }
    Ok(LogText {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        cut,
        size,
    })
}

/// What a log viewer shows of a file.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct LogText {
    pub text: String,
    /// Only the end of the file is shown: it is larger than [`MAX_LOG_BYTES`].
    pub cut: bool,
    pub size: u64,
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
            clip_layers: Mutex::new(BTreeMap::new()),
            files_root,
            dir: Mutex::new(None),
            added: Mutex::new(BTreeMap::new()),
        }
    }

    /// The files added by hand to Files block `block_id`.
    pub fn added(&self, block_id: &str) -> Vec<files::FileEntry> {
        self.added
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(block_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Whether a Files block of this capture offers `path` (found, or added by hand).
    pub fn offers(&self, block_id: &str, path: &str) -> bool {
        let found = self
            .view
            .files
            .get(block_id)
            .is_some_and(|f| f.entries.iter().any(|e| e.path == path));
        found || self.added(block_id).iter().any(|e| e.path == path)
    }

    /// Add a file the person chose to Files block `block_id`: copied into the capture's folder
    /// now, listed as `added/<name>` (a number added when the name is taken). Gives its entry.
    pub fn add_file(
        &self,
        workspace: &Workspace,
        block_id: &str,
        from: &Path,
    ) -> Result<files::FileEntry, String> {
        if !self.view.files.contains_key(block_id) {
            return Err("That Files block is not in this capture".into());
        }
        let name = from
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.trim().is_empty())
            .ok_or("That is not a file")?;
        let mut added = self.added.lock().unwrap_or_else(|e| e.into_inner());
        let taken = |p: &str| {
            added.values().flatten().any(|e| e.path == p)
                || self
                    .view
                    .files
                    .values()
                    .any(|f| f.entries.iter().any(|e| e.path == p))
        };
        let (stem, ext) = match name.rfind('.') {
            Some(i) if i > 0 => (&name[..i], &name[i..]),
            _ => (name.as_str(), ""),
        };
        let mut path = format!("{ADDED}/{name}");
        let mut n = 2;
        while taken(&path) {
            path = format!("{ADDED}/{stem} ({n}){ext}");
            n += 1;
        }
        let copy = self
            .with_dir(workspace, |dir| dir.adopt(from, &path, MAX_ADDED_BYTES))
            .map_err(|e| format!("The capture's folder could not be used: {e}"))?
            .map_err(|e| format!("\u{201c}{name}\u{201d} cannot be added: {e}"))?;
        let size = std::fs::metadata(&copy).map(|m| m.len()).unwrap_or(0);
        let entry = files::FileEntry {
            path,
            size,
            selected: true,
        };
        added
            .entry(block_id.to_string())
            .or_default()
            .push(entry.clone());
        Ok(entry)
    }

    /// The text of a file a Files block offers, for a log viewer: this capture's copy if it has
    /// one, else the file in the project; only its end when it is large. Never a file the
    /// capture did not offer.
    pub fn read_offered(&self, path: &str) -> Result<LogText, String> {
        let offered = self.view.files.keys().any(|block| self.offers(block, path));
        if !offered {
            return Err("That file is not one this capture offers".into());
        }
        let copy = self
            .dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|d| d.staged(path));
        let real = match copy {
            Some(c) => c,
            None => {
                let root = self
                    .files_root
                    .as_deref()
                    .ok_or("There is no folder to read the file from")?;
                cargoframe_bundle::inside(Path::new(root), path).map_err(|e| e.to_string())?
            }
        };
        read_end(&real, MAX_LOG_BYTES).map_err(|e| format!("The file could not be read: {e}"))
    }

    /// This capture is a recording: tell the window about it.
    pub fn with_clip(mut self, clip: Clip) -> Self {
        self.view.video = Some(VideoView {
            width: clip.width,
            height: clip.height,
            duration_ms: clip.duration_ms,
            bytes: clip.bytes,
            audio: clip.has_audio,
            crop: None,
            follow: Vec::new(),
            keys: Vec::new(),
            started_at_ms: None,
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

    /// Keep (or, with `None`, forget) a text the window drew over the recording.
    pub fn set_clip_layer(&self, name: &str, layer: Option<ClipLayer>) {
        let mut all = self.clip_layers.lock().unwrap_or_else(|e| e.into_inner());
        match layer {
            Some(l) => {
                all.insert(name.to_string(), l);
            }
            None => {
                all.remove(name);
            }
        }
    }

    /// A text the window drew over the recording, if it sent it.
    pub fn clip_layer(&self, name: &str) -> Option<ClipLayer> {
        self.clip_layers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .cloned()
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
    /// How long before the capture the clip ends: a continuous recording hands out only the
    /// pieces it has finished.
    pub ends_before_ms: u64,
}

/// A text the window drew over a recording: a see-through blank picture, and each line's
/// picture with when it shows (on the clip's timeline), all the same size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipLayer {
    pub blank: Vec<u8>,
    pub lines: Vec<((u64, u64), Vec<u8>)>,
}

/// Now, in milliseconds since 1970 (UTC).
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
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
    let started = session
        .view
        .captured_at_ms
        .checked_sub(info.ends_before_ms + info.duration_ms);
    let mut session = session.with_clip(Clip {
        path: moved,
        width: info.width,
        height: info.height,
        duration_ms: info.duration_ms,
        bytes: info.bytes,
        has_audio: info.has_audio,
    });
    if let Some(v) = session.view.video.as_mut() {
        v.started_at_ms = started;
    }
    Ok(session)
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
            // `--set labels=crash,gpu`: the tags, replacing the ones it started with
            Some(cargoframe_layout::BlockKind::Tags(_)) => {
                let (tags, over) = cargoframe_layout::split_tags([value.as_str()]);
                view.tags.insert(id.clone(), view::TagsState { tags, over });
            }
            _ => unknown.push(id.as_str()),
        }
    }
    if !unknown.is_empty() {
        view.notes.push(format!(
            "Another program sent values for {}, which this layout has no text box, dropdown or tags for",
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
    let mut log_times = Vec::new();
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
        // how the lines of the files they found write their time, for the log viewers
        for (id, rule) in &loaded.items {
            if rule.log_time.trim().is_empty() {
                continue;
            }
            for file in report.found.get(id).into_iter().flatten() {
                log_times.push(cargoframe_window::view::LogTimeView {
                    file: file.clone(),
                    format: rule.log_time.trim().to_string(),
                    rule: rule.name.clone(),
                });
            }
        }
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
    let mut view = view::build(&inputs);
    view.captured_at_ms = unix_ms();
    view.log_times = log_times;
    let png = captured
        .frame
        .to_png()
        .map_err(|e| PrepareError::Capture(e.to_string()))?;
    Ok(Session::new(png, view).with_files_root(files_root.map(|r| r.display().to_string())))
}

#[cfg(test)]
#[path = "session_tests.rs"]
pub(crate) mod tests;
