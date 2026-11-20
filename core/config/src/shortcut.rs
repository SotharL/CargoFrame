//! Keyboard shortcuts for triggers, written the way people type them ("ctrl + shift + f9") and
//! stored in one canonical form ("Ctrl+Shift+F9").
//!
//! Rules: modifiers (Ctrl, Alt, Shift, Super) and one to three other keys (`Ctrl+A+S`), each
//! once. A key alone needs a modifier, unless it is a function, sound, music or browser key, or
//! Pause; two or more keys together need none. Modifier-only and empty shortcuts are refused.
//!
//! A shortcut of modifiers and one key the system's shortcut service knows is held by the
//! system ([`held_by_system`]); any other is watched by CargoFrame (`cargoframe-keys`).

use cargoframe_i18n::{t, tf};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutError {
    Empty,
    NoKey,
    TooManyKeys,
    RepeatedKey(String),
    UnknownKey(String),
    RepeatedModifier(&'static str),
    NeedsModifier,
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str(t("Press the keys you want to use")),
            Self::NoKey => f.write_str(t(
                "Add a key after the modifiers (for example Ctrl+Shift+F9)",
            )),
            Self::TooManyKeys => f.write_str(t(
                "A shortcut has at most three keys besides Ctrl, Alt, Shift and Win/Super",
            )),
            Self::RepeatedKey(k) => f.write_str(&tf("{k} is used twice", &[("k", k)])),
            Self::UnknownKey(k) => {
                f.write_str(&tf("'{k}' is not a key CargoFrame can use", &[("k", k)]))
            }
            Self::RepeatedModifier(m) => f.write_str(&tf("{m} is used twice", &[("m", m)])),
            Self::NeedsModifier => f.write_str(t("Add Ctrl, Alt, Shift or Win/Super to this key")),
        }
    }
}

impl std::error::Error for ShortcutError {}

const MODIFIER_ORDER: [&str; 4] = ["Ctrl", "Alt", "Shift", "Super"];

fn modifier(token: &str) -> Option<&'static str> {
    match token.to_ascii_lowercase().as_str() {
        "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => Some("Ctrl"),
        "alt" | "option" | "opt" => Some("Alt"),
        "shift" => Some("Shift"),
        "super" | "win" | "windows" | "meta" | "cmd" | "command" => Some("Super"),
        _ => None,
    }
}

/// Canonical spelling of a non-modifier key, and whether it counts as a function key.
fn key(token: &str) -> Option<(String, bool)> {
    let lower = token.to_ascii_lowercase();
    // F1 to F24
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        if (1..=24).contains(&n) {
            return Some((format!("F{n}"), true));
        }
    }
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphanumeric() {
            return Some((c.to_ascii_uppercase().to_string(), false));
        }
        let named = match c {
            '-' => Some("Minus"),
            '=' => Some("Equal"),
            '[' => Some("BracketLeft"),
            ']' => Some("BracketRight"),
            ';' => Some("Semicolon"),
            '\'' => Some("Quote"),
            ',' => Some("Comma"),
            '.' => Some("Period"),
            '/' => Some("Slash"),
            '\\' => Some("Backslash"),
            '`' => Some("Backquote"),
            _ => None,
        };
        return named.map(|n| (n.to_string(), false));
    }
    let named = match lower.as_str() {
        "space" => "Space",
        "enter" | "return" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" | "del" => "Delete",
        "insert" | "ins" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" | "pgup" => "PageUp",
        "pagedown" | "pgdn" => "PageDown",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "printscreen" | "prtsc" | "prtscr" | "print" => "PrintScreen",
        "pause" => "Pause",
        "scrolllock" => "ScrollLock",
        "escape" | "esc" => "Escape",
        "capslock" => "CapsLock",
        "numlock" => "NumLock",
        "contextmenu" | "menu" | "apps" => "ContextMenu",
        // the key between Shift and Z on European keyboards (`<>`), and the Japanese ones
        "intlbackslash" | "<" | ">" => "IntlBackslash",
        "intlro" => "IntlRo",
        "intlyen" => "IntlYen",
        // Keys the plugin already spells in its own way keep that spelling.
        "minus" => "Minus",
        "equal" => "Equal",
        "comma" => "Comma",
        "period" => "Period",
        "slash" => "Slash",
        "backslash" => "Backslash",
        "semicolon" => "Semicolon",
        "quote" => "Quote",
        "backquote" => "Backquote",
        "bracketleft" => "BracketLeft",
        "bracketright" => "BracketRight",
        _ => {
            // the number pad, sound, music and browser keys, by the name browsers give them
            let found = OTHER_KEYS
                .iter()
                .find(|k| k.eq_ignore_ascii_case(token))
                .copied()?;
            return Some((found.to_string(), STANDALONE.contains(&found)));
        }
    };
    Some((named.to_string(), named == "Pause"))
}

