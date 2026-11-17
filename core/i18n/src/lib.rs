//! The language of what CargoFrame says: connector labels and errors, template titles, checks,
//! the tray and notifications. English is written in the code; each other language is a pack,
//! `packs/<code>.json`, that maps the English text to its translation. Logs stay in English.
//!
//! - `t("Create issue")` gives the text in the language in use.
//! - `tf("Signed in to {host} as {who}", &[("host", &host), ("who", &who)])` fills `{names}` in
//!   after translating, so a translation may move them.
//! - `msg!("…")` only marks a text for the packs, where `t` cannot run (a `const`); translate it
//!   with `t` where it is shown.
//!
//! `scripts/i18n/check.mjs` reads these calls: every text must be in every pack, and no pack may
//! keep a text the code no longer has (docs/i18n.md).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// The languages CargoFrame speaks: code, name in that language. English first.
pub const LANGUAGES: [(&str, &str); 5] = [
    ("en", "English"),
    ("fr", "Français"),
    ("es", "Español"),
    ("it", "Italiano"),
    ("de", "Deutsch"),
];

const PACKS: [&str; 5] = [
    "{}",
    include_str!("../packs/fr.json"),
    include_str!("../packs/es.json"),
    include_str!("../packs/it.json"),
    include_str!("../packs/de.json"),
];

/// Marks a text for the packs without translating it (for a `const`). `t` translates it later.
#[macro_export]
macro_rules! msg {
    ($s:literal) => {
        $s
    };
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static LOADED: [OnceLock<HashMap<String, &'static str>>; 5] = [
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
];

/// The pack of language `i`, read the first time it is needed.
fn pack(i: usize) -> &'static HashMap<String, &'static str> {
    LOADED[i].get_or_init(|| {
        let map: HashMap<String, String> = serde_json::from_str(PACKS[i]).unwrap_or_default();
        map.into_iter()
            .filter(|(_, v)| !v.is_empty())
            // a pack lives as long as the program: each text is kept once
            .map(|(k, v)| (k, &*Box::leak(v.into_boxed_str())))
            .collect()
    })
}

/// The language a setting asks for: `"system"` (or anything unknown) follows the computer's
/// language, else the code itself when CargoFrame speaks it. English when nothing matches.
pub fn resolve(setting: &str, system: Option<&str>) -> &'static str {
    let wanted = setting.trim().to_ascii_lowercase();
    let from = if wanted.is_empty() || wanted == "system" {
        system.unwrap_or("en").to_ascii_lowercase()
    } else {
        wanted
    };
    // "fr-FR", "fr_CA.UTF-8", "de" → the two first letters
    let code: String = from
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or("en", |(c, _)| c)
}

/// The computer's language, as the system says it ("fr-FR").
pub fn system_language() -> Option<String> {
    sys_locale::get_locale()
}

/// Use language `code` from now on (see [`resolve`]).
pub fn set_language(code: &str) {
    let i = LANGUAGES.iter().position(|(c, _)| *c == code).unwrap_or(0);
    CURRENT.store(i, Ordering::Relaxed);
}

/// The language in use.
pub fn language() -> &'static str {
    LANGUAGES[CURRENT.load(Ordering::Relaxed)].0
}

/// `text` in the language in use; the English text when the pack has none. A text that is not
/// written in the call itself (a `what` handed down) must be marked with `msg!` where it is written.
pub fn t(text: &str) -> &str {
    let i = CURRENT.load(Ordering::Relaxed);
    if i == 0 {
        return text;
    }
    pack(i).get(text).copied().unwrap_or(text)
}

/// `text` translated, then each `{name}` filled with its value. A value is never read again.
pub fn tf(text: &str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    fill(t(text), values)
}

/// Fill `{name}` in `text` with its value; an unknown `{name}` stays as it is.
pub fn fill(text: &str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match values.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(&v.to_string()),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_or_the_system_picks_a_language_english_when_none_matches() {
        assert_eq!(resolve("system", Some("fr-FR")), "fr");
        assert_eq!(resolve("", Some("de_DE.UTF-8")), "de");
        assert_eq!(resolve("system", Some("pt-BR")), "en");
        assert_eq!(resolve("system", None), "en");
        assert_eq!(resolve("it", Some("fr-FR")), "it");
        assert_eq!(resolve("ES", None), "es");
        assert_eq!(resolve("klingon", Some("es-MX")), "en");
    }

    #[test]
    fn values_are_filled_after_translating_and_never_read_again() {
        assert_eq!(
            fill(
                "Signed in to {host} as {who}",
                &[("host", &"a.b"), ("who", &"{host}")]
            ),
            "Signed in to a.b as {host}"
        );
        assert_eq!(fill("keep {this} and {", &[]), "keep {this} and {");
    }

    #[test]
    fn every_pack_is_json_of_texts() {
        for (i, (code, _)) in LANGUAGES.iter().enumerate().skip(1) {
            let map: Result<HashMap<String, String>, _> = serde_json::from_str(PACKS[i]);
            assert!(map.is_ok(), "packs/{code}.json is not a map of texts");
        }
        assert_eq!(msg!("Save"), "Save");
    }

    #[test]
    fn a_chosen_language_is_spoken_and_english_stays_the_fallback() {
        // the only test that changes the language, and it puts English back
        set_language("fr");
        assert_eq!(t("Quit CargoFrame"), "Quitter CargoFrame");
        assert_eq!(
            tf(
                "Signed in to {host} as {who}",
                &[("host", &"acme"), ("who", &"Lou")]
            ),
            "Connecté à acme en tant que Lou"
        );
        assert_eq!(t("no pack has this text"), "no pack has this text");
        set_language("de");
        assert_eq!(t("Quit CargoFrame"), "CargoFrame beenden");
        set_language("en");
        assert_eq!(t("Quit CargoFrame"), "Quit CargoFrame");
    }
}
