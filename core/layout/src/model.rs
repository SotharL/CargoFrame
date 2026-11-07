//! Layout data model. Serialized as-is into `layouts/<id>.json` and sent to the Svelte builder.

use serde::{Deserialize, Serialize};

/// A cell rectangle on the grid. `x` and `w` count columns, `y` and `h` count rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    /// Taken from the file name when loaded.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

impl Layout {
    /// A new layout has no blocks: the window opens empty with an "Edit layout" hint.
    pub fn empty(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            blocks: Vec::new(),
        }
    }

    pub fn block(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub rect: Rect,
    #[serde(flatten)]
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockKind {
    Text(TextBlock),
    Screenshot,
    /// The recording a video trigger made: played, trimmed, cropped and marked in the window.
    /// What is recorded (the screen or the window in focus) and which sounds go in is set here.
    Video(VideoBlock),
    SharingButton(SharingButton),
    Dropdown(DropdownBlock),
    Files(FilesBlock),
}

/// What a video trigger records, set in the layout the trigger opens.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoBlock {
    /// What is recorded.
    pub subject: VideoSubject,
    /// Which sounds go into the recording.
    pub audio: VideoAudio,
    /// With a screenshot block in the same layout: start it filled with the picture taken when
    /// the recording stopped. Off, it starts empty and the person takes one from the recording.
    pub screenshot_at_end: bool,
}

/// What a recording shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoSubject {
    /// The whole screen.
    #[default]
    Screen,
    /// The window that has the focus when the recording starts, as it is placed and sized then.
    FocusedWindow,
}

/// Where one sound comes from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioChoice {
    /// Not recorded.
    Off,
    /// The system's own choice: the sound the computer plays (for the desktop sound), or the
    /// default microphone.
    #[default]
    Default,
    /// One device, by the name the system gives it.
    Device(String),
}

/// The sounds that go into a recording: what the computer plays, and what a microphone hears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoAudio {
    /// The sound of the computer. On by default.
    pub desktop: AudioChoice,
    /// A microphone. Off by default.
    pub microphone: AudioChoice,
}

