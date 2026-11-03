use crate::store::Document;
use cargoframe_handoff::ChatList;
use cargoframe_layout::Layout;
use cargoframe_rules::Rule;
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

/// How the screen is captured for a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCapture {
    /// CargoFrame picks what suits this system.
    #[default]
    Auto,
    /// Windows, the old way (GDI): works almost everywhere, costs more CPU.
    Gdigrab,
    /// Windows, Desktop Duplication: the picture stays on the graphics card.
    Ddagrab,
    /// Linux with an X11 session.
    X11,
}

/// How a video is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoEncoder {
    /// CargoFrame picks what suits this system.
    #[default]
    Auto,
    /// On the processor: works everywhere.
    Software,
    Nvenc,
    Qsv,
    Amf,
    /// Windows' own encoder.
    MediaFoundation,
    Vaapi,
}

/// `video.json`: how videos are recorded. Nothing here records by itself; a recording starts
/// from a trigger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Video {
    pub schema_version: u32,
    /// Folder that holds ffmpeg and ffprobe, or the ffmpeg program itself. Empty means "look
    /// next to CargoFrame, then on the search path".
    pub ffmpeg: Option<String>,
    pub capture: VideoCapture,
    pub encoder: VideoEncoder,
    pub fps: u32,
    /// How many seconds before the stop shortcut the clip starts, at most.
    pub window_secs: u32,
    /// Larger pictures are scaled down to this height. 0 keeps the screen's own size.
    pub max_height: u32,
}

impl Video {
    pub const MIN_FPS: u32 = 5;
    pub const MAX_FPS: u32 = 60;
    pub const MAX_WINDOW_SECS: u32 = 30;
}

impl Default for Video {
    fn default() -> Self {
        Self {
            schema_version: Self::SCHEMA,
            ffmpeg: None,
            capture: VideoCapture::Auto,
            encoder: VideoEncoder::Auto,
            fps: 30,
            window_secs: 10,
            max_height: 1080,
        }
    }
}

impl Document for Video {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "video";
}

/// What a trigger captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    /// The window in focus; the display it is on when there is none (or it cannot be read).
    #[default]
    ActiveWindow,
    /// The display under the mouse pointer.
    Display,
    /// Every display as one picture.
    AllDisplays,
}

/// What a trigger makes: a picture of the screen, or a recording of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Media {
    #[default]
    Screenshot,
    /// The first press starts recording, the next one stops it and opens the window with the
    /// clip (see Settings, Video).
    Video,
}

/// `triggers/<id>.json`: a shortcut that captures and opens one layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trigger {
    pub schema_version: u32,
    pub name: String,
    pub enabled: bool,
    /// Canonical form, e.g. `Ctrl+Shift+F9`. Empty means "tray menu only".
    pub shortcut: String,
    pub capture: CaptureMode,
    /// A screenshot (the default, and what a trigger file without this field means) or a video.
    pub media: Media,
    /// `layouts/<id>.json`. Each trigger has its own layout in V1.
    pub layout_id: Option<String>,
}

impl Default for Trigger {
    fn default() -> Self {
        Self {
            schema_version: Self::SCHEMA,
            name: String::new(),
            enabled: true,
            shortcut: String::new(),
            capture: CaptureMode::default(),
            media: Media::default(),
            layout_id: None,
        }
    }
}

impl Document for Trigger {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "trigger";
}

/// `layouts/<id>.json` is the layout model itself.
impl Document for Layout {
    const SCHEMA: u32 = cargoframe_layout::SCHEMA_VERSION;
    const NAME: &'static str = "layout";
}

/// `rules/<id>.json` is the data rule itself.
impl Document for Rule {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "rule";
}

/// `infotypes/<id>.json`.
impl Document for crate::infotype::InfoType {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "infotype";
}

/// `chats.json`: pinned and recent Claude chats. Stored beside `general.json`.
impl Document for ChatList {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "chats";
}

/// `connections/<id>.json`: how to reach one service. Secrets are not in it.
impl Document for cargoframe_connectors::Connection {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "connection";
}