/// Keys spelled as browsers name them.
const OTHER_KEYS: &[&str] = &[
    "Numpad0",
    "Numpad1",
    "Numpad2",
    "Numpad3",
    "Numpad4",
    "Numpad5",
    "Numpad6",
    "Numpad7",
    "Numpad8",
    "Numpad9",
    "NumpadAdd",
    "NumpadSubtract",
    "NumpadMultiply",
    "NumpadDivide",
    "NumpadDecimal",
    "NumpadEnter",
    "AudioVolumeMute",
    "AudioVolumeDown",
    "AudioVolumeUp",
    "MediaTrackNext",
    "MediaTrackPrevious",
    "MediaStop",
    "MediaPlayPause",
    "LaunchMail",
    "LaunchApp2",
    "BrowserBack",
    "BrowserForward",
    "BrowserRefresh",
    "BrowserSearch",
    "BrowserHome",
];

/// The keys that may make a shortcut alone, besides F1 to F24 and Pause: nobody types with them.
const STANDALONE: &[&str] = &[
    "AudioVolumeMute",
    "AudioVolumeDown",
    "AudioVolumeUp",
    "MediaTrackNext",
    "MediaTrackPrevious",
    "MediaStop",
    "MediaPlayPause",
    "LaunchMail",
    "LaunchApp2",
    "BrowserBack",
    "BrowserForward",
    "BrowserRefresh",
    "BrowserSearch",
    "BrowserHome",
];

/// The keys the system's shortcut service (the global-shortcut plugin) can hold, with modifiers.
fn system_key(key: &str) -> bool {
    !matches!(
        key,
        "IntlBackslash"
            | "IntlRo"
            | "IntlYen"
            | "ContextMenu"
            | "NumLock"
            | "LaunchMail"
            | "LaunchApp2"
            | "NumpadEnter"
    ) && !key.starts_with("Browser")
}

/// Is this canonical shortcut held by the system (modifiers and one key it knows)? Otherwise
/// CargoFrame watches the keys itself.
pub fn held_by_system(canonical: &str) -> bool {
    let (_, keys) = split(canonical);
    keys.len() == 1 && system_key(keys[0])
}

