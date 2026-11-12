//! From presses to what the clip shows: one line of keys at a time.
//!
//! Keys pressed close together (less than [`LINGER_MS`] apart) make one line that grows as they
//! come: written text as it was typed, other keys by their names, combinations as "Ctrl+S". The
//! line shows from the first press until [`LINGER_MS`] after the last one, and each press makes
//! a new [`Caption`] (the line as it is from then on), so the clip shows each key when it was
//! pressed.

use crate::{Key, Mods, Press};
use serde::Serialize;

/// How long a line stays after the last key of it.
pub const LINGER_MS: u64 = 1_500;
/// The longest line shown: the start of a longer one gives way ("…").
pub const MAX_CHARS: usize = 32;
/// The most lines one clip carries (a fast typist makes about ten a second).
pub const MAX_CAPTIONS: usize = 400;

/// A line of keys, and when it shows, on the clip's own timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Caption {
    pub from_ms: u64,
    pub to_ms: u64,
    pub text: String,
}

enum Piece {
    Typed(char),
    Token(String),
}

/// What one press adds to the line, or nothing (a modifier alone).
fn piece(p: &Press) -> Option<Piece> {
    let Mods {
        ctrl,
        alt,
        shift,
        meta,
    } = p.mods;
    match p.key {
        Key::Modifier => None,
        Key::Char { typed, base } => {
            // AltGr is Ctrl and Alt together: what it writes is typed text
            let alt_gr = ctrl && alt && !meta && typed != base && !typed.is_control();
            if (ctrl || alt || meta) && !alt_gr {
                let name = if base == ' ' {
                    "Space".to_string()
                } else {
                    base.to_uppercase().collect()
                };
                Some(Piece::Token(combo(p.mods, &name, true)))
            } else if typed.is_control() {
                None
            } else {
                Some(Piece::Typed(typed))
            }
        }
        Key::Named(name) => Some(Piece::Token(if ctrl || alt || meta || shift {
            combo(p.mods, name, true)
        } else {
            name.to_string()
        })),
    }
}

fn combo(m: Mods, name: &str, with_shift: bool) -> String {
    let mut s = String::new();
    for (on, word) in [
        (m.ctrl, "Ctrl+"),
        (m.alt, "Alt+"),
        (m.shift && with_shift, "Shift+"),
        (m.meta, "Win+"),
    ] {
        if on {
            s.push_str(word);
        }
    }
    s.push_str(name);
    s
}

/// The line the pieces make: typed text as it was typed, names apart.
fn line(pieces: &[Piece]) -> String {
    let mut s = String::new();
    let mut last_typed = false;
    for p in pieces {
        match p {
            Piece::Typed(c) => {
                if !last_typed && !s.is_empty() {
                    s.push(' ');
                }
                s.push(*c);
                last_typed = true;
            }
            Piece::Token(t) => {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(t);
                last_typed = false;
            }
        }
    }
    // a space just pressed shows as one
    if s.ends_with(' ') {
        s.pop();
        s.push('␣');
    }
    let count = s.chars().count();
    if count > MAX_CHARS {
        let tail: String = s.chars().skip(count - (MAX_CHARS - 1)).collect();
        s = format!("…{}", tail.trim_start());
    }
    s
}

