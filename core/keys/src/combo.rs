//! Shortcuts the system cannot hold: several keys at once ("Ctrl+A+S"), or a key its shortcut
//! service does not know (the `<>` key of European keyboards, the number pad on X11, the
//! browser keys). A thread looks at the few keys these shortcuts use, forty times a second, and
//! says when one is pressed and let go. It runs only while such a shortcut exists; the others
//! are held by the system, which costs nothing while waiting.
//!
//! The keys are named as browsers name them (`KeyA`, `IntlBackslash`, `NumpadAdd`): by where they
//! are on the keyboard, whatever its layout. Unlike a shortcut the system holds, the keys still
//! reach the program in use.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// How often the keys are looked at.
pub const COMBO_POLL: Duration = Duration::from_millis(25);

/// How a key is found on Windows: by its virtual-key code, or (for the keys that write, whose
/// code moves with the layout) by its place on the keyboard (scan code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinKey {
    Vk(u16),
    Scan(u16),
}

/// One key: its name, its Linux (evdev) number, and how Windows finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    pub name: &'static str,
    pub evdev: u16,
    pub win: WinKey,
}

const fn vk(name: &'static str, evdev: u16, v: u16) -> Code {
    Code {
        name,
        evdev,
        win: WinKey::Vk(v),
    }
}
const fn sc(name: &'static str, evdev: u16, s: u16) -> Code {
    Code {
        name,
        evdev,
        win: WinKey::Scan(s),
    }
}

