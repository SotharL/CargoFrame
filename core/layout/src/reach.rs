//! Which blocks a sharing button sends, and how.
//!
//! One answer for the builder (grey out what a button cannot send and say why), the capture
//! window (mark what each button sends) and the share itself (which block is the prompt, which
//! one picks the chat). It looks at the layout, the button's own send rules and the capability
//! table of the destination, and nothing else.

use crate::capabilities::{Capabilities, Capability};
use crate::model::{
    Block, BlockKind, DropdownSource, FilesHow, InfoMapping, Layout, SharingButton, TextRole,
    ValueSource,
};
use crate::validate::{sharing_mode, ConnectionKind, Registry, SharingMode};
use serde::Serialize;

/// How a block takes part in a share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Role {
    Screenshot,
    /// The recording of a video trigger; sent where the screenshot would be.
    Video,
    Files,
    /// The text block that holds the prompt.
    Prompt,
    /// Names the zip saved on this computer.
    ZipName,
    /// A value placed in a field of the destination (`summary`, `priority`, `chat`...).
    Field {
        field: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Verdict {
    /// The block goes with this button.
    Sent {
        #[serde(flatten)]
        role: Role,
    },
    /// The destination has no place for it. `why` is the capability note.
    CannotSend { why: &'static str },
    /// The button's own rules leave it out (picture or files switched off).
    SwitchedOff,
    /// The destination could take it, but this button is not set up to use it: a text or
    /// dropdown block that is neither the prompt nor mapped to a field.
    NotUsed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockReach {
    pub block_id: String,
    #[serde(flatten)]
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reach {
    pub capabilities: Capabilities,
    pub blocks: Vec<BlockReach>,
}

impl Reach {
    pub fn of(&self, block_id: &str) -> Option<&Verdict> {
        self.blocks
            .iter()
            .find(|b| b.block_id == block_id)
            .map(|b| &b.verdict)
    }
}

/// The mappings a share really uses: the button's own, plus the chat. A button to Claude with no
/// mapping for `chat` uses the layout's Claude chats dropdown, so picking a chat there is never
/// silently ignored.
pub fn effective_mappings(
    layout: &Layout,
    button: &SharingButton,
    kind: Option<ConnectionKind>,
) -> Vec<InfoMapping> {
    let mut out = button.mappings.clone();
    if kind == Some(ConnectionKind::Claude) && !out.iter().any(|m| m.field == "chat") {
        let chats = layout.blocks.iter().find(|b| {
            matches!(&b.kind, BlockKind::Dropdown(d) if d.source == DropdownSource::ClaudeChats)
        });
        if let Some(b) = chats {
            out.push(InfoMapping {
                field: "chat".into(),
                source: ValueSource::Block(b.id.clone()),
            });
        }
    }
    out
}

fn usable(cap: Capability) -> Result<(), &'static str> {
    if cap.works() {
        Ok(())
    } else {
        Err(cap.note)
    }
}

/// What the button `button_id` does with every other block. `None` if there is no such button.
pub fn reach(layout: &Layout, button_id: &str, reg: &dyn Registry) -> Option<Reach> {
    let BlockKind::SharingButton(button) = &layout.block(button_id)?.kind else {
        return None;
    };
    let mode = sharing_mode(layout, button_id, reg);
    let info = match mode {
        SharingMode::Send => button
            .connection_id
            .as_deref()
            .and_then(|id| reg.connection(id)),
        SharingMode::SaveLocally => None,
    };
    let capabilities = info.map_or_else(Capabilities::local, |i| i.capabilities);
    let mappings = effective_mappings(layout, button, info.map(|i| i.kind));
    let zip_name = layout
        .blocks
        .iter()
        .find(|b| matches!(&b.kind, BlockKind::Text(t) if t.role == TextRole::ZipName))
        .map(|b| b.id.as_str());

    let blocks = layout
        .blocks
        .iter()
        .filter(|b| !matches!(b.kind, BlockKind::SharingButton(_)))
        .map(|b| BlockReach {
            block_id: b.id.clone(),
            verdict: verdict(
                b,
                button,
                &mappings,
                &capabilities,
                mode == SharingMode::SaveLocally,
                zip_name,
            ),
        })
        .collect();
    Some(Reach {
        capabilities,
        blocks,
    })
}

fn verdict(
    block: &Block,
    button: &SharingButton,
    mappings: &[InfoMapping],
    caps: &Capabilities,
    local: bool,
    zip_name: Option<&str>,
) -> Verdict {
    let sent = |role| Verdict::Sent { role };
    let field_of = || {
        mappings
            .iter()
            .find(|m| matches!(&m.source, ValueSource::Block(id) if *id == block.id))
            .map(|m| m.field.clone())
    };
    match &block.kind {
        BlockKind::Screenshot => {
            if !button.send.screenshot {
                Verdict::SwitchedOff
            } else {
                match usable(caps.screenshot) {
                    Ok(()) => sent(Role::Screenshot),
                    Err(why) => Verdict::CannotSend { why },
                }
            }
        }
        BlockKind::Video => {
            // the button's "send the picture" switch covers the recording too
            if !button.send.screenshot {
                Verdict::SwitchedOff
            } else {
                match usable(caps.video) {
                    Ok(()) => sent(Role::Video),
                    Err(why) => Verdict::CannotSend { why },
                }
            }
        }
        BlockKind::Files(_) => {
            if button.send.files == FilesHow::None {
                Verdict::SwitchedOff
            } else {
                match usable(caps.files) {
                    Ok(()) => sent(Role::Files),
                    Err(why) => Verdict::CannotSend { why },
                }
            }
        }
        BlockKind::Text(t) => {
            let is_prompt = button.prompt_block_id.as_deref() == Some(block.id.as_str());
            let role = if is_prompt {
                Some(Role::Prompt)
            } else if let Some(field) = field_of() {
                Some(Role::Field { field })
            } else if local && t.role == TextRole::ZipName && zip_name == Some(block.id.as_str()) {
                Some(Role::ZipName)
            } else {
                None
            };
            match role {
                None => Verdict::NotUsed,
                Some(Role::ZipName) => sent(Role::ZipName),
                Some(role) => match usable(caps.text) {
                    Ok(()) => sent(role),
                    Err(why) => Verdict::CannotSend { why },
                },
            }
        }
        BlockKind::Dropdown(_) => match field_of() {
            None => Verdict::NotUsed,
            Some(field) => match usable(caps.dropdown) {
                Ok(()) => sent(Role::Field { field }),
                Err(why) => Verdict::CannotSend { why },
            },
        },
        BlockKind::SharingButton(_) => Verdict::NotUsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::CustomShape;
    use crate::model::*;
    use crate::validate::{ConnectionInfo, ConnectionState};
    use std::collections::HashMap;

    #[derive(Default)]
    struct Reg(HashMap<String, ConnectionInfo>);

    impl Reg {
        fn with(mut self, id: &str, kind: ConnectionKind, state: ConnectionState) -> Self {
            self.0.insert(id.into(), ConnectionInfo::new(kind, state));
            self
        }
        fn custom(mut self, id: &str, shape: CustomShape) -> Self {
            self.0.insert(
                id.into(),
                ConnectionInfo {
                    capabilities: Capabilities::custom(shape),
                    ..ConnectionInfo::new(ConnectionKind::CustomApi, ConnectionState::Connected)
                },
            );
            self
        }
    }

    impl Registry for Reg {
        fn connection(&self, id: &str) -> Option<ConnectionInfo> {
            self.0.get(id).copied()
        }
        fn rule_exists(&self, _: &str) -> bool {
            true
        }
        fn is_known_variable(&self, _: &str) -> bool {
            true
        }
    }

    fn block(id: &str, kind: BlockKind) -> Block {
        Block {
            id: id.into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
            kind,
        }
    }

    fn text(id: &str, role: TextRole) -> Block {
        block(
            id,
            BlockKind::Text(TextBlock {
                title: id.into(),
                placement: TitlePlacement::Above,
                role,
                default_text: String::new(),
            }),
        )
    }

    fn files(id: &str) -> Block {
        block(
            id,
            BlockKind::Files(FilesBlock {
                title: id.into(),
                include: vec![],
                preselect: vec![],
                look_in: LookIn::default(),
            }),
        )
    }

    fn chats(id: &str) -> Block {
        block(
            id,
            BlockKind::Dropdown(DropdownBlock {
                title: id.into(),
                source: DropdownSource::ClaudeChats,
            }),
        )
    }

    fn layout(blocks: Vec<Block>, button: SharingButton) -> Layout {
        let mut all = blocks;
        all.push(block("go", BlockKind::SharingButton(button)));
        Layout {
            id: "l".into(),
            name: "L".into(),
            blocks: all,
        }
    }

    fn to(connection: &str) -> SharingButton {
        SharingButton {
            label: "Go".into(),
            connection_id: Some(connection.into()),
            ..SharingButton::default()
        }
    }

    fn sent(role: Role) -> Verdict {
        Verdict::Sent { role }
    }

    #[test]
    fn a_destination_that_takes_everything_gets_every_block_it_is_bound_to() {
        let reg = Reg::default().with("j", ConnectionKind::Jira, ConnectionState::Connected);
        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            mappings: vec![InfoMapping {
                field: "summary".into(),
                source: ValueSource::Block("t".into()),
            }],
            ..to("j")
        };
        let l = layout(
            vec![
                block("shot", BlockKind::Screenshot),
                files("f"),
                text("p", TextRole::Prompt),
                text("t", TextRole::Plain),
                text("loose", TextRole::Plain),
            ],
            button,
        );
        let r = reach(&l, "go", &reg).unwrap();
        assert_eq!(r.of("shot"), Some(&sent(Role::Screenshot)));
        assert_eq!(r.of("f"), Some(&sent(Role::Files)));
        assert_eq!(r.of("p"), Some(&sent(Role::Prompt)));
        assert_eq!(
            r.of("t"),
            Some(&sent(Role::Field {
                field: "summary".into()
            }))
        );
        assert_eq!(r.of("loose"), Some(&Verdict::NotUsed));
        assert_eq!(r.of("go"), None, "a button is not a block it sends");
    }

    #[test]
    fn reddit_cannot_send_the_files_block_and_says_why() {
        let reg = Reg::default().with("r", ConnectionKind::Reddit, ConnectionState::Connected);
        let l = layout(
            vec![block("shot", BlockKind::Screenshot), files("f")],
            to("r"),
        );
        let r = reach(&l, "go", &reg).unwrap();
        assert_eq!(r.of("shot"), Some(&sent(Role::Screenshot)));
        match r.of("f") {
            Some(Verdict::CannotSend { why }) => assert!(why.contains("Reddit"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_video_block_is_sent_where_a_video_can_go_and_says_why_where_it_cannot() {
        let reg = Reg::default()
            .with("s", ConnectionKind::Slack, ConnectionState::Connected)
            .with("r", ConnectionKind::Reddit, ConnectionState::Connected)
            .with("c", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![block("clip", BlockKind::Video)], to("s"));
        let r = reach(&l, "go", &reg).unwrap();
        assert_eq!(r.of("clip"), Some(&sent(Role::Video)));

        for (conn, word) in [("r", "Reddit"), ("c", "Claude")] {
            let l = layout(vec![block("clip", BlockKind::Video)], to(conn));
            match reach(&l, "go", &reg).unwrap().of("clip") {
                Some(Verdict::CannotSend { why }) => assert!(why.contains(word), "{why}"),
                other => panic!("{other:?}"),
            }
        }

        // the button's "send the picture" switch covers the recording as well
        let off = SharingButton {
            send: SendRules {
                screenshot: false,
                ..SendRules::default()
            },
            ..to("s")
        };
        let l = layout(vec![block("clip", BlockKind::Video)], off);
        assert_eq!(
            reach(&l, "go", &reg).unwrap().of("clip"),
            Some(&Verdict::SwitchedOff)
        );
    }

    #[test]
    fn saving_locally_keeps_the_video() {
        let reg = Reg::default();
        let l = layout(
            vec![block("clip", BlockKind::Video)],
            SharingButton::default(),
        );
        assert_eq!(
            reach(&l, "go", &reg).unwrap().of("clip"),
            Some(&sent(Role::Video))
        );
    }

    #[test]
    fn the_buttons_own_rules_can_switch_the_picture_and_the_files_off() {
        let reg = Reg::default().with("j", ConnectionKind::Jira, ConnectionState::Connected);
        let button = SharingButton {
            send: SendRules {
                screenshot: false,
                files: FilesHow::None,
                ..SendRules::default()
            },
            ..to("j")
        };
        let l = layout(
            vec![block("shot", BlockKind::Screenshot), files("f")],
            button,
        );
        let r = reach(&l, "go", &reg).unwrap();
        assert_eq!(r.of("shot"), Some(&Verdict::SwitchedOff));
        assert_eq!(r.of("f"), Some(&Verdict::SwitchedOff));
    }

    #[test]
    fn two_buttons_of_one_layout_reach_different_blocks() {
        let reg = Reg::default()
            .with("j", ConnectionKind::Jira, ConnectionState::Connected)
            .with("r", ConnectionKind::Reddit, ConnectionState::Connected);
        let mut l = layout(vec![files("f")], to("j"));
        l.blocks
            .push(block("go2", BlockKind::SharingButton(to("r"))));
        assert_eq!(
            reach(&l, "go", &reg).unwrap().of("f"),
            Some(&sent(Role::Files))
        );
        assert!(matches!(
            reach(&l, "go2", &reg).unwrap().of("f"),
            Some(Verdict::CannotSend { .. })
        ));
    }

    #[test]
    fn a_custom_api_without_a_text_body_cannot_send_the_prompt() {
        let reg = Reg::default().custom("c", CustomShape::default());
        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            ..to("c")
        };
        let l = layout(vec![text("p", TextRole::Prompt)], button);
        assert!(matches!(
            reach(&l, "go", &reg).unwrap().of("p"),
            Some(Verdict::CannotSend { .. })
        ));
    }

    #[test]
    fn a_button_to_claude_uses_the_chat_dropdown_without_being_told_to() {
        let reg = Reg::default().with("c", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![chats("chat")], to("c"));
        let r = reach(&l, "go", &reg).unwrap();
        assert_eq!(
            r.of("chat"),
            Some(&sent(Role::Field {
                field: "chat".into()
            }))
        );
        let BlockKind::SharingButton(b) = &l.block("go").unwrap().kind else {
            unreachable!()
        };
        let m = effective_mappings(&l, b, Some(ConnectionKind::Claude));
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].source, ValueSource::Block("chat".into()));
        // For another destination the dropdown stays unused.
        assert!(effective_mappings(&l, b, Some(ConnectionKind::Jira)).is_empty());
    }

    #[test]
    fn an_explicit_chat_mapping_is_not_doubled() {
        let button = SharingButton {
            mappings: vec![InfoMapping {
                field: "chat".into(),
                source: ValueSource::Block("other".into()),
            }],
            ..to("c")
        };
        let l = layout(vec![chats("chat"), chats("other")], button.clone());
        let m = effective_mappings(&l, &button, Some(ConnectionKind::Claude));
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].source, ValueSource::Block("other".into()));
    }

    #[test]
    fn with_no_active_connection_everything_is_kept_in_the_zip() {
        let reg = Reg::default();
        let button = SharingButton {
            connection_id: None,
            ..SharingButton::default()
        };
        let l = layout(
            vec![
                block("shot", BlockKind::Screenshot),
                files("f"),
                text("z", TextRole::ZipName),
                text("z2", TextRole::ZipName),
            ],
            button,
        );
        let r = reach(&l, "go", &reg).unwrap();
        assert_eq!(r.of("shot"), Some(&sent(Role::Screenshot)));
        assert_eq!(r.of("f"), Some(&sent(Role::Files)));
        assert_eq!(r.of("z"), Some(&sent(Role::ZipName)));
        assert_eq!(
            r.of("z2"),
            Some(&Verdict::NotUsed),
            "only the first one names the zip"
        );
    }

    #[test]
    fn a_zip_name_is_not_sent_to_a_service() {
        let reg = Reg::default().with("j", ConnectionKind::Jira, ConnectionState::Connected);
        let l = layout(vec![text("z", TextRole::ZipName)], to("j"));
        assert_eq!(
            reach(&l, "go", &reg).unwrap().of("z"),
            Some(&Verdict::NotUsed)
        );
    }

    #[test]
    fn serialises_flat_for_the_interface() {
        let reg = Reg::default().with("r", ConnectionKind::Reddit, ConnectionState::Connected);
        let l = layout(vec![files("f"), text("t", TextRole::Plain)], to("r"));
        let json = serde_json::to_value(reach(&l, "go", &reg).unwrap()).unwrap();
        assert_eq!(json["blocks"][0]["block_id"], "f");
        assert_eq!(json["blocks"][0]["state"], "cannot_send");
        assert_eq!(json["blocks"][1]["state"], "not_used");
        assert_eq!(json["capabilities"]["files"]["support"], "no");
    }

    #[test]
    fn a_field_role_names_the_field_beside_it() {
        let v = Verdict::Sent {
            role: Role::Field {
                field: "priority".into(),
            },
        };
        let json = serde_json::to_value(v).unwrap();
        assert_eq!(json["state"], "sent");
        assert_eq!(json["role"], "field");
        assert_eq!(json["field"], "priority");
    }

    #[test]
    fn an_unknown_button_has_no_reach() {
        let l = layout(vec![], to("x"));
        assert!(reach(&l, "nope", &Reg::default()).is_none());
        assert!(reach(&l, "go", &Reg::default()).is_some());
    }
}
