//! The keyboard on an X11 desktop: which keys are down, from the X server (`QueryKeymap`), and
//! what each writes, from its keymap. Pure Rust, no library needed.

use crate::{names, Key, Mods, Poll};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, KeyButMask};
use x11rb::rust_connection::RustConnection;

pub struct Keyboard {
    conn: RustConnection,
    root: u32,
    first: u8,
    per: usize,
    syms: Vec<u32>,
    down: [u8; 32],
}

// what the keysyms of the modifiers are
const SHIFT: [u32; 2] = [0xffe1, 0xffe2];
const CTRL: [u32; 2] = [0xffe3, 0xffe4];
const ALT: [u32; 2] = [0xffe9, 0xffea];
const META: [u32; 4] = [0xffe7, 0xffe8, 0xffeb, 0xffec];
const ALT_GR: u32 = 0xfe03;
const CAPS: u32 = 0xffe5;

impl Keyboard {
    pub fn connect() -> Result<Self, String> {
        let no = |e: &dyn std::fmt::Display| format!("The keyboard could not be read: {e}");
        let (conn, screen) = RustConnection::connect(None).map_err(|e| no(&e))?;
        let setup = conn.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let root = setup.roots[screen].root;
        let map = conn
            .get_keyboard_mapping(first, last - first + 1)
            .map_err(|e| no(&e))?
            .reply()
            .map_err(|e| no(&e))?;
        Ok(Self {
            conn,
            root,
            first,
            per: usize::from(map.keysyms_per_keycode).max(1),
            syms: map.keysyms,
            down: [0; 32],
        })
    }

    fn sym(&self, code: u8, level: usize) -> u32 {
        let Some(i) = usize::from(code).checked_sub(usize::from(self.first)) else {
            return 0;
        };
        if level >= self.per {
            return 0;
        }
        self.syms.get(i * self.per + level).copied().unwrap_or(0)
    }

    fn is_down(keys: &[u8; 32], code: usize) -> bool {
        keys[code / 8] & (1 << (code % 8)) != 0
    }

    fn held(&self, keys: &[u8; 32], wanted: &[u32]) -> bool {
        (0..256).any(|c| Self::is_down(keys, c) && wanted.contains(&self.sym(c as u8, 0)))
    }

    fn caps_lock(&self) -> bool {
        self.conn
            .query_pointer(self.root)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| u16::from(r.mask) & u16::from(KeyButMask::LOCK) != 0)
    }
}

/// A keysym as something shown.
pub(crate) fn key_of(sym: u32) -> Option<Key> {
    let named = |n: &'static str| Some(Key::Named(n));
    let char_of = |c: char| Some(Key::Char { typed: c, base: c });
    match sym {
        0 => None,
        s if SHIFT.contains(&s) || CTRL.contains(&s) || ALT.contains(&s) || META.contains(&s) => {
            Some(Key::Modifier)
        }
        ALT_GR | CAPS | 0xff7f /* Num_Lock */ | 0xff14 /* Scroll_Lock */ => Some(Key::Modifier),
        0xff0d | 0xff8d => named(names::ENTER),
        0xff08 => named(names::BACKSPACE),
        0xff09 | 0xfe20 => named(names::TAB),
        0xff1b => named(names::ESCAPE),
        0xffff | 0xff9f => named(names::DELETE),
        0xff63 | 0xff9e => named(names::INSERT),
        0xff50 | 0xff95 => named(names::HOME),
        0xff57 | 0xff9c => named(names::END),
        0xff55 | 0xff9a => named(names::PAGE_UP),
        0xff56 | 0xff9b => named(names::PAGE_DOWN),
        0xff51 | 0xff96 => named(names::LEFT),
        0xff52 | 0xff97 => named(names::UP),
        0xff53 | 0xff98 => named(names::RIGHT),
        0xff54 | 0xff99 => named(names::DOWN),
        0xff61 => named(names::PRINT),
        0xffbe..=0xffd5 => named(names::F[(sym - 0xffbe) as usize]),
        0xffb0..=0xffb9 => char_of(char::from(b'0' + (sym - 0xffb0) as u8)),
        0xffaa => char_of('*'),
        0xffab => char_of('+'),
        0xffad => char_of('-'),
        0xffae => char_of('.'),
        0xffaf => char_of('/'),
        0xff80 => char_of(' '),
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(sym).and_then(char_of),
        0x0100_0100..=0x0110_ffff => char::from_u32(sym - 0x0100_0000).and_then(char_of),
        _ => None,
    }
}

