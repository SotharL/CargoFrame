//! The `system.*` values of a capture: what this computer is (read once, in the background after
//! the program starts, so a capture never waits for it) and its screens (read at each capture,
//! as one can be plugged in or out).

use cargoframe_capture::system::{self, System};
use std::sync::OnceLock;

static SYSTEM: OnceLock<System> = OnceLock::new();

/// Read the computer now, off the main thread, so the first capture has it ready.
pub fn warm() {
    std::thread::Builder::new()
        .name("system-info".into())
        .spawn(|| {
            let _ = SYSTEM.get_or_init(system::read);
        })
        .ok();
}

/// The values, by name. A capture made before the computer was read waits for that once.
pub fn values() -> Vec<(&'static str, String)> {
    SYSTEM.get_or_init(system::read).values(&system::screens())
}