/// Every key a shortcut can use.
pub const CODES: &[Code] = &[
    // the modifiers, each side
    vk("ControlLeft", 29, 0xA2),
    vk("ControlRight", 97, 0xA3),
    vk("AltLeft", 56, 0xA4),
    vk("AltRight", 100, 0xA5),
    vk("ShiftLeft", 42, 0xA0),
    vk("ShiftRight", 54, 0xA1),
    vk("MetaLeft", 125, 0x5B),
    vk("MetaRight", 126, 0x5C),
    // letters, by place
    sc("KeyA", 30, 0x1E),
    sc("KeyB", 48, 0x30),
    sc("KeyC", 46, 0x2E),
    sc("KeyD", 32, 0x20),
    sc("KeyE", 18, 0x12),
    sc("KeyF", 33, 0x21),
    sc("KeyG", 34, 0x22),
    sc("KeyH", 35, 0x23),
    sc("KeyI", 23, 0x17),
    sc("KeyJ", 36, 0x24),
    sc("KeyK", 37, 0x25),
    sc("KeyL", 38, 0x26),
    sc("KeyM", 50, 0x32),
    sc("KeyN", 49, 0x31),
    sc("KeyO", 24, 0x18),
    sc("KeyP", 25, 0x19),
    sc("KeyQ", 16, 0x10),
    sc("KeyR", 19, 0x13),
    sc("KeyS", 31, 0x1F),
    sc("KeyT", 20, 0x14),
    sc("KeyU", 22, 0x16),
    sc("KeyV", 47, 0x2F),
    sc("KeyW", 17, 0x11),
    sc("KeyX", 45, 0x2D),
    sc("KeyY", 21, 0x15),
    sc("KeyZ", 44, 0x2C),
    // the row of numbers
    sc("Digit1", 2, 0x02),
    sc("Digit2", 3, 0x03),
    sc("Digit3", 4, 0x04),
    sc("Digit4", 5, 0x05),
    sc("Digit5", 6, 0x06),
    sc("Digit6", 7, 0x07),
    sc("Digit7", 8, 0x08),
    sc("Digit8", 9, 0x09),
    sc("Digit9", 10, 0x0A),
    sc("Digit0", 11, 0x0B),
    // signs, by place
    sc("Minus", 12, 0x0C),
    sc("Equal", 13, 0x0D),
    sc("BracketLeft", 26, 0x1A),
    sc("BracketRight", 27, 0x1B),
    sc("Backslash", 43, 0x2B),
    sc("Semicolon", 39, 0x27),
    sc("Quote", 40, 0x28),
    sc("Backquote", 41, 0x29),
    sc("Comma", 51, 0x33),
    sc("Period", 52, 0x34),
    sc("Slash", 53, 0x35),
    sc("IntlBackslash", 86, 0x56),
    sc("IntlRo", 89, 0x73),
    sc("IntlYen", 124, 0x7D),
    // keys that do not write
    vk("Escape", 1, 0x1B),
    vk("Enter", 28, 0x0D),
    vk("Tab", 15, 0x09),
    vk("Space", 57, 0x20),
    vk("Backspace", 14, 0x08),
    vk("CapsLock", 58, 0x14),
    vk("Insert", 110, 0x2D),
    vk("Delete", 111, 0x2E),
    vk("Home", 102, 0x24),
    vk("End", 107, 0x23),
    vk("PageUp", 104, 0x21),
    vk("PageDown", 109, 0x22),
    vk("ArrowUp", 103, 0x26),
    vk("ArrowDown", 108, 0x28),
    vk("ArrowLeft", 105, 0x25),
    vk("ArrowRight", 106, 0x27),
    vk("PrintScreen", 99, 0x2C),
    vk("ScrollLock", 70, 0x91),
    vk("Pause", 119, 0x13),
    vk("ContextMenu", 127, 0x5D),
    // F1 to F24
    vk("F1", 59, 0x70),
    vk("F2", 60, 0x71),
    vk("F3", 61, 0x72),
    vk("F4", 62, 0x73),
    vk("F5", 63, 0x74),
    vk("F6", 64, 0x75),
    vk("F7", 65, 0x76),
    vk("F8", 66, 0x77),
    vk("F9", 67, 0x78),
    vk("F10", 68, 0x79),
    vk("F11", 87, 0x7A),
    vk("F12", 88, 0x7B),
    vk("F13", 183, 0x7C),
    vk("F14", 184, 0x7D),
    vk("F15", 185, 0x7E),
    vk("F16", 186, 0x7F),
    vk("F17", 187, 0x80),
    vk("F18", 188, 0x81),
    vk("F19", 189, 0x82),
    vk("F20", 190, 0x83),
    vk("F21", 191, 0x84),
    vk("F22", 192, 0x85),
    vk("F23", 193, 0x86),
    vk("F24", 194, 0x87),
    // the number pad
    vk("NumLock", 69, 0x90),
    vk("Numpad0", 82, 0x60),
    vk("Numpad1", 79, 0x61),
    vk("Numpad2", 80, 0x62),
    vk("Numpad3", 81, 0x63),
    vk("Numpad4", 75, 0x64),
    vk("Numpad5", 76, 0x65),
    vk("Numpad6", 77, 0x66),
    vk("Numpad7", 71, 0x67),
    vk("Numpad8", 72, 0x68),
    vk("Numpad9", 73, 0x69),
    vk("NumpadAdd", 78, 0x6B),
    vk("NumpadSubtract", 74, 0x6D),
    vk("NumpadMultiply", 55, 0x6A),
    vk("NumpadDivide", 98, 0x6F),
    vk("NumpadDecimal", 83, 0x6E),
    vk("NumpadEnter", 96, 0x0D),
    // sound, music and browser keys
    vk("AudioVolumeMute", 113, 0xAD),
    vk("AudioVolumeDown", 114, 0xAE),
    vk("AudioVolumeUp", 115, 0xAF),
    vk("MediaTrackNext", 163, 0xB0),
    vk("MediaTrackPrevious", 165, 0xB1),
    vk("MediaStop", 166, 0xB2),
    vk("MediaPlayPause", 164, 0xB3),
    vk("LaunchMail", 155, 0xB4),
    vk("LaunchApp2", 140, 0xB7),
    vk("BrowserBack", 158, 0xA6),
    vk("BrowserForward", 159, 0xA7),
    vk("BrowserRefresh", 173, 0xA8),
    vk("BrowserSearch", 217, 0xAA),
    vk("BrowserHome", 172, 0xAC),
];

