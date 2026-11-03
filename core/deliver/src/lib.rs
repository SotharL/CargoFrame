//! Hand a capture to a chat window by pasting it.
//!
//! Some destinations have no way in except their own window: a Claude chat takes no upload
//! from outside, and a link can open a chat but cannot fill it. So CargoFrame does what a
//! person would do: it opens the chat, waits until the right window is really in front, pastes
//! the screenshot and the files (as files), pastes the text, and presses Enter.
//!
//! This is done carefully, because it types into another program:
//!
//! * Nothing is pasted until the window in front is the right one: Claude's own app, or a
//!   known web browser showing a Claude page. [`Target::accepts`] decides.
//! * Before every paste and before Enter the window in front is checked again, and it has to be
//!   the same window as before. If the person clicked away, delivery stops on the spot.
//! * Everything is behind the [`Desk`] trait, so the whole sequence is tested with a pretend
//!   desktop; the real one is in [`SystemDesk`].

mod error;
mod system;
#[cfg(windows)]
mod win;

pub use error::DeliverError;
pub use system::{System, SystemDesk};

use std::path::PathBuf;
use std::time::Duration;

/// A window on the desktop, as far as delivery needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub title: String,
    /// The program's file name without folder or extension, lower case (`claude`, `chrome`).
    pub program: Option<String>,
}

/// Which kind of window must be in front before anything is pasted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The Claude desktop app.
    App,
    /// A web browser showing a Claude page.
    Browser,
}

/// Programs that show web pages. A window titled "claude" in a terminal is not one of these, so
/// a terminal running Claude Code is never typed into.
const BROWSERS: &[&str] = &[
    "chrome",
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "msedge",
    "microsoft-edge",
    "firefox",
    "brave",
    "brave-browser",
    "opera",
    "vivaldi",
    "arc",
    "zen",
    "librewolf",
    "waterfox",
    "floorp",
    "thorium",
    "safari",
];

impl Target {
    pub fn accepts(self, w: &Window) -> bool {
        let Some(program) = w.program.as_deref() else {
            return false;
        };
        match self {
            Target::App => program == "claude" && !w.title.trim().is_empty(),
            Target::Browser => {
                BROWSERS.contains(&program) && w.title.to_lowercase().contains("claude")
            }
        }
    }
}

/// A key press the delivery needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Ctrl+V
    Paste,
    Enter,
}

/// The desktop, as delivery sees it. Everything with a side effect is here.
pub trait Desk {
    /// The window that has the focus now.
    fn foreground(&self) -> Result<Option<Window>, String>;
    /// Whether [`put_files`](Desk::put_files) works on this system.
    fn can_paste_files(&self) -> bool {
        true
    }
    /// Put these files on the clipboard, as if they had been copied in the file manager.
    fn put_files(&self, files: &[PathBuf]) -> Result<(), String>;
    fn put_text(&self, text: &str) -> Result<(), String>;
    /// Press a key in the window that has the focus.
    fn press(&self, key: Key) -> Result<(), String>;
    fn sleep(&self, d: Duration);
}

/// How long to wait for what. The windows cannot be asked whether the page has loaded or the
/// upload has finished, so these are waits, and the person can lengthen them for a slow computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// How long Claude has to come to the front after its link was opened.
    pub front_timeout: Duration,
    /// After it is in front: the chat loads and the message box takes the focus.
    pub settle: Duration,
    /// After a paste: the program takes in what was pasted.
    pub beat: Duration,
    /// Waiting for attachments to upload: a base, plus a time per megabyte, up to a limit.
    pub upload_base: Duration,
    pub upload_per_mb: Duration,
    pub upload_max: Duration,
    /// How often the front window is looked at while waiting.
    pub poll: Duration,
}

impl Timing {
    pub fn for_target(target: Target, extra: Duration) -> Self {
        Self {
            front_timeout: Duration::from_secs(25),
            settle: match target {
                Target::App => Duration::from_millis(2500),
                Target::Browser => Duration::from_millis(4000),
            } + extra,
            beat: Duration::from_millis(350),
            upload_base: Duration::from_millis(2000),
            upload_per_mb: Duration::from_millis(1500),
            upload_max: Duration::from_secs(45),
            poll: Duration::from_millis(150),
        }
    }

    fn upload_wait(&self, bytes: u64) -> Duration {
        let mb = bytes.div_ceil(1024 * 1024);
        (self.upload_base + self.upload_per_mb * u32::try_from(mb).unwrap_or(u32::MAX))
            .min(self.upload_max)
    }
}

/// One delivery.
#[derive(Debug, Clone)]
pub struct Delivery {
    /// The link that opens the chat.
    pub open_url: String,
    pub target: Target,
    /// Files to attach, the screenshot first. Each one must exist.
    pub files: Vec<PathBuf>,
    pub text: String,
    /// Press Enter at the end, so the message is sent. Off: it waits in the message box.
    pub submit: bool,
    pub timing: Timing,
}

