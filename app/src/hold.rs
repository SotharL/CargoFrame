//! Holding a shortcut. With continuous recording on (Settings, Recording), a shortcut held for
//! two seconds starts a recording that goes on, and held again stops it; a short press does what
//! a press always does (with a continuous recording running, a video trigger opens the window
//! with its last seconds). So the press is acted on when the key comes up, not when it goes down.
//! With the setting off, nothing waits: a press is acted on at once, as before.
//!
//! Wayland desktops only tell CargoFrame that a shortcut was pressed, never that it was let go,
//! so there a continuous recording is started and stopped from the tray menu.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// What a key going down or up means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Nothing yet (the key went down; wait to see whether it is held).
    Wait,
    /// A press: do what the trigger does.
    Press,
}

/// The shortcuts down right now, by trigger id, and whether each was already taken as held.
#[derive(Default)]
pub struct Holds {
    down: Mutex<BTreeMap<String, (Instant, bool)>>,
}

impl Holds {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, (Instant, bool)>> {
        self.down.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The key of `id` went down at `now`. A key held down repeats this: only the first counts.
    /// Returns whether it is a new press (so the caller starts waiting for the hold).
    pub fn down(&self, id: &str, now: Instant) -> bool {
        let mut down = self.lock();
        if down.contains_key(id) {
            return false;
        }
        down.insert(id.to_string(), (now, false));
        true
    }

    /// The time of the hold has passed for the press that went down at `since`: if that press
    /// is still down and not taken yet, it is a hold now.
    pub fn held(&self, id: &str, since: Instant) -> bool {
        let mut down = self.lock();
        match down.get_mut(id) {
            Some((at, taken)) if *at == since && !*taken => {
                *taken = true;
                true
            }
            _ => false,
        }
    }

    /// The key of `id` came up: a press, unless it was taken as a hold.
    pub fn up(&self, id: &str) -> Act {
        match self.lock().remove(id) {
            Some((_, true)) => Act::Wait,
            Some((_, false)) => Act::Press,
            // up without down (the shortcut was registered while it was held)
            None => Act::Wait,
        }
    }

    /// When the press that went down at `since` became a hold.
    pub fn after(since: Instant, hold_ms: u64) -> Instant {
        since + Duration::from_millis(hold_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_press_is_a_press_when_it_comes_up_and_a_long_one_a_hold() {
        let h = Holds::default();
        let t0 = Instant::now();
        assert!(h.down("bug", t0));
        // the key repeats while held: still the same press
        assert!(!h.down("bug", t0 + Duration::from_millis(500)));
        assert_eq!(h.up("bug"), Act::Press);

        let t1 = t0 + Duration::from_secs(5);
        assert!(h.down("bug", t1));
        assert!(h.held("bug", t1));
        // taken once only
        assert!(!h.held("bug", t1));
        assert_eq!(h.up("bug"), Act::Wait);

        // a timer from an older press does nothing to a newer one
        let t2 = t1 + Duration::from_secs(5);
        assert!(h.down("bug", t2));
        assert!(!h.held("bug", t1));
        assert_eq!(h.up("bug"), Act::Press);
        // another trigger is its own
        assert_eq!(h.up("other"), Act::Wait);
        assert_eq!(Holds::after(t0, 2000), t0 + Duration::from_secs(2));
    }
}