impl Default for VideoAudio {
    fn default() -> Self {
        Self {
            desktop: AudioChoice::Default,
            microphone: AudioChoice::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitlePlacement {
    #[default]
    Above,
    Inside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    #[default]
    Plain,
    /// The text sent to an AI destination. A sharing button to an AI tool needs one.
    Prompt,
    /// The name of the zip saved locally (a share with no connection, or one that kept a copy).
    /// Only the first block with this role is used.
    ZipName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextBlock {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub placement: TitlePlacement,
    #[serde(default)]
    pub role: TextRole,
    /// Initial content; may use `{{variables}}`.
    #[serde(default)]
    pub default_text: String,
}

/// Where a value for an info-type field comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ValueSource {
    /// A text or dropdown block of this layout.
    Block(String),
    /// A data rule, by id.
    Rule(String),
    Fixed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoMapping {
    /// Field name from the connection's info type.
    pub field: String,
    pub source: ValueSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnSuccess {
    #[default]
    CloseWindow,
    KeepOpen,
}

/// What happens to the ticked files when a button is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesHow {
    /// Sent as files (attached, pasted, or zipped, as the destination does it).
    #[default]
    Attach,
    /// Text files that fit the size cap are written into the message; the rest are attached.
    /// Only a destination that takes a message (a Claude chat) can do this.
    Inline,
    /// The ticked files stay out of this button's share.
    None,
}

/// The size cap for files written into a message, in kilobytes, and the most it may be set to.
pub const DEFAULT_INLINE_KB: u32 = 64;
pub const MAX_INLINE_KB: u32 = 512;

fn yes() -> bool {
    true
}

fn default_inline_kb() -> u32 {
    DEFAULT_INLINE_KB
}

/// What one button sends beyond its text: the picture and the files. The prompt, the title and
/// the values are chosen by the button's own prompt block and field list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendRules {
    #[serde(default = "yes")]
    pub screenshot: bool,
    #[serde(default)]
    pub files: FilesHow,
    /// With `files` set to inline: the most text, in kilobytes, written into the message.
    #[serde(default = "default_inline_kb")]
    pub inline_limit_kb: u32,
}

impl Default for SendRules {
    fn default() -> Self {
        Self {
            screenshot: true,
            files: FilesHow::Attach,
            inline_limit_kb: DEFAULT_INLINE_KB,
        }
    }
}

impl SendRules {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The cap in bytes, kept inside what can be set.
    pub fn inline_limit_bytes(&self) -> usize {
        self.inline_limit_kb.clamp(1, MAX_INLINE_KB) as usize * 1024
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SharingButton {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub connection_id: Option<String>,
    /// For AI destinations: the text block (role Prompt) that holds the prompt.
    #[serde(default)]
    pub prompt_block_id: Option<String>,
    #[serde(default)]
    pub mappings: Vec<InfoMapping>,
    #[serde(default)]
    pub on_success: OnSuccess,
    /// Whether the picture and the files go with it. Absent from the file when untouched.
    #[serde(default, skip_serializing_if = "SendRules::is_default")]
    pub send: SendRules,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DropdownSource {
    /// Options fetched from a web service through a connection.
    WebApi {
        connection_id: String,
        options_id: String,
    },
    /// Pinned and recent Claude chats, plus "New chat".
    ClaudeChats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropdownBlock {
    #[serde(default)]
    pub title: String,
    pub source: DropdownSource,
    /// Keep the last five choices at the top of the list and start on the last one used.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub remember: bool,
    /// The person may leave it empty, or type or paste something that is not in the list (a
    /// thread's link). With no list at all it is just a text box.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    /// The text block whose words narrow the list while it is shown, so a thread named like the
    /// title is found at once. Empty: no narrowing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrow_by: Option<String>,
}

impl DropdownBlock {
    /// A dropdown with the usual behaviour: remembers its choices, must be answered, shows all.
    pub fn new(title: &str, source: DropdownSource) -> Self {
        Self {
            title: title.to_string(),
            source,
            remember: true,
            optional: false,
            narrow_by: None,
        }
    }
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesBlock {
    #[serde(default)]
    pub title: String,
    /// Globs, relative to each folder looked in, listing the files to offer.
    #[serde(default)]
    pub include: Vec<String>,
    /// Globs of the files ticked by default.
    #[serde(default)]
    pub preselect: Vec<String>,
    /// Which folders to look in. The default is the project folder.
    #[serde(default)]
    pub look_in: LookIn,
}

/// A folder a Files block can look in, found from the program that had focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LookFolder {
    /// The project folder worked out for the capture (see the brief): the default.
    #[serde(rename = "project")]
    Project,
    /// The folder the program file sits in.
    #[serde(rename = "program")]
    Program,
    /// One folder above the program's folder.
    #[serde(rename = "above_1")]
    Above1,
    #[serde(rename = "above_2")]
    Above2,
    #[serde(rename = "above_3")]
    Above3,
}

impl LookFolder {
    /// How many folders above the program's own folder, for the choices that count that way.
    pub fn levels_above_program(self) -> Option<usize> {
        match self {
            LookFolder::Project => None,
            LookFolder::Program => Some(0),
            LookFolder::Above1 => Some(1),
            LookFolder::Above2 => Some(2),
            LookFolder::Above3 => Some(3),
        }
    }
}

/// Where a Files block looks: some folders, and a named subfolder inside each (such as `logs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookIn {
    /// Empty means the project folder, like the default.
    #[serde(default = "default_folders")]
    pub folders: Vec<LookFolder>,
    /// A folder inside each of the folders above; empty means the folder itself.
    #[serde(default)]
    pub subfolder: String,
}

fn default_folders() -> Vec<LookFolder> {
    vec![LookFolder::Project]
}

impl Default for LookIn {
    fn default() -> Self {
        Self {
            folders: default_folders(),
            subfolder: String::new(),
        }
    }
}

impl LookIn {
    /// The folders to look in, in a fixed order and without repeats; the project folder when
    /// none was chosen.
    pub fn folders(&self) -> Vec<LookFolder> {
        let mut out = self.folders.clone();
        out.sort();
        out.dedup();
        if out.is_empty() {
            out.push(LookFolder::Project);
        }
        out
    }

    /// The subfolder as a clean relative path with `/` separators: `None` when none is set,
    /// `Err` when it is not a plain relative folder (absolute, drive letter, `..`).
    pub fn clean_subfolder(&self) -> Result<Option<String>, NotASubfolder> {
        clean_subfolder(&self.subfolder)
    }
}

/// The text is not a plain relative folder: it could point outside the folder it is joined to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotASubfolder;

/// `logs`, `\build\logs\` or `./logs` become `logs`, `build/logs`, `logs`. Anything that could
/// point outside the folder it is joined to is refused.
pub fn clean_subfolder(raw: &str) -> Result<Option<String>, NotASubfolder> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.contains('\0') || raw.contains(':') || raw.starts_with(['/', '\\']) {
        return Err(NotASubfolder);
    }
    let mut parts = Vec::new();
    for part in raw.split(['/', '\\']) {
        match part.trim() {
            "" | "." => {}
            ".." => return Err(NotASubfolder),
            p => parts.push(p.to_string()),
        }
    }
    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("/")))
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    #[test]
    fn layout_round_trips_with_a_flat_block_shape() {
        let layout = Layout {
            id: "bugs".into(),
            name: "Bug report".into(),
            blocks: vec![
                Block {
                    id: "shot".into(),
                    rect: Rect {
                        x: 0,
                        y: 0,
                        w: 8,
                        h: 6,
                    },
                    kind: BlockKind::Screenshot,
                },
                Block {
                    id: "prompt".into(),
                    rect: Rect {
                        x: 8,
                        y: 0,
                        w: 4,
                        h: 3,
                    },
                    kind: BlockKind::Text(TextBlock {
                        title: "Prompt".into(),
                        placement: TitlePlacement::Inside,
                        role: TextRole::Prompt,
                        default_text: "Look at {{app.title}}".into(),
                    }),
                },
                Block {
                    id: "send".into(),
                    rect: Rect {
                        x: 8,
                        y: 3,
                        w: 4,
                        h: 1,
                    },
                    kind: BlockKind::SharingButton(SharingButton {
                        label: "Send".into(),
                        connection_id: Some("claude".into()),
                        prompt_block_id: Some("prompt".into()),
                        mappings: vec![InfoMapping {
                            field: "summary".into(),
                            source: ValueSource::Rule("version".into()),
                        }],
                        on_success: OnSuccess::KeepOpen,
                        send: Default::default(),
                    }),
                },
                Block {
                    id: "chat".into(),
                    rect: Rect {
                        x: 8,
                        y: 4,
                        w: 4,
                        h: 1,
                    },
                    kind: BlockKind::Dropdown(DropdownBlock::new(
                        "Chat",
                        DropdownSource::ClaudeChats,
                    )),
                },
            ],
        };
        let json = serde_json::to_value(&layout).unwrap();
        // The builder reads `type` next to the block's own fields.
        assert_eq!(json["blocks"][0]["type"], "screenshot");
        assert_eq!(json["blocks"][1]["type"], "text");
        assert_eq!(json["blocks"][1]["role"], "prompt");
        assert_eq!(json["blocks"][2]["mappings"][0]["source"]["kind"], "rule");
        assert_eq!(json["blocks"][3]["source"]["type"], "claude_chats");
        let back: Layout = serde_json::from_value(json).unwrap();
        assert_eq!(back, layout);
    }

    #[test]
    fn a_video_block_is_written_and_read_as_type_video() {
        let block: Block = serde_json::from_str(
            r#"{"id": "clip", "type": "video", "rect": {"x":0,"y":0,"w":8,"h":8}}"#,
        )
        .unwrap();
        assert_eq!(block.kind, BlockKind::Video(VideoBlock::default()));
        assert_eq!(serde_json::to_value(&block).unwrap()["type"], "video");
    }

    #[test]
    fn an_old_video_block_records_the_screen_with_the_computer_sound_and_no_microphone() {
        let block: Block = serde_json::from_str(
            r#"{"id": "clip", "type": "video", "rect": {"x":0,"y":0,"w":8,"h":8}}"#,
        )
        .unwrap();
        let BlockKind::Video(v) = &block.kind else {
            panic!("not video");
        };
        assert_eq!(v.subject, VideoSubject::Screen);
        assert_eq!(v.audio.desktop, AudioChoice::Default);
        assert_eq!(v.audio.microphone, AudioChoice::Off);
    }

    #[test]
    fn what_a_video_block_records_is_written_with_plain_names() {
        let block = Block {
            id: "clip".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 8,
                h: 8,
            },
            kind: BlockKind::Video(VideoBlock {
                subject: VideoSubject::FocusedWindow,
                audio: VideoAudio {
                    desktop: AudioChoice::Off,
                    microphone: AudioChoice::Device("USB Mic".into()),
                },
                screenshot_at_end: true,
            }),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "video");
        assert_eq!(json["subject"], "focused_window");
        assert_eq!(json["screenshot_at_end"], true);
        assert_eq!(json["audio"]["desktop"], "off");
        assert_eq!(
            json["audio"]["microphone"],
            serde_json::json!({"device": "USB Mic"})
        );
        assert_eq!(serde_json::from_value::<Block>(json).unwrap(), block);
        // each part may be left out of a file: it then takes its default
        let partial: Block = serde_json::from_str(
            r#"{"id":"c","type":"video","rect":{"x":0,"y":0,"w":8,"h":8},"audio":{"microphone":"default"}}"#,
        )
        .unwrap();
        let BlockKind::Video(v) = &partial.kind else {
            panic!("not video");
        };
        assert_eq!(v.audio.desktop, AudioChoice::Default);
        assert_eq!(v.audio.microphone, AudioChoice::Default);
        assert_eq!(v.subject, VideoSubject::Screen);
    }

    #[test]
    fn a_files_block_without_look_in_looks_in_the_project_folder() {
        let text = r#"{"id": "f", "type": "files", "rect": {"x":0,"y":0,"w":6,"h":2}, "include": ["*.log"]}"#;
        let block: Block = serde_json::from_str(text).unwrap();
        let BlockKind::Files(f) = &block.kind else {
            panic!("not files");
        };
        assert_eq!(f.look_in, LookIn::default());
        assert_eq!(f.look_in.folders(), vec![LookFolder::Project]);
        assert_eq!(f.look_in.clean_subfolder(), Ok(None));
    }

    #[test]
    fn look_in_is_written_with_plain_names() {
        let look = LookIn {
            folders: vec![LookFolder::Above2, LookFolder::Program, LookFolder::Above2],
            subfolder: "logs".into(),
        };
        let json = serde_json::to_value(&look).unwrap();
        assert_eq!(
            json["folders"],
            serde_json::json!(["above_2", "program", "above_2"])
        );
        let back: LookIn = serde_json::from_value(json).unwrap();
        assert_eq!(back, look);
        // Sorted, without repeats.
        assert_eq!(
            look.folders(),
            vec![LookFolder::Program, LookFolder::Above2]
        );
        // Nothing chosen means the project folder.
        let none = LookIn {
            folders: vec![],
            subfolder: String::new(),
        };
        assert_eq!(none.folders(), vec![LookFolder::Project]);
        assert_eq!(LookFolder::Above3.levels_above_program(), Some(3));
        assert_eq!(LookFolder::Project.levels_above_program(), None);
    }

    #[test]
    fn subfolders_are_plain_relative_folders() {
        let ok = |s: &str| clean_subfolder(s);
        assert_eq!(ok(""), Ok(None));
        assert_eq!(ok("  "), Ok(None));
        assert_eq!(ok("logs"), Ok(Some("logs".into())));
        assert_eq!(ok("./logs/"), Ok(Some("logs".into())));
        assert_eq!(ok("build\\logs"), Ok(Some("build/logs".into())));
        assert_eq!(ok("a//b"), Ok(Some("a/b".into())));
        for bad in [
            "..",
            "../x",
            "a/../b",
            "/etc",
            "\\\\server\\share",
            "C:\\x",
            "a:b",
            "x\0y",
        ] {
            assert_eq!(ok(bad), Err(NotASubfolder), "{bad:?}");
        }
    }

    #[test]
    fn the_zip_name_role_is_written_in_snake_case() {
        let text =
            r#"{"id": "z", "type": "text", "rect": {"x":0,"y":0,"w":6,"h":1}, "role": "zip_name"}"#;
        let block: Block = serde_json::from_str(text).unwrap();
        match &block.kind {
            BlockKind::Text(t) => assert_eq!(t.role, TextRole::ZipName),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(serde_json::to_value(&block).unwrap()["role"], "zip_name");
    }

    #[test]
    fn hand_written_minimal_blocks_load_with_defaults() {
        let text = r#"{
            "blocks": [
              {"id": "t", "type": "text", "rect": {"x":0,"y":0,"w":6,"h":2}},
              {"id": "b", "type": "sharing_button", "rect": {"x":6,"y":0,"w":6,"h":1}},
              {"id": "f", "type": "files", "rect": {"x":0,"y":2,"w":12,"h":3}, "include": ["*.log"]}
            ]
        }"#;
        let layout: Layout = serde_json::from_str(text).unwrap();
        assert_eq!(layout.blocks.len(), 3);
        match &layout.blocks[0].kind {
            BlockKind::Text(t) => {
                assert_eq!(t.role, TextRole::Plain);
                assert_eq!(t.placement, TitlePlacement::Above);
            }
            other => panic!("unexpected {other:?}"),
        }
        match &layout.blocks[1].kind {
            BlockKind::SharingButton(s) => assert!(s.connection_id.is_none()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn an_unknown_block_type_is_an_error_not_a_silent_drop() {
        let text = r#"{"blocks":[{"id":"x","type":"hologram","rect":{"x":0,"y":0,"w":1,"h":1}}]}"#;
        assert!(serde_json::from_str::<Layout>(text).is_err());
    }
}