/// The lines a clip shows, for the presses (on the recording's clock) between `from_ms` and
/// `to_ms` of the recording. Times come back on the clip's timeline (0 is `from_ms`).
pub fn captions(presses: &[Press], from_ms: u64, to_ms: u64) -> Vec<Caption> {
    let mut out: Vec<Caption> = Vec::new();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut last_at: Option<u64> = None;
    let shown: Vec<(u64, Piece)> = presses
        .iter()
        .filter(|p| p.at_ms <= to_ms)
        .filter_map(|p| piece(p).map(|x| (p.at_ms, x)))
        .collect();
    let mut iter = shown.into_iter().peekable();
    while let Some((at, p)) = iter.next() {
        if last_at.is_some_and(|l| at.saturating_sub(l) > LINGER_MS) {
            pieces.clear();
        }
        pieces.push(p);
        last_at = Some(at);
        let next = iter.peek().map(|(t, _)| *t);
        let until = match next {
            Some(n) if n.saturating_sub(at) <= LINGER_MS => n,
            _ => at + LINGER_MS,
        };
        // only what shows during the clip, cut to it
        let (a, b) = (at.max(from_ms), until.min(to_ms));
        if b <= a {
            continue;
        }
        let text = line(&pieces);
        let (a, b) = (a - from_ms, b - from_ms);
        // a press that changes nothing seen (a second Shift+…) just makes the line last
        match out.last_mut() {
            Some(c) if c.text == text && c.to_ms == a => c.to_ms = b,
            _ => out.push(Caption {
                from_ms: a,
                to_ms: b,
                text,
            }),
        }
        // the old pieces of a long line are not needed to write its end
        if pieces.len() > MAX_CHARS * 2 {
            pieces.drain(..pieces.len() - MAX_CHARS);
        }
    }
    if out.len() > MAX_CAPTIONS {
        out.drain(..out.len() - MAX_CAPTIONS);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64, key: Key) -> Press {
        Press {
            at_ms: ms,
            key,
            mods: Mods::default(),
        }
    }

    fn typed(ms: u64, c: char) -> Press {
        at(
            ms,
            Key::Char {
                typed: c,
                base: c.to_ascii_lowercase(),
            },
        )
    }

    fn texts(c: &[Caption]) -> Vec<&str> {
        c.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn typing_makes_one_growing_line_that_stays_a_moment() {
        let p: Vec<Press> = "Hi you"
            .chars()
            .enumerate()
            .map(|(i, c)| typed(1_000 + i as u64 * 100, c))
            .collect();
        let c = captions(&p, 0, 10_000);
        assert_eq!(texts(&c), ["H", "Hi", "Hi␣", "Hi y", "Hi yo", "Hi you"]);
        // each shows until the next key, the last one a moment more
        assert_eq!((c[0].from_ms, c[0].to_ms), (1_000, 1_100));
        assert_eq!((c[5].from_ms, c[5].to_ms), (1_500, 1_500 + LINGER_MS));
    }

    #[test]
    fn a_pause_starts_a_new_line_and_names_and_combinations_stand_apart() {
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        let p = vec![
            typed(0, 'o'),
            typed(100, 'k'),
            at(200, Key::Named("⏎")),
            // two seconds later: a new line
            Press {
                at_ms: 2_500,
                key: Key::Char {
                    typed: '\u{13}',
                    base: 's',
                },
                mods: ctrl,
            },
            Press {
                at_ms: 2_600,
                key: Key::Modifier,
                mods: ctrl,
            },
            Press {
                at_ms: 2_700,
                key: Key::Named("⇥"),
                mods: Mods {
                    shift: true,
                    ..Mods::default()
                },
            },
        ];
        let c = captions(&p, 0, 10_000);
        assert_eq!(texts(&c), ["o", "ok", "ok ⏎", "Ctrl+S", "Ctrl+S Shift+⇥"]);
        // the modifier alone shows nothing, so Ctrl+S lasts until Shift+Tab
        assert_eq!((c[3].from_ms, c[3].to_ms), (2_500, 2_700));
    }

    #[test]
    fn alt_gr_writes_and_ctrl_space_is_named() {
        let p = vec![
            Press {
                at_ms: 0,
                key: Key::Char {
                    typed: '@',
                    base: 'à',
                },
                mods: Mods {
                    ctrl: true,
                    alt: true,
                    ..Mods::default()
                },
            },
            Press {
                at_ms: 100,
                key: Key::Char {
                    typed: ' ',
                    base: ' ',
                },
                mods: Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            },
        ];
        assert_eq!(texts(&captions(&p, 0, 5_000)), ["@", "@ Ctrl+Space"]);
    }

    #[test]
    fn a_long_line_keeps_its_end() {
        let p: Vec<Press> = (0..100u64)
            .map(|i| typed(i * 50, char::from(b'a' + (i % 26) as u8)))
            .collect();
        let c = captions(&p, 0, 60_000);
        let last = &c.last().unwrap().text;
        assert_eq!(last.chars().count(), MAX_CHARS);
        assert!(last.starts_with('…'));
        assert!(last.ends_with("uv"), "{last}");
    }

    #[test]
    fn only_what_shows_in_the_clip_is_kept_on_its_own_timeline() {
        let p = vec![typed(1_000, 'a'), typed(5_000, 'b'), typed(9_000, 'c')];
        // the clip is 4.5 s to 9.5 s of the recording
        let c = captions(&p, 4_500, 9_500);
        assert_eq!(texts(&c), ["b", "c"]);
        assert_eq!((c[0].from_ms, c[0].to_ms), (500, 500 + LINGER_MS));
        // the last one is cut where the clip ends
        assert_eq!((c[1].from_ms, c[1].to_ms), (4_500, 5_000));
        // a line that began before the clip shows its earlier keys too
        let p = vec![typed(4_000, 'a'), typed(4_600, 'b')];
        let c = captions(&p, 4_500, 9_500);
        assert_eq!(texts(&c), ["a", "ab"]);
        assert_eq!(c[0].from_ms, 0);
    }

    #[test]
    fn a_very_busy_clip_keeps_its_last_lines() {
        let p: Vec<Press> = (0..2_000u64)
            .map(|i| typed(i * 10, char::from(b'a' + (i % 26) as u8)))
            .collect();
        let c = captions(&p, 0, 100_000);
        assert_eq!(c.len(), MAX_CAPTIONS);
        assert_eq!(c.last().unwrap().to_ms, 19_990 + LINGER_MS);
    }
}
