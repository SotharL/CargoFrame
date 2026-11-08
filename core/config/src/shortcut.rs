//! Keyboard shortcuts for triggers, written the way people type them ("ctrl + shift + f9") and
//! stored in one canonical form ("Ctrl+Shift+F9") that the global-shortcut plugin accepts.
//!
//! Rules: at least one modifier unless the key is a function key, one non-modifier key, no
//! repeated modifier. Modifier-only and empty shortcuts are refused.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutError {
    Empty,
    NoKey,
    TwoKeys(String, String),
    UnknownKey(String),
    RepeatedModifier(&'static str),
    NeedsModifier,
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Press the keys you want to use"),
            Self::NoKey => write!(
                f,
                "Add a key after the modifiers (for example Ctrl+Shift+F9)"
            ),
            Self::TwoKeys(a, b) => write!(f, "Only one key is allowed, found {a} and {b}"),
            Self::UnknownKey(k) => write!(f, "'{k}' is not a key CargoFrame can use"),
            Self::RepeatedModifier(m) => write!(f, "{m} is used twice"),
            Self::NeedsModifier => write!(f, "Add Ctrl, Alt, Shift or Win/Super to this key"),
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
        _ => return None,
    };
    Some((named.to_string(), false))
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
    let mut main: Option<(String, bool, String)> = None;
    for token in tokens {
        if let Some(m) = modifier(token) {
            if mods.contains(&m) {
                return Err(ShortcutError::RepeatedModifier(m));
            }
            mods.push(m);
        } else if let Some((canon, is_f)) = key(token) {
            if let Some((prev, _, _)) = &main {
                return Err(ShortcutError::TwoKeys(prev.clone(), canon));
            }
            main = Some((canon, is_f, token.to_string()));
        } else {
            return Err(ShortcutError::UnknownKey(token.to_string()));
        }
    }
    let (main, is_function, _) = main.ok_or(ShortcutError::NoKey)?;
    if mods.is_empty() && !is_function {
        return Err(ShortcutError::NeedsModifier);
    }
    mods.sort_by_key(|m| MODIFIER_ORDER.iter().position(|o| o == m));
    let mut out = mods.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&main);
    Ok(out)
}

/// A canonical shortcut split into its modifiers and its key.
fn parts(canonical: &str) -> (Vec<&str>, &str) {
    let mut tokens: Vec<&str> = canonical.split('+').collect();
    let key = tokens.pop().unwrap_or("");
    (tokens, key)
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
        assert!(matches!(
            normalize("Ctrl+A+B"),
            Err(ShortcutError::TwoKeys(_, _))
        ));
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
    fn normalizing_twice_changes_nothing() {
        for s in ["Ctrl+Shift+F9", "Alt+Super+C", "F1", "Ctrl+PageUp"] {
            assert_eq!(normalize(s).unwrap(), s);
        }
    }
}
