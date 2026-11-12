//! The keys pressed while a recording runs, so the clip can show them ("Ctrl+S", "hello ⏎").
//!
//! A layout's Video block asks for it; nothing is read otherwise. While it runs, a thread looks
//! at the keyboard about fifty times a second (no hook into other programs' input) and keeps
//! each press with its time, in memory only, for as long as the recording keeps its pictures.
//! [`captions`] turns the presses into the lines the clip shows, each with when it shows.
//!
//! - Windows: the state of each key (`GetAsyncKeyState`), named with the keyboard layout of the
//!   window in focus.
//! - Linux (X11): the keyboard's state from the X server (`QueryKeymap`), named with its keymap.
//! - Wayland does not let a program see keys pressed in other programs: [`Tracker::start`] says
//!   so, and the recording goes on without them.

mod caption;
#[cfg(windows)]
#[allow(unsafe_code)]
mod win;
#[cfg(target_os = "linux")]
mod x11;

pub use caption::{captions, Caption, LINGER_MS, MAX_CAPTIONS, MAX_CHARS};

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What a key is, as far as showing it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A key that writes: `typed` is what it wrote (with Shift, Caps Lock or AltGr), `base` the
    /// key's own sign, used in a combination ("Ctrl+S", not "Ctrl+s").
    Char { typed: char, base: char },
    /// A key that does not write, by the short name shown ("⏎", "Esc", "F5").
    Named(&'static str),
    /// Ctrl, Alt, Shift or the Windows key alone: it shows only with the key it goes with.
    Modifier,
}

/// The keys held with another one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// The Windows key, or Super.
    pub meta: bool,
}

/// One key pressed, and when (milliseconds on the recording's clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub at_ms: u64,
    pub key: Key,
    pub mods: Mods,
}

/// How often the keyboard is looked at.
pub const POLL: Duration = Duration::from_millis(20);
/// The most presses held at once, whatever happens.
pub const MAX_PRESSES: usize = 20_000;

/// The names of the keys that do not write, shared by both systems.
pub(crate) mod names {
    pub const ENTER: &str = "⏎";
    pub const BACKSPACE: &str = "⌫";
    pub const TAB: &str = "⇥";
    pub const ESCAPE: &str = "Esc";
    pub const DELETE: &str = "Del";
    pub const INSERT: &str = "Ins";
    pub const HOME: &str = "Home";
    pub const END: &str = "End";
    pub const PAGE_UP: &str = "PgUp";
    pub const PAGE_DOWN: &str = "PgDn";
    pub const LEFT: &str = "←";
    pub const UP: &str = "↑";
    pub const RIGHT: &str = "→";
    pub const DOWN: &str = "↓";
    pub const PRINT: &str = "PrtSc";
    pub const F: [&str; 24] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14",
        "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
    ];
}

/// Reads the keyboard: each call says which keys went down since the last one.
pub(crate) trait Poll: Send {
    fn poll(&mut self) -> Vec<(Key, Mods)>;
}

fn open() -> Result<Box<dyn Poll>, String> {
    #[cfg(windows)]
    {
        Ok(Box::new(win::Keyboard::new()))
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Err(WAYLAND.into());
        }
        x11::Keyboard::connect().map(|k| Box::new(k) as Box<dyn Poll>)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("This system does not let CargoFrame see the keys pressed.".into())
    }
}

/// Why the keys cannot be shown on a Wayland desktop.
pub const WAYLAND: &str =
    "A Wayland desktop does not let a program see the keys pressed in other programs, so the recording shows no keys.";

/// Keys are read while it lives.
pub struct Tracker {
    stop: Arc<AtomicBool>,
    presses: Arc<Mutex<VecDeque<Press>>>,
    thread: Option<JoinHandle<()>>,
}

impl Tracker {
    /// Start reading the keyboard. Times are counted from `clock` (when the recording started).
    /// With `keep_ms`, presses older than that are forgotten as it goes (a continuous recording).
    pub fn start(clock: Instant, keep_ms: Option<u64>) -> Result<Tracker, String> {
        let keyboard = open()?;
        Ok(Self::with(keyboard, clock, keep_ms))
    }

    pub(crate) fn with(
        mut keyboard: Box<dyn Poll>,
        clock: Instant,
        keep_ms: Option<u64>,
    ) -> Tracker {
        let stop = Arc::new(AtomicBool::new(false));
        let presses: Arc<Mutex<VecDeque<Press>>> = Arc::default();
        let (flag, held) = (stop.clone(), presses.clone());
        let thread = std::thread::Builder::new()
            .name("recording-keys".into())
            .spawn(move || {
                // what is down when it starts is not a press
                let _ = keyboard.poll();
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(POLL);
                    let new = keyboard.poll();
                    if new.is_empty() {
                        continue;
                    }
                    let at_ms = u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let mut list = held.lock().unwrap_or_else(|e| e.into_inner());
                    for (key, mods) in new {
                        list.push_back(Press { at_ms, key, mods });
                    }
                    let from = keep_ms.map(|k| at_ms.saturating_sub(k.saturating_mul(2)));
                    while list.len() > MAX_PRESSES
                        || list
                            .front()
                            .is_some_and(|p| from.is_some_and(|f| p.at_ms < f))
                    {
                        list.pop_front();
                    }
                }
            })
            .map_err(|e| tracing::warn!("the keys could not be read: {e}"))
            .ok();
        Tracker {
            stop,
            presses,
            thread,
        }
    }

    /// The presses so far, oldest first, while it goes on.
    pub fn presses(&self) -> Vec<Press> {
        self.presses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .copied()
            .collect()
    }

    /// Stop reading, and hand back what was pressed.
    pub fn finish(mut self) -> Vec<Press> {
        self.halt();
        self.presses()
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.halt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keyboard that presses the keys it is given, one batch per look.
    struct Script(Vec<Vec<(Key, Mods)>>);

    impl Poll for Script {
        fn poll(&mut self) -> Vec<(Key, Mods)> {
            if self.0.is_empty() {
                Vec::new()
            } else {
                self.0.remove(0)
            }
        }
    }

    fn char_key(c: char) -> (Key, Mods) {
        (Key::Char { typed: c, base: c }, Mods::default())
    }

    #[test]
    fn the_tracker_keeps_the_presses_in_order_but_not_what_was_down_at_the_start() {
        let script = Script(vec![
            vec![char_key('x')], // already down when it starts
            vec![char_key('a')],
            vec![],
            vec![char_key('b'), char_key('c')],
        ]);
        let t = Tracker::with(Box::new(script), Instant::now(), None);
        std::thread::sleep(POLL * 10);
        let got: Vec<Key> = t.finish().iter().map(|p| p.key).collect();
        assert_eq!(got, vec![char_key('a').0, char_key('b').0, char_key('c').0]);
    }

    #[test]
    fn a_continuous_tracker_forgets_old_presses() {
        let clock = Instant::now() - Duration::from_secs(60);
        // the clock says a minute has gone, so with 10 s kept nothing old is held
        let t = Tracker::with(
            Box::new(Script(vec![vec![], vec![char_key('a')]])),
            clock,
            Some(10_000),
        );
        std::thread::sleep(POLL * 5);
        let p = t.finish();
        assert_eq!(p.len(), 1);
        assert!(p[0].at_ms >= 60_000);
    }
}
