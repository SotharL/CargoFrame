use crate::connections::Connections;
use crate::session::Sessions;
use crate::timing::Timings;
use crate::transfer::Entry;
use crate::updates::Updates;
use crate::warm::Warm;
use cargoframe_bundle::{Sweep, Workspace};
use cargoframe_config::Store;
use std::collections::BTreeSet;
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
    /// The captures the CargoFrame windows are showing, one per window.
    pub sessions: Sessions,
    /// Where each capture keeps its temporary copies while its window is open.
    pub workspace: Workspace,
    /// Which hidden windows are being kept warm (Memory mode).
    pub warm: Warm,
    /// The last timings, for Settings, General.
    pub timings: Timings,
    /// The screen recording that is running, if any.
    pub recording: crate::video::Recording,
    /// While a trigger records: the sign on screen and the program in focus.
    watch: Mutex<Option<crate::watch::Watch>>,
    /// The shortcuts held down right now (see `hold`).
    pub holds: crate::hold::Holds,
    /// Looking for, downloading and installing a newer version.
    pub updates: std::sync::Arc<Updates>,
    /// The program that had focus at the last capture, so the layout builder can show what a
    /// Files block would find without asking again. Kept in memory only.
    last_program: Mutex<Option<String>>,
    /// A capture is being taken; a second shortcut press meanwhile is ignored.
    busy: AtomicBool,
    /// The captures whose share is being sent; a second press of a sharing button of the same
    /// window meanwhile is refused. Windows send independently of each other.
    sharing: Mutex<BTreeSet<u64>>,
    /// The captures whose window was closed during a share: stop retrying and say how it ended.
    share_cancelled: Mutex<BTreeSet<u64>>,
    /// How the shortcuts work here, and the ones that could not be set, by trigger id.
    shortcut_status: Mutex<cargoframe_shortcuts::Status>,
    /// What was handed to a Wayland desktop (Linux only).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub desktop_keys: crate::desktop_keys::DesktopKeys,
    /// The settings file the person opened to import, read and checked, waiting for their
    /// choice of what to bring in. The window never names a file: it only ever sees this.
    pending_import: Mutex<Option<Vec<Entry>>>,
}

impl AppState {
    /// Keep the watch of the recording that has just started.
    pub fn set_watch(&self, w: crate::watch::Watch) {
        *self.watch.lock().unwrap_or_else(|e| e.into_inner()) = Some(w);
    }

    /// What the watch of the recording has seen so far, leaving it running.
    pub fn peek_watch(&self) -> Option<crate::watch::Watched> {
        self.watch
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|w| w.peek())
    }

    /// The watch of the recording, taken away (its sign goes when it is finished or dropped).
    pub fn take_watch(&self) -> Option<crate::watch::Watch> {
        self.watch.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

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
            recording: crate::video::Recording::new(),
            watch: Mutex::new(None),
            holds: crate::hold::Holds::default(),
            updates: std::sync::Arc::new(Updates::new(
                std::sync::Arc::new(cargoframe_net::UreqHttp::new()),
                std::sync::Arc::new(crate::secrets::KeyringStore),
            )),
            busy: AtomicBool::new(false),
            sharing: Mutex::new(BTreeSet::new()),
            share_cancelled: Mutex::new(BTreeSet::new()),
            shortcut_status: Mutex::new(cargoframe_shortcuts::Status::default()),
            desktop_keys: crate::desktop_keys::DesktopKeys::default(),
            pending_import: Mutex::new(None),
        }
    }

    /// Delete the capture folders that no open window owns and that are older than
    /// `older_than` (a folder made a moment ago may belong to a window still opening).
    pub fn sweep_captures(&self, older_than: Duration) -> Sweep {
        let keep: Vec<_> = self
            .sessions
            .all()
            .iter()
            .filter_map(|s| s.dir_path())
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

    /// Claim the right to send the capture `id`. False if its share is already running.
    pub fn try_begin_share(&self, id: u64) -> bool {
        if !lock(&self.sharing).insert(id) {
            return false;
        }
        lock(&self.share_cancelled).remove(&id);
        true
    }

    pub fn end_share(&self, id: u64) {
        lock(&self.sharing).remove(&id);
        lock(&self.share_cancelled).remove(&id);
    }

    /// The window of the capture `id` was closed: its share stops retrying.
    pub fn cancel_share(&self, id: u64) {
        if lock(&self.sharing).contains(&id) {
            lock(&self.share_cancelled).insert(id);
        }
    }

    pub fn share_cancelled(&self, id: u64) -> bool {
        lock(&self.share_cancelled).contains(&id)
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

    pub fn set_shortcut_status(&self, status: cargoframe_shortcuts::Status) {
        *self
            .shortcut_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = status;
    }

    /// Change the status after the fact (a desktop that answers later). Linux only.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn update_shortcut_status(&self, f: impl FnOnce(&mut cargoframe_shortcuts::Status)) {
        f(&mut self
            .shortcut_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()));
    }

    pub fn shortcut_status(&self) -> cargoframe_shortcuts::Status {
        self.shortcut_status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_config::Dirs;

    fn state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Dirs::under(dir.path()));
        (
            AppState::new(store, false, crate::session::tests::no_connections()),
            dir,
        )
    }

    #[test]
    fn two_windows_send_at_the_same_time_but_one_window_does_not_send_twice() {
        let (s, _d) = state();
        assert!(s.try_begin_share(1));
        assert!(s.try_begin_share(2), "another window can send meanwhile");
        assert!(!s.try_begin_share(1), "this one is already sending");
        s.end_share(1);
        assert!(s.try_begin_share(1));
    }

    #[test]
    fn closing_a_window_stops_only_its_own_share() {
        let (s, _d) = state();
        assert!(s.try_begin_share(1));
        assert!(s.try_begin_share(2));
        s.cancel_share(1);
        assert!(s.share_cancelled(1));
        assert!(!s.share_cancelled(2));
        // a share that is not running has nothing to cancel, so the next one starts clean
        s.end_share(1);
        s.cancel_share(1);
        assert!(!s.share_cancelled(1));
        assert!(s.try_begin_share(1));
        assert!(!s.share_cancelled(1));
    }
}