/// The key named `name`, as browsers name it.
pub fn code(name: &str) -> Option<&'static Code> {
    CODES.iter().find(|c| c.name.eq_ignore_ascii_case(name))
}

/// The keys a modifier is ("Ctrl" is either Control key).
pub fn modifier_codes(modifier: &str) -> Option<[&'static str; 2]> {
    match modifier {
        "Ctrl" => Some(["ControlLeft", "ControlRight"]),
        "Alt" => Some(["AltLeft", "AltRight"]),
        "Shift" => Some(["ShiftLeft", "ShiftRight"]),
        "Super" => Some(["MetaLeft", "MetaRight"]),
        _ => None,
    }
}

const MODIFIERS: [&str; 4] = ["Ctrl", "Alt", "Shift", "Super"];

/// A shortcut as keys to watch: each part is satisfied by any of its keys (a modifier's left or
/// right key), and the modifiers it leaves out must be up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combo {
    pub id: String,
    parts: Vec<Vec<usize>>,
    others: Vec<usize>,
}

impl Combo {
    /// A canonical shortcut (`Ctrl+KeyA+KeyS`, `IntlBackslash`) for `id`; `None` when a key is
    /// not one this module knows.
    pub fn new(id: &str, shortcut: &str) -> Option<Combo> {
        let index = |name: &str| CODES.iter().position(|c| c.name == name);
        let mut parts = Vec::new();
        let mut used = Vec::new();
        for token in shortcut.split('+').filter(|t| !t.is_empty()) {
            if let Some(sides) = modifier_codes(token) {
                used.push(token);
                parts.push(sides.iter().filter_map(|n| index(n)).collect());
            } else {
                parts.push(vec![index(code(token)?.name)?]);
            }
        }
        if parts.is_empty() {
            return None;
        }
        let others = MODIFIERS
            .iter()
            .filter(|m| !used.contains(m))
            .flat_map(|m| modifier_codes(m).unwrap_or_default())
            .filter_map(index)
            .collect();
        Some(Combo {
            id: id.to_string(),
            parts,
            others,
        })
    }

    /// The keys looked at for it.
    fn watched(&self) -> impl Iterator<Item = usize> + '_ {
        self.parts.iter().flatten().chain(&self.others).copied()
    }

    /// Is it held, given which keys are down (by index in [`CODES`])?
    pub fn held(&self, down: &dyn Fn(usize) -> bool) -> bool {
        self.parts.iter().all(|p| p.iter().any(|&k| down(k)))
            && !self.others.iter().any(|&k| down(k))
    }
}

/// Reads whether keys are down.
pub(crate) trait Board: Send {
    /// For each key of `keys` (by index in [`CODES`]), whether it is down now.
    fn down(&mut self, keys: &[usize]) -> Vec<bool>;
}

/// Which combos went down or came up between two looks.
#[derive(Debug, Default)]
pub(crate) struct Edges {
    held: Vec<bool>,
}

impl Edges {
    pub(crate) fn step(
        &mut self,
        combos: &[Combo],
        down: &dyn Fn(usize) -> bool,
    ) -> Vec<(usize, bool)> {
        self.held.resize(combos.len(), false);
        let mut out = Vec::new();
        for (i, c) in combos.iter().enumerate() {
            let now = c.held(down);
            if now != self.held[i] {
                self.held[i] = now;
                out.push((i, now));
            }
        }
        out
    }
}

fn board() -> Result<Box<dyn Board>, String> {
    #[cfg(windows)]
    {
        Ok(Box::new(crate::win::Board::new()))
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_none() {
            return Err(crate::WAYLAND_SHORTCUTS.into());
        }
        crate::x11::Board::connect().map(|b| Box::new(b) as Box<dyn Board>)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("This system does not let CargoFrame see the keys pressed.".into())
    }
}

/// The combos watched, until dropped.
pub struct Combos {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Combos {
    /// Watch `combos`; `on(id, down)` is called when one is pressed (`true`) and let go.
    pub fn start(
        combos: Vec<Combo>,
        on: impl Fn(&str, bool) + Send + 'static,
    ) -> Result<Combos, String> {
        let board = board()?;
        Ok(Self::with(board, combos, on))
    }

