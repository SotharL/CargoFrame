use crate::store::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
}

/// How long the window's web view stays alive after it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    /// Keep the view for 5 minutes, then free it.
    #[default]
    Balanced,
    /// Free it as soon as the window closes.
    LowMemory,
    /// Never free it: fastest to reopen, uses more memory.
    AlwaysWarm,
}

/// `general.json`. Whether CargoFrame runs at sign-in is not stored here: the operating
/// system's own startup entry is the single source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub schema_version: u32,
    pub theme: Theme,
    pub memory_mode: MemoryMode,
    /// Off while the repository is private (no public release assets to check).
    pub check_updates: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            schema_version: Self::SCHEMA,
            theme: Theme::default(),
            memory_mode: MemoryMode::default(),
            check_updates: false,
        }
    }
}

impl Document for General {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "general";
}

/// `safe.json`: what happens when sharing fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Safe {
    pub schema_version: u32,
    /// Folder for the fallback zip. Empty means "ask me".
    pub folder: Option<String>,
    /// Seconds before a send counts as timed out.
    pub timeout_secs: u32,
    /// Retries after the first attempt.
    pub max_retries: u32,
}

impl Default for Safe {
    fn default() -> Self {
        Self {
            schema_version: Self::SCHEMA,
            folder: None,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

impl Document for Safe {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "safe";
}
