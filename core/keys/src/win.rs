//! The keyboard on Windows: which keys are down (`GetAsyncKeyState`), and what each writes with
//! the keyboard layout of the window in focus (`ToUnicodeEx`, told not to change the keyboard's
//! state, so dead keys keep working in the program being typed in).

use crate::{names, Key, Mods, Poll};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, MapVirtualKeyW, ToUnicodeEx,
    MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub struct Keyboard {
    down: [bool; 256],
}

// virtual-key codes
const VK_SHIFT: usize = 0x10;
const VK_CONTROL: usize = 0x11;
const VK_MENU: usize = 0x12;
const VK_CAPITAL: usize = 0x14;
const L_SHIFT: i32 = 0xA0;
const R_SHIFT: i32 = 0xA1;
const L_CTRL: i32 = 0xA2;
const R_CTRL: i32 = 0xA3;
const L_ALT: i32 = 0xA4;
const R_ALT: i32 = 0xA5;
const L_WIN: i32 = 0x5B;
const R_WIN: i32 = 0x5C;

impl Keyboard {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self { down: [false; 256] }
    }
}

/// Is the key down now (the high bit), or was it pressed and let go since the last look (the
/// low bit, so a quick tap between two looks is not missed)?
fn state(vk: i32) -> (bool, bool) {
    // SAFETY: GetAsyncKeyState takes any key code and has no other precondition.
    let s = unsafe { GetAsyncKeyState(vk) } as u16;
    (s & 0x8000 != 0, s & 1 != 0)
}

/// The keys that never show alone: mouse buttons, the generic Shift/Ctrl/Alt (their left and
/// right keys are read instead), and the IME keys.
fn skipped(vk: i32) -> bool {
    matches!(vk, 0x01..=0x06 | 0x10..=0x12 | 0x15..=0x1A | 0x1C..=0x1F | 0xE5 | 0xE7)
}

pub(crate) fn named(vk: i32) -> Option<Key> {
    let n = |s: &'static str| Some(Key::Named(s));
    let c = |ch: char| {
        Some(Key::Char {
            typed: ch,
            base: ch,
        })
    };
    match vk {
        L_SHIFT | R_SHIFT | L_CTRL | R_CTRL | L_ALT | R_ALT | L_WIN | R_WIN => Some(Key::Modifier),
        0x14 | 0x90 | 0x91 => Some(Key::Modifier), // Caps Lock, Num Lock, Scroll Lock
        0x0D => n(names::ENTER),
        0x08 => n(names::BACKSPACE),
        0x09 => n(names::TAB),
        0x1B => n(names::ESCAPE),
        0x2E => n(names::DELETE),
        0x2D => n(names::INSERT),
        0x24 => n(names::HOME),
        0x23 => n(names::END),
        0x21 => n(names::PAGE_UP),
        0x22 => n(names::PAGE_DOWN),
        0x25 => n(names::LEFT),
        0x26 => n(names::UP),
        0x27 => n(names::RIGHT),
        0x28 => n(names::DOWN),
        0x2C => n(names::PRINT),
        0x70..=0x87 => n(names::F[(vk - 0x70) as usize]),
        0x20 => c(' '),
        0x60..=0x69 => c(char::from(b'0' + (vk - 0x60) as u8)),
        0x6A => c('*'),
        0x6B => c('+'),
        0x6D => c('-'),
        0x6E => c('.'),
        0x6F => c('/'),
        _ => None,
    }
}

/// What a writing key writes now, and its own sign; `None` for a key that writes nothing.
fn written(vk: i32, mods: Mods) -> Option<Key> {
    // SAFETY: plain Win32 calls with properly sized buffers; ToUnicodeEx is given flag 4, so it
    // does not change the keyboard's state (dead keys stay with the program typed in).
    unsafe {
        let base = char::from_u32(MapVirtualKeyW(vk as u32, MAPVK_VK_TO_CHAR) & 0x7FFF)
            .filter(|c| !c.is_control())?
            .to_lowercase()
            .next()?;
        let mut keys = [0u8; 256];
        if mods.shift {
            keys[VK_SHIFT] = 0x80;
        }
        // AltGr is Ctrl and Alt together
        if mods.ctrl && mods.alt {
            keys[VK_CONTROL] = 0x80;
            keys[VK_MENU] = 0x80;
        }
        if GetKeyState(VK_CAPITAL as i32) & 1 != 0 {
            keys[VK_CAPITAL] = 0x01;
        }
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let layout = GetKeyboardLayout(thread);
        let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC);
        let mut out = [0u16; 8];
        let n = ToUnicodeEx(vk as u32, scan, &keys, &mut out, 4, Some(layout));
        let typed = if n > 0 {
            char::decode_utf16(out[..n as usize].iter().copied())
                .next()
                .and_then(Result::ok)
                .filter(|c| !c.is_control())
                .unwrap_or(base)
        } else {
            base
        };
        Some(Key::Char { typed, base })
    }
}

impl Poll for Keyboard {
    fn poll(&mut self) -> Vec<(Key, Mods)> {
        let is = |vk: i32| state(vk).0;
        let mods = Mods {
            shift: is(L_SHIFT) || is(R_SHIFT),
            ctrl: is(L_CTRL) || is(R_CTRL),
            alt: is(L_ALT) || is(R_ALT),
            meta: is(L_WIN) || is(R_WIN),
        };
        let mut out = Vec::new();
        for vk in 0x07..0xFFi32 {
            if skipped(vk) {
                continue;
            }
            let (down, tapped) = state(vk);
            let was = std::mem::replace(&mut self.down[vk as usize], down);
            if was || !(down || tapped) {
                continue;
            }
            if let Some(key) = named(vk).or_else(|| written(vk, mods)) {
                out.push((key, mods));
            }
        }
        out
    }
}
