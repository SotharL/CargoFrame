//! The language CargoFrame speaks: chosen in Settings, General, or the computer's own.

use serde::Serialize;

/// Use the language `setting` asks for (`system`, `fr`…) from now on; gives its code.
pub fn apply(setting: &str) -> &'static str {
    let system = cargoframe_i18n::system_language();
    let code = cargoframe_i18n::resolve(setting, system.as_deref());
    cargoframe_i18n::set_language(code);
    code
}

/// What the windows need to speak the same language as the program.
#[derive(Debug, Serialize)]
pub struct LanguageInfo {
    /// The language in use.
    pub code: &'static str,
    /// The computer's language, as the system says it.
    pub system: Option<String>,
    /// Every language CargoFrame speaks: code and name in that language.
    pub languages: Vec<(&'static str, &'static str)>,
}

pub fn info() -> LanguageInfo {
    LanguageInfo {
        code: cargoframe_i18n::language(),
        system: cargoframe_i18n::system_language(),
        languages: cargoframe_i18n::LANGUAGES.to_vec(),
    }
}
