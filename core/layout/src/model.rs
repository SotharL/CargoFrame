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
    SharingButton(SharingButton),
    Dropdown(DropdownBlock),
    Files(FilesBlock),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesBlock {
    #[serde(default)]
    pub title: String,
    /// Globs, relative to the resolved project folder, listing the files to offer.
    #[serde(default)]
    pub include: Vec<String>,
    /// Globs of the files ticked by default.
    #[serde(default)]
    pub preselect: Vec<String>,
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
                    kind: BlockKind::Dropdown(DropdownBlock {
                        title: "Chat".into(),
                        source: DropdownSource::ClaudeChats,
                    }),
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