/// The keys of a canonical shortcut as browsers name them (`Ctrl+KeyA+Digit1`), which is how
/// `cargoframe-keys` finds them on the keyboard. Modifiers stay as they are.
pub fn key_codes(canonical: &str) -> String {
    canonical
        .split('+')
        .map(|k| {
            let mut c = k.chars();
            match (c.next(), c.next()) {
                (Some(l), None) if l.is_ascii_uppercase() => format!("Key{l}"),
                (Some(d), None) if d.is_ascii_digit() => format!("Digit{d}"),
                _ => k.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Modifiers and keys of a canonical shortcut.
fn split(canonical: &str) -> (Vec<&str>, Vec<&str>) {
    canonical
        .split('+')
        .filter(|t| !t.is_empty())
        .partition(|t| MODIFIER_ORDER.contains(t))
}

/// Parse and canonicalize. `"ctrl+shift+f9"` becomes `"Ctrl+Shift+F9"`.
pub fn normalize(input: &str) -> Result<String, ShortcutError> {
    let tokens: Vec<&str> = input
        .split('+')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err(ShortcutError::Empty);
    }
    let mut mods: Vec<&'static str> = Vec::new();
    let mut keys: Vec<(String, bool)> = Vec::new();
    for token in tokens {
        if let Some(m) = modifier(token) {
            if mods.contains(&m) {
                return Err(ShortcutError::RepeatedModifier(m));
            }
            mods.push(m);
        } else if let Some((canon, alone)) = key(token) {
            if keys.iter().any(|(k, _)| *k == canon) {
                return Err(ShortcutError::RepeatedKey(canon));
            }
            keys.push((canon, alone));
        } else {
            return Err(ShortcutError::UnknownKey(token.to_string()));
        }
    }
    match keys.len() {
        0 => return Err(ShortcutError::NoKey),
        1 if mods.is_empty() && !keys[0].1 => return Err(ShortcutError::NeedsModifier),
        n if n > 3 => return Err(ShortcutError::TooManyKeys),
        _ => {}
    }
    mods.sort_by_key(|m| MODIFIER_ORDER.iter().position(|o| o == m));
    // several keys are one set, whatever the order they were pressed in
    keys.sort();
    let mut out: Vec<&str> = mods;
    out.extend(keys.iter().map(|(k, _)| k.as_str()));
    Ok(out.join("+"))
}

/// A canonical shortcut split into its modifiers and its (last) key. A desktop's shortcut
/// settings take one key: [`desktop_ready`] says whether a shortcut fits.
fn parts(canonical: &str) -> (Vec<&str>, &str) {
    let (mods, keys) = split(canonical);
    (mods, keys.last().copied().unwrap_or(""))
}

/// Can a Wayland desktop hold this shortcut (modifiers and one key)?
pub fn desktop_ready(canonical: &str) -> bool {
    split(canonical).1.len() == 1
}

/// The X keysym name of a canonical key (`F9`, `a`, `bracketleft`, `Return`), the name Wayland
/// desktops use in their shortcut settings.
pub fn keysym(key: &str) -> String {
    let named = match key {
        "Minus" => "minus",
        "Equal" => "equal",
        "BracketLeft" => "bracketleft",
        "BracketRight" => "bracketright",
        "Semicolon" => "semicolon",
        "Quote" => "apostrophe",
        "Comma" => "comma",
        "Period" => "period",
        "Slash" => "slash",
        "Backslash" => "backslash",
        "Backquote" => "grave",
        "Space" => "space",
        "Enter" => "Return",
        "Tab" => "Tab",
        "Backspace" => "BackSpace",
        "Delete" => "Delete",
        "Insert" => "Insert",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "Page_Up",
        "PageDown" => "Page_Down",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "PrintScreen" => "Print",
        "Pause" => "Pause",
        "ScrollLock" => "Scroll_Lock",
        "Escape" => "Escape",
        "CapsLock" => "Caps_Lock",
        "NumLock" => "Num_Lock",
        "ContextMenu" => "Menu",
        "IntlBackslash" => "less",
        "NumpadAdd" => "KP_Add",
        "NumpadSubtract" => "KP_Subtract",
        "NumpadMultiply" => "KP_Multiply",
        "NumpadDivide" => "KP_Divide",
        "NumpadDecimal" => "KP_Decimal",
        "NumpadEnter" => "KP_Enter",
        "AudioVolumeMute" => "XF86AudioMute",
        "AudioVolumeDown" => "XF86AudioLowerVolume",
        "AudioVolumeUp" => "XF86AudioRaiseVolume",
        "MediaPlayPause" => "XF86AudioPlay",
        "MediaStop" => "XF86AudioStop",
        "MediaTrackNext" => "XF86AudioNext",
        "MediaTrackPrevious" => "XF86AudioPrev",
        other if other.starts_with("Numpad") => return format!("KP_{}", &other[6..]),
        // F1..F24 keep their name; a letter is lower case
        other if other.len() == 1 => return other.to_ascii_lowercase(),
        other => other,
    };
    named.to_string()
}

/// GNOME's spelling: `<Control><Shift>F9`.
pub fn for_gnome(canonical: &str) -> String {
    let (mods, key) = parts(canonical);
    let mut out: String = mods
        .iter()
        .map(|m| match *m {
            "Ctrl" => "<Control>",
            "Alt" => "<Alt>",
            "Shift" => "<Shift>",
            _ => "<Super>",
        })
        .collect();
    out.push_str(&keysym(key));
    out
}

/// The desktop portal's spelling (XDG shortcuts): `CTRL+SHIFT+F9`.
pub fn for_portal(canonical: &str) -> String {
    let (mods, key) = parts(canonical);
    let mut out: Vec<String> = mods
        .iter()
        .map(|m| {
            match *m {
                "Ctrl" => "CTRL",
                "Alt" => "ALT",
                "Shift" => "SHIFT",
                _ => "LOGO",
            }
            .to_string()
        })
        .collect();
    out.push(keysym(key));
    out.join("+")
}

/// sway's spelling: `Ctrl+Shift+F9`, with `Mod4` for the Super key.
pub fn for_sway(canonical: &str) -> String {
    let (mods, key) = parts(canonical);
    let mut out: Vec<String> = mods
        .iter()
        .map(|m| if *m == "Super" { "Mod4" } else { m }.to_string())
        .collect();
    out.push(keysym(key));
    out.join("+")
}

/// Hyprland's spelling, the two first fields of a `bind`: `CTRL SHIFT, F9`.
pub fn for_hyprland(canonical: &str) -> String {
    let (mods, key) = parts(canonical);
    let mods: Vec<String> = mods.iter().map(|m| m.to_ascii_uppercase()).collect();
    format!("{}, {}", mods.join(" "), keysym(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_form_orders_modifiers_and_fixes_case() {
        assert_eq!(normalize("shift + ctrl + f9").unwrap(), "Ctrl+Shift+F9");
        assert_eq!(normalize("Win+Alt+c").unwrap(), "Alt+Super+C");
        assert_eq!(normalize("CmdOrCtrl+Shift+S").unwrap(), "Ctrl+Shift+S");
        assert_eq!(normalize("ctrl+prtsc").unwrap(), "Ctrl+PrintScreen");
        assert_eq!(normalize("Ctrl+Alt+/").unwrap(), "Ctrl+Alt+Slash");
        assert_eq!(normalize("Ctrl+1").unwrap(), "Ctrl+1");
    }

    #[test]
    fn each_desktop_gets_its_own_spelling() {
        assert_eq!(for_gnome("Ctrl+Shift+F9"), "<Control><Shift>F9");
        assert_eq!(for_gnome("Alt+Super+C"), "<Alt><Super>c");
        assert_eq!(for_portal("Ctrl+Shift+F9"), "CTRL+SHIFT+F9");
        assert_eq!(for_portal("Super+PrintScreen"), "LOGO+Print");
        assert_eq!(for_sway("Ctrl+Alt+Slash"), "Ctrl+Alt+slash");
        assert_eq!(for_sway("Super+Quote"), "Mod4+apostrophe");
        assert_eq!(for_hyprland("Ctrl+Shift+F9"), "CTRL SHIFT, F9");
        assert_eq!(for_hyprland("F12"), ", F12");
        assert_eq!(keysym("PageDown"), "Page_Down");
        assert_eq!(keysym("1"), "1");
    }

    #[test]
    fn function_keys_may_stand_alone_but_letters_may_not() {
        assert_eq!(normalize("f12").unwrap(), "F12");
        assert_eq!(normalize("a"), Err(ShortcutError::NeedsModifier));
        assert_eq!(normalize("PrintScreen"), Err(ShortcutError::NeedsModifier));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(normalize(""), Err(ShortcutError::Empty));
        assert_eq!(normalize("  +  "), Err(ShortcutError::Empty));
        assert_eq!(normalize("Ctrl+Shift"), Err(ShortcutError::NoKey));
        assert_eq!(normalize("Ctrl+A+B+C+D"), Err(ShortcutError::TooManyKeys));
        assert_eq!(
            normalize("Ctrl+A+a"),
            Err(ShortcutError::RepeatedKey("A".into()))
        );
        assert!(matches!(
            normalize("Ctrl+Banana"),
            Err(ShortcutError::UnknownKey(_))
        ));
        assert_eq!(
            normalize("Ctrl+Control+A"),
            Err(ShortcutError::RepeatedModifier("Ctrl"))
        );
        assert!(matches!(
            normalize("F25"),
            Err(ShortcutError::UnknownKey(_))
        ));
    }

    #[test]
    fn several_keys_and_keys_of_every_kind_make_a_shortcut() {
        // a set of keys, whatever the order they were pressed in
        assert_eq!(normalize("ctrl+s+a").unwrap(), "Ctrl+A+S");
        assert_eq!(normalize("A+S").unwrap(), "A+S");
        assert_eq!(
            normalize("Ctrl+IntlBackslash").unwrap(),
            "Ctrl+IntlBackslash"
        );
        assert_eq!(normalize("ctrl+numpadadd").unwrap(), "Ctrl+NumpadAdd");
        assert_eq!(normalize("MediaPlayPause").unwrap(), "MediaPlayPause");
        assert_eq!(normalize("Pause").unwrap(), "Pause");
        assert_eq!(normalize("Numpad5"), Err(ShortcutError::NeedsModifier));
        // which the system holds, and which CargoFrame watches
        assert!(held_by_system("Ctrl+Shift+F9"));
        assert!(held_by_system("Ctrl+NumpadAdd"));
        assert!(!held_by_system("Ctrl+A+S"));
        assert!(!held_by_system("Ctrl+IntlBackslash"));
        assert_eq!(key_codes("Ctrl+A+1+F9"), "Ctrl+KeyA+Digit1+F9");
        // a desktop takes one key
        assert!(desktop_ready("Ctrl+IntlBackslash"));
        assert!(!desktop_ready("Ctrl+A+S"));
        assert_eq!(for_gnome("Ctrl+IntlBackslash"), "<Control>less");
        assert_eq!(keysym("Numpad7"), "KP_7");
    }

    #[test]
    fn normalizing_twice_changes_nothing() {
        for s in [
            "Ctrl+Shift+F9",
            "Alt+Super+C",
            "F1",
            "Ctrl+PageUp",
            "Ctrl+A+S",
            "Ctrl+NumpadAdd",
        ] {
            assert_eq!(normalize(s).unwrap(), s);
        }
    }
}
