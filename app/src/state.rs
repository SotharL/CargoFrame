use crate::connections::Connections;
use crate::session::Sessions;
use crate::timing::Timings;
use crate::transfer::Entry;
use crate::updates::Updates;
use crate::warm::Warm;
use cargoframe_bundle::{Sweep, Workspace};
use cargoframe_config::Store;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Shared by the tray, the commands and the windows.
pub struct AppState {
    pub store: Store,
    /// The services CargoFrame can share to, and their sign-ins.
    pub connections: Connections,
    /// "Pause triggers" in the tray menu.
    paused: AtomicBool,
    /// False when a Linux desktop has no tray host.
    pub tray_available: bool,
    /// The capture the CargoFrame window is showing.
    pub sessions: Sessions,
    /// Where each capture keeps its temporary copies while its window is open.
    pub workspace: Workspace,
    /// Which hidden windows are being kept warm (Memory mode).
    pub warm: Warm,
    /// The last timings, for Settings, General.
    pub timings: Timings,
    /// Looking for, downloading and installing a newer version.
    pub updates: std::sync::Arc<Updates>,
    /// The program that had focus at the last capture, so the layout builder can show what a
    /// Files block would find without asking again. Kept in memory only.
    last_program: Mutex<Option<String>>,
    /// A capture is being taken; a second shortcut press meanwhile is ignored.
    busy: AtomicBool,
    /// A share is being sent; a second press of a sharing button meanwhile is refused.
    sharing: AtomicBool,
    /// The CargoFrame window was closed during a share: stop retrying and say how it ended.
    share_cancelled: AtomicBool,
    /// Shortcuts that could not be registered, by trigger id, with the reason.
    shortcut_errors: Mutex<BTreeMap<String, String>>,
    /// The settings file the person opened to import, read and checked, waiting for their
    /// choice of what to bring in. The window never names a file: it only ever sees this.
    pending_import: Mutex<Option<Vec<Entry>>>,
}

impl AppState {
    pub fn new(store: Store, tray_available: bool, connections: Connections) -> Self {
        let workspace = Workspace::new(store.dirs().data().join("captures"));
        Self {
            workspace,
            store,
            connections,
            paused: AtomicBool::new(false),
            last_program: Mutex::new(None),
            tray_available,
            sessions: Sessions::new(),
            warm: Warm::new(),
            timings: Timings::new(),
            updates: std::sync::Arc::new(Updates::new(
                std::sync::Arc::new(cargoframe_net::UreqHttp::new()),
                std::sync::Arc::new(crate::secrets::KeyringStore),
            )),
            busy: AtomicBool::new(false),
            sharing: AtomicBool::new(false),
            share_cancelled: AtomicBool::new(false),
            shortcut_errors: Mutex::new(BTreeMap::new()),
            pending_import: Mutex::new(None),
        }
    }

    /// Delete the capture folders that no open window owns and that are older than
    /// `older_than` (a folder made a moment ago may belong to a window still opening).
    pub fn sweep_captures(&self, older_than: Duration) -> Sweep {
        let keep: Vec<_> = self
            .sessions
            .current()
            .and_then(|s| s.dir_path())
            .into_iter()
            .collect();
        self.workspace.sweep(&keep, older_than)
    }

    /// Remember the program of the capture that was just taken.
    pub fn set_last_program(&self, program: Option<String>) {
        if program.is_some() {
            *self.last_program.lock().unwrap_or_else(|e| e.into_inner()) = program;
        }
    }

    pub fn last_program(&self) -> Option<String> {
        self.last_program
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Claim the right to capture. False if a capture is already running.
    pub fn try_begin_capture(&self) -> bool {
        !self.busy.swap(true, Ordering::AcqRel)
    }

    pub fn end_capture(&self) {
        self.busy.store(false, Ordering::Release);
    }

    /// Claim the right to send. False if a share is already running.
    pub fn try_begin_share(&self) -> bool {
        if self.sharing.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.share_cancelled.store(false, Ordering::Release);
        true
    }

    pub fn end_share(&self) {
        self.sharing.store(false, Ordering::Release);
    }

    pub fn cancel_share(&self) {
        self.share_cancelled.store(true, Ordering::Release);
    }

    pub fn share_cancelled(&self) -> bool {
        self.share_cancelled.load(Ordering::Acquire)
    }

    pub fn set_pending_import(&self, entries: Option<Vec<Entry>>) {
        *self
            .pending_import
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = entries;
    }

    /// The opened settings file, once: choosing what to import uses it up.
    pub fn take_pending_import(&self) -> Option<Vec<Entry>> {
        self.pending_import
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub fn set_shortcut_errors(&self, errors: BTreeMap<String, String>) {
        *self
            .shortcut_errors
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = errors;
    }

    pub fn shortcut_errors(&self) -> BTreeMap<String, String> {
        self.shortcut_errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