/// What was done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    /// The title of the window it was pasted into.
    pub window: String,
    pub files: usize,
    /// Files that could not be attached on this system.
    pub skipped_files: usize,
    pub text: bool,
    pub submitted: bool,
}

/// Does the whole job against a real or pretend desktop.
pub trait Deliver: Send + Sync {
    fn deliver(
        &self,
        delivery: &Delivery,
        open: &dyn Fn(&str) -> Result<(), String>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Delivered, DeliverError>;
}

/// The sequence. See the module notes for the safety rules.
pub fn deliver(
    desk: &dyn Desk,
    open: &dyn Fn(&str) -> Result<(), String>,
    d: &Delivery,
    cancelled: &dyn Fn() -> bool,
) -> Result<Delivered, DeliverError> {
    if d.files.is_empty() && d.text.trim().is_empty() {
        return Err(DeliverError::NothingToSend);
    }
    let mut total_bytes = 0u64;
    for f in &d.files {
        let meta = std::fs::metadata(f)
            .ok()
            .filter(|m| m.is_file())
            .ok_or_else(|| DeliverError::MissingFile(f.display().to_string()))?;
        total_bytes += meta.len();
    }
    let attach = d.files.is_empty() || desk.can_paste_files();
    let timing = d.timing;

    open(&d.open_url).map_err(DeliverError::Open)?;
    let window = wait_for_front(desk, d.target, &timing, cancelled)?;

    // The chat loads and the message box takes the focus. Watch the window the whole time.
    watch(
        desk,
        d.target,
        &window,
        timing.settle,
        &timing,
        "wait",
        cancelled,
    )?;

    let mut pasted_files = 0;
    if attach && !d.files.is_empty() {
        guard(desk, d.target, &window, "attach")?;
        desk.put_files(&d.files).map_err(DeliverError::Clipboard)?;
        desk.sleep(timing.beat);
        guard(desk, d.target, &window, "attach")?;
        desk.press(Key::Paste).map_err(DeliverError::Keys)?;
        pasted_files = d.files.len();
        desk.sleep(timing.beat);
    }

    let pasted_text = !d.text.trim().is_empty();
    if pasted_text {
        guard(desk, d.target, &window, "write")?;
        desk.put_text(&d.text).map_err(DeliverError::Clipboard)?;
        desk.sleep(timing.beat);
        guard(desk, d.target, &window, "write")?;
        desk.press(Key::Paste).map_err(DeliverError::Keys)?;
        desk.sleep(timing.beat);
    }

    // Enter while an upload is still running would be ignored, so wait for it.
    if pasted_files > 0 {
        watch(
            desk,
            d.target,
            &window,
            timing.upload_wait(total_bytes),
            &timing,
            "upload",
            cancelled,
        )?;
    }

    if d.submit {
        guard(desk, d.target, &window, "send")?;
        desk.press(Key::Enter).map_err(DeliverError::Keys)?;
    }
    Ok(Delivered {
        window: window.title,
        files: pasted_files,
        skipped_files: if attach { 0 } else { d.files.len() },
        text: pasted_text,
        submitted: d.submit,
    })
}

fn wait_for_front(
    desk: &dyn Desk,
    target: Target,
    timing: &Timing,
    cancelled: &dyn Fn() -> bool,
) -> Result<Window, DeliverError> {
    let mut waited = Duration::ZERO;
    let mut seen: Option<String> = None;
    loop {
        if cancelled() {
            return Err(DeliverError::Cancelled);
        }
        match desk.foreground().map_err(DeliverError::Desk)? {
            Some(w) if target.accepts(&w) => return Ok(w),
            Some(w) => seen = Some(w.title),
            None => {}
        }
        if waited >= timing.front_timeout {
            return Err(DeliverError::NotInFront {
                target,
                waited,
                seen,
            });
        }
        desk.sleep(timing.poll);
        waited += timing.poll;
    }
}

/// The window in front must still be the one that was checked.
fn guard(
    desk: &dyn Desk,
    target: Target,
    window: &Window,
    stage: &'static str,
) -> Result<(), DeliverError> {
    match desk.foreground().map_err(DeliverError::Desk)? {
        Some(w) if w.id == window.id && target.accepts(&w) => Ok(()),
        other => Err(DeliverError::LostFocus {
            stage,
            now: other.map(|w| w.title),
        }),
    }
}

/// Wait `total`, looking at the front window every `poll`.
fn watch(
    desk: &dyn Desk,
    target: Target,
    window: &Window,
    total: Duration,
    timing: &Timing,
    stage: &'static str,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DeliverError> {
    let mut waited = Duration::ZERO;
    while waited < total {
        if cancelled() {
            return Err(DeliverError::Cancelled);
        }
        let step = timing
            .poll
            .max(Duration::from_millis(50))
            .min(total - waited);
        desk.sleep(step);
        waited += step;
        guard(desk, target, window, stage)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
