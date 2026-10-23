//! Layout data model.

/// A cell rectangle on the grid. `x` and `w` count columns, `y` and `h` count rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub id: String,
    pub name: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    pub rect: Rect,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Text(TextBlock),
    Screenshot,
    SharingButton(SharingButton),
    Dropdown(DropdownBlock),
    Files(FilesBlock),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlePlacement {
    Above,
    Inside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Plain,
    /// The text sent to an AI destination. A sharing button to an AI tool needs one.
    Prompt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    pub title: String,
    pub placement: TitlePlacement,
    pub role: TextRole,
    /// Initial content; may use `{{variables}}`.
    pub default_text: String,
}

/// Where a value for an info-type field comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueSource {
    /// A text or dropdown block of this layout.
    Block(String),
    /// A data rule, by id.
    Rule(String),
    Fixed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoMapping {
    /// Field name from the connection's info type.
    pub field: String,
    pub source: ValueSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnSuccess {
    CloseWindow,
    KeepOpen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharingButton {
    pub label: String,
    pub connection_id: Option<String>,
    /// For AI destinations: the text block (role Prompt) that holds the prompt.
    pub prompt_block_id: Option<String>,
    pub mappings: Vec<InfoMapping>,
    pub on_success: OnSuccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownSource {
    /// Options fetched from a web service through a connection.
    WebApi {
        connection_id: String,
        options_id: String,
    },
    /// Pinned and recent Claude chats, plus "New chat".
    ClaudeChats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownBlock {
    pub title: String,
    pub source: DropdownSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesBlock {
    pub title: String,
    /// Globs, relative to the resolved project folder, listing the files to offer.
    pub include: Vec<String>,
    /// Globs of the files ticked by default.
    pub preselect: Vec<String>,
}
