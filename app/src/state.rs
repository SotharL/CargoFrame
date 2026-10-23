use cargoframe_config::Store;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared by the tray, the commands and the windows.
pub struct AppState {
    pub store: Store,
    /// "Pause triggers" in the tray menu.
    paused: AtomicBool,
    /// False when a Linux desktop has no tray host.
    pub tray_available: bool,
}

impl AppState {
    pub fn new(store: Store, tray_available: bool) -> Self {
        Self {
            store,
            paused: AtomicBool::new(false),
            tray_available,
        }
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
}
