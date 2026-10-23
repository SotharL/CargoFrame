use crate::connections::Connections;
use crate::session::Sessions;
use crate::timing::Timings;
use crate::warm::Warm;
use cargoframe_config::Store;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

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
    /// Which hidden windows are being kept warm (Memory mode).
    pub warm: Warm,
    /// The last timings, for Settings, General.
    pub timings: Timings,
    /// A capture is being taken; a second shortcut press meanwhile is ignored.
    busy: AtomicBool,
    /// A share is being sent; a second press of a sharing button meanwhile is refused.
    sharing: AtomicBool,
    /// The CargoFrame window was closed during a share: stop retrying and say how it ended.
    share_cancelled: AtomicBool,
    /// Shortcuts that could not be registered, by trigger id, with the reason.
    shortcut_errors: Mutex<BTreeMap<String, String>>,
}

impl AppState {
    pub fn new(store: Store, tray_available: bool, connections: Connections) -> Self {
        Self {
            store,
            connections,
            paused: AtomicBool::new(false),
            tray_available,
            sessions: Sessions::new(),
            warm: Warm::new(),
            timings: Timings::new(),
            busy: AtomicBool::new(false),
            sharing: AtomicBool::new(false),
            share_cancelled: AtomicBool::new(false),
            shortcut_errors: Mutex::new(BTreeMap::new()),
        }
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
