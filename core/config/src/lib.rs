//! CargoFrame configuration: one small, versioned JSON file per item.
//!
//! ```text
//! %AppData%\CargoFrame\            (Linux: ~/.config/CargoFrame/)
//!   general.json   safe.json   video.json
//!   triggers\<id>.json   connections\<id>.json   rules\<id>.json
//!   infotypes\<id>.json  layouts\<id>.json
//! ```
//!
//! * Every file carries `schema_version`. A file from a newer CargoFrame is never touched. A
//!   file from an older one is migrated step by step, after its original is copied to
//!   `<file>.bak-v<old>`.
//! * Writes are atomic (temporary file, then rename), so a crash never leaves a half-written
//!   file.
//! * Ids become file names, so they are restricted to `[a-z0-9_-]`, 1 to 64 characters.
//! * Secrets are never stored here; they live in the operating system's credential store.

pub mod error;
pub mod infotype;
pub mod models;
pub mod paths;
pub mod shortcut;
pub mod store;
pub mod triggers;

pub use error::ConfigError;
pub use infotype::{InfoKind, InfoOption, InfoOptions, InfoType};
pub use models::{
    normalize_accent, CaptureMode, General, Media, MemoryMode, Notifications, Safe, Theme, Trigger,
    Video, VideoCapture, VideoEncoder, DEFAULT_ACCENT,
};
pub use paths::Dirs;
pub use store::{valid_id, Document, Kind, Loaded, Store};
