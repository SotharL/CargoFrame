//! What happens to a window's web view when the window is closed (Settings, General, Memory).
//!
//! - **Low memory:** the window is destroyed, so the web view is freed at once.
//! - **Balanced:** the window is hidden and kept for a few minutes. Opening it again in that
//!   time is instant; after it, the window is destroyed.
//! - **Always warm:** the window is only ever hidden.
//!
//! The decision and the "was it opened again meanwhile" bookkeeping are plain code so they can
//! be tested without a window.

use cargoframe_config::MemoryMode;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// How long Balanced keeps a hidden window.
pub const WARM_FOR: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnClose {
    /// Let the window close (and its web view go away).
    Destroy,
    /// Hide it. `release_after` is when to destroy it if nobody opened it again; `None` keeps
    /// it for good.
    Hide { release_after: Option<Duration> },
}

pub fn on_close(mode: MemoryMode) -> OnClose {
    match mode {
        MemoryMode::LowMemory => OnClose::Destroy,
        MemoryMode::Balanced => OnClose::Hide {
            release_after: Some(WARM_FOR),
        },
        MemoryMode::AlwaysWarm => OnClose::Hide {
            release_after: None,
        },
    }
}

/// Counts how often each window was hidden or shown, so a release timer started for one hide
/// does nothing if the window was shown (or hidden again) since.
#[derive(Default)]
pub struct Warm {
    epochs: Mutex<HashMap<String, u64>>,
}

impl Warm {
    pub fn new() -> Self {
        Self::default()
    }

    fn bump(&self, label: &str) -> u64 {
        let mut map = self.epochs.lock().unwrap_or_else(|e| e.into_inner());
        let n = map.entry(label.to_string()).or_insert(0);
        *n += 1;
        *n
    }

    /// The window was hidden. The returned ticket belongs to this hide.
    pub fn hidden(&self, label: &str) -> u64 {
        self.bump(label)
    }

    /// The window was shown, or created: any release timer in flight is void.
    pub fn shown(&self, label: &str) {
        self.bump(label);
    }

    /// Whether the hide that issued `ticket` is still the latest thing that happened.
    pub fn still_hidden(&self, label: &str, ticket: u64) -> bool {
        self.epochs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(label)
            .copied()
            == Some(ticket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_memory_mode_has_its_own_close_behavior() {
        assert_eq!(on_close(MemoryMode::LowMemory), OnClose::Destroy);
        assert_eq!(
            on_close(MemoryMode::Balanced),
            OnClose::Hide {
                release_after: Some(Duration::from_secs(300))
            }
        );
        assert_eq!(
            on_close(MemoryMode::AlwaysWarm),
            OnClose::Hide {
                release_after: None
            }
        );
    }

    #[test]
    fn a_release_is_void_once_the_window_was_shown_again() {
        let warm = Warm::new();
        let ticket = warm.hidden("capture");
        assert!(warm.still_hidden("capture", ticket));
        warm.shown("capture");
        assert!(!warm.still_hidden("capture", ticket));
    }

    #[test]
    fn only_the_latest_hide_may_release() {
        let warm = Warm::new();
        let first = warm.hidden("capture");
        warm.shown("capture");
        let second = warm.hidden("capture");
        assert!(!warm.still_hidden("capture", first));
        assert!(warm.still_hidden("capture", second));
    }

    #[test]
    fn windows_do_not_affect_each_other() {
        let warm = Warm::new();
        let capture = warm.hidden("capture");
        warm.shown("settings");
        assert!(warm.still_hidden("capture", capture));
    }
}