    pub(crate) fn with(
        mut board: Box<dyn Board>,
        combos: Vec<Combo>,
        on: impl Fn(&str, bool) + Send + 'static,
    ) -> Combos {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("shortcut-keys".into())
            .spawn(move || {
                let mut keys: Vec<usize> = combos.iter().flat_map(Combo::watched).collect();
                keys.sort_unstable();
                keys.dedup();
                let mut edges = Edges::default();
                while !flag.load(Ordering::SeqCst) {
                    let read = board.down(&keys);
                    let down = |k: usize| keys.binary_search(&k).is_ok_and(|i| read[i]);
                    for (i, pressed) in edges.step(&combos, &down) {
                        on(&combos[i].id, pressed);
                    }
                    std::thread::sleep(COMBO_POLL);
                }
            })
            .map_err(|e| tracing::warn!("the shortcut keys could not be watched: {e}"))
            .ok();
        Combos { stop, thread }
    }
}

impl Drop for Combos {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn at(name: &str) -> usize {
        CODES.iter().position(|c| c.name == name).unwrap()
    }

    #[test]
    fn every_key_has_one_name_and_one_place() {
        let mut names: Vec<&str> = CODES.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CODES.len());
        let mut evdev: Vec<u16> = CODES.iter().map(|c| c.evdev).collect();
        evdev.sort_unstable();
        evdev.dedup();
        assert_eq!(evdev.len(), CODES.len());
    }

    #[test]
    fn a_combo_is_held_with_its_keys_and_none_of_the_other_modifiers() {
        let c = Combo::new("t", "Ctrl+KeyA+KeyS").unwrap();
        let downs = |list: &[&str]| {
            let set: Vec<usize> = list.iter().map(|n| at(n)).collect();
            move |k: usize| set.contains(&k)
        };
        assert!(c.held(&downs(&["ControlRight", "KeyA", "KeyS"])));
        assert!(!c.held(&downs(&["ControlLeft", "KeyA"])));
        // Shift is not in it: Ctrl+Shift+A+S is another shortcut
        assert!(!c.held(&downs(&["ControlLeft", "ShiftLeft", "KeyA", "KeyS"])));
        // a key alone, a key no system shortcut knows
        let lone = Combo::new("t", "IntlBackslash").unwrap();
        assert!(lone.held(&downs(&["IntlBackslash"])));
        assert!(!lone.held(&downs(&["IntlBackslash", "AltLeft"])));
        assert!(Combo::new("t", "Ctrl+NoSuchKey").is_none());
    }

    struct Script(Vec<Vec<&'static str>>);

    impl Board for Script {
        fn down(&mut self, keys: &[usize]) -> Vec<bool> {
            let now = if self.0.len() > 1 {
                self.0.remove(0)
            } else {
                self.0[0].clone()
            };
            keys.iter()
                .map(|k| now.iter().any(|n| at(n) == *k))
                .collect()
        }
    }

    #[test]
    fn a_press_and_its_release_are_told_once_each() {
        let said: Arc<Mutex<Vec<(String, bool)>>> = Arc::default();
        let out = said.clone();
        let script = Script(vec![
            vec![],
            vec!["ControlLeft", "KeyA"],
            vec!["ControlLeft", "KeyA", "KeyS"],
            vec!["ControlLeft", "KeyA", "KeyS"],
            vec!["ControlLeft"],
            vec![],
        ]);
        let combos = vec![Combo::new("save", "Ctrl+KeyA+KeyS").unwrap()];
        let w = Combos::with(Box::new(script), combos, move |id, down| {
            out.lock().unwrap().push((id.to_string(), down));
        });
        std::thread::sleep(COMBO_POLL * 10);
        drop(w);
        assert_eq!(
            *said.lock().unwrap(),
            vec![("save".to_string(), true), ("save".to_string(), false)]
        );
    }
}