impl Poll for Keyboard {
    fn poll(&mut self) -> Vec<(Key, Mods)> {
        let Ok(keys) = self
            .conn
            .query_keymap()
            .map_err(|_| ())
            .and_then(|c| c.reply().map_err(|_| ()))
            .map(|r| r.keys)
        else {
            return Vec::new();
        };
        let mods = Mods {
            ctrl: self.held(&keys, &CTRL),
            alt: self.held(&keys, &ALT),
            shift: self.held(&keys, &SHIFT),
            meta: self.held(&keys, &META),
        };
        let alt_gr = self.held(&keys, &[ALT_GR]);
        let mut out = Vec::new();
        for code in 0..256usize {
            if !Self::is_down(&keys, code) || Self::is_down(&self.down, code) {
                continue;
            }
            let base = self.sym(code as u8, 0);
            let Some(key) = key_of(base) else { continue };
            let key = match key {
                Key::Char { base: b, .. } => {
                    // the sign the key writes now: with Shift (or Caps Lock, for letters), or AltGr
                    let shifted =
                        mods.shift ^ (b.is_alphabetic() && b.is_lowercase() && self.caps_lock());
                    let level = match (alt_gr, shifted) {
                        (true, false) => 4,
                        (true, true) => 5,
                        (false, true) => 1,
                        (false, false) => 0,
                    };
                    let typed = match key_of(self.sym(code as u8, level)) {
                        Some(Key::Char { typed, .. }) => typed,
                        // no sign at that level: the letter in capitals, or the key's own sign
                        _ if shifted => b.to_uppercase().next().unwrap_or(b),
                        _ => b,
                    };
                    Key::Char { typed, base: b }
                }
                other => other,
            };
            out.push((key, mods));
        }
        self.down = keys;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysyms_are_named_as_shown() {
        assert_eq!(
            key_of(0x61),
            Some(Key::Char {
                typed: 'a',
                base: 'a'
            })
        );
        assert_eq!(
            key_of(0xe9),
            Some(Key::Char {
                typed: 'é',
                base: 'é'
            })
        );
        assert_eq!(
            key_of(0x0100_20ac),
            Some(Key::Char {
                typed: '€',
                base: '€'
            })
        );
        assert_eq!(key_of(0xff0d), Some(Key::Named("⏎")));
        assert_eq!(key_of(0xffc2), Some(Key::Named("F5")));
        assert_eq!(key_of(0xffe3), Some(Key::Modifier));
        assert_eq!(
            key_of(0xffb7),
            Some(Key::Char {
                typed: '7',
                base: '7'
            })
        );
        assert_eq!(key_of(0x1234), None);
        assert_eq!(key_of(0), None);
    }
}

/// Needs an X display and `xdotool`: `xvfb-run cargo test -p cargoframe-keys -- --ignored`.
#[cfg(test)]
mod on_a_display {
    use super::*;
    use std::process::Command;

    #[test]
    #[ignore = "needs an X display and xdotool"]
    fn keys_typed_on_the_display_are_read() {
        let mut k = Keyboard::connect().expect("a display");
        let _ = k.poll();
        let mut seen = Vec::new();
        for args in [
            &["keydown", "h"][..],
            &["keyup", "h"],
            &["keydown", "shift"],
            &["keydown", "1"],
            &["keyup", "1"],
            &["keyup", "shift"],
            &["keydown", "ctrl"],
            &["keydown", "s"],
            &["keyup", "s"],
            &["keyup", "ctrl"],
            &["keydown", "Return"],
            &["keyup", "Return"],
        ] {
            Command::new("xdotool").args(args).status().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(40));
            seen.extend(k.poll());
        }
        let shown: Vec<(Key, bool)> = seen
            .into_iter()
            .filter(|(k, _)| *k != Key::Modifier)
            .map(|(k, m)| (k, m.ctrl))
            .collect();
        assert_eq!(
            shown,
            vec![
                (
                    Key::Char {
                        typed: 'h',
                        base: 'h'
                    },
                    false
                ),
                (
                    Key::Char {
                        typed: '!',
                        base: '1'
                    },
                    false
                ),
                (
                    Key::Char {
                        typed: 's',
                        base: 's'
                    },
                    true
                ),
                (Key::Named("⏎"), false),
            ]
        );
    }
}
