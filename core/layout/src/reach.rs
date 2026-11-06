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
use std::collections::BTreeSet;

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
    /// The button cannot do its job without this block (the prompt of an AI chat, a field the
    /// service requires): the window keeps its box ticked.
    pub required: bool,
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

    let ai = info.is_some_and(|i| i.kind == ConnectionKind::Claude);
    let connection = info.and(button.connection_id.as_deref());
    let blocks = layout
        .blocks
        .iter()
        .filter(|b| !matches!(b.kind, BlockKind::SharingButton(_)))
        .map(|b| {
            let verdict = verdict(
                b,
                button,
                &mappings,
                &capabilities,
                mode == SharingMode::SaveLocally,
                zip_name,
            );
            let required = match &verdict {
                Verdict::Sent { role: Role::Prompt } => ai,
                Verdict::Sent {
                    role: Role::Field { field },
                } => connection.is_some_and(|c| reg.field_required(c, field)),
                _ => false,
            };
            BlockReach {
                block_id: b.id.clone(),
                verdict,
                required,
            }
        })
        .collect();
    Some(Reach {
        capabilities,
        blocks,
    })
}

/// What one click of a sharing button sends, once the person has ticked or cleared boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The button with its picture, files and prompt as this click has them.
    pub button: SharingButton,
    /// The mappings this click uses (the chat included): those whose block was cleared are gone.
    pub mappings: Vec<InfoMapping>,
    /// The files blocks whose ticked files go.
    pub files_blocks: BTreeSet<String>,
    /// The blocks this click leaves out, so the share can skip what only it knows how to find
    /// (the name of the saved zip).
    pub left_out: BTreeSet<String>,
}

/// Apply the boxes ticked in the window to the button, for this one click.
///
/// `ticked` is every block the person wants this button to send. `None` means "as the layout
/// says", which is what a window without boxes sends. Nothing is trusted: a box for a block the
/// destination cannot take, or that the button has no field for, is refused; so is clearing a
/// block the button cannot work without. A box may switch on the picture or the files the layout
/// switched off, because the destination can take them.
pub fn select(
    layout: &Layout,
    button_id: &str,
    reg: &dyn Registry,
    ticked: Option<&[String]>,
) -> Result<Selection, Vec<String>> {
    let Some(BlockKind::SharingButton(own)) = layout.block(button_id).map(|b| &b.kind) else {
        return Err(vec!["That sharing button is not in this layout".to_string()]);
    };
    let reach = reach(layout, button_id, reg)
        .ok_or_else(|| vec!["That sharing button is not in this layout".to_string()])?;
    let mode = sharing_mode(layout, button_id, reg);
    let kind = match mode {
        SharingMode::Send => own
            .connection_id
            .as_deref()
            .and_then(|id| reg.connection(id))
            .map(|i| i.kind),
        SharingMode::SaveLocally => None,
    };
    let mut button = own.clone();
    let mut mappings = effective_mappings(layout, own, kind);
    let files_of = |blocks: &BTreeSet<String>| -> BTreeSet<String> {
        layout
            .blocks
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Files(_)) && blocks.contains(&b.id))
            .map(|b| b.id.clone())
            .collect()
    };
    let all: BTreeSet<String> = layout.blocks.iter().map(|b| b.id.clone()).collect();
    let Some(ticked) = ticked else {
        return Ok(Selection {
            button,
            mappings,
            files_blocks: files_of(&all),
            left_out: BTreeSet::new(),
        });
    };
    let ticked: BTreeSet<&str> = ticked.iter().map(String::as_str).collect();
    if let Some(unknown) = ticked.iter().find(|id| layout.block(id).is_none()) {
        return Err(vec![format!(
            "\u{201c}{unknown}\u{201d} is not a block of this layout"
        )]);
    }

    let mut reasons = Vec::new();
    let mut files_on = BTreeSet::new();
    let mut left_out = BTreeSet::new();
    let mut has_picture = false;
    let mut picture_on = false;
    for r in &reach.blocks {
        let Some(block) = layout.block(&r.block_id) else {
            continue;
        };
        let name = block_name(block);
        let on = ticked.contains(r.block_id.as_str());
        match (&r.verdict, on) {
            (Verdict::CannotSend { why }, true) => {
                reasons.push(format!("{name} cannot be sent: {why}"));
            }
            (Verdict::NotUsed, true) => {
                reasons.push(format!("This button has no field for {name}"));
            }
            (Verdict::Sent { .. }, false) if r.required => {
                reasons.push(format!("{name} is needed by this button"));
            }
            _ => {}
        }
        // Whether the block goes with this click: what the layout says, changed by the box.
        let goes = match &r.verdict {
            Verdict::Sent { .. } | Verdict::SwitchedOff => on,
            Verdict::CannotSend { .. } | Verdict::NotUsed => false,
        };
        if !goes {
            left_out.insert(r.block_id.clone());
        }
        match &block.kind {
            BlockKind::Screenshot | BlockKind::Video => {
                has_picture = true;
                picture_on |= goes;
            }
            BlockKind::Files(_) => {
                if goes {
                    files_on.insert(r.block_id.clone());
                }
            }
            BlockKind::Text(_) | BlockKind::Dropdown(_) => {
                if !goes {
                    if button.prompt_block_id.as_deref() == Some(r.block_id.as_str()) {
                        button.prompt_block_id = None;
                    }
                    mappings.retain(
                        |m| !matches!(&m.source, ValueSource::Block(id) if *id == r.block_id),
                    );
                }
            }
            BlockKind::SharingButton(_) => {}
        }
    }
    if !reasons.is_empty() {
        return Err(reasons);
    }
    if has_picture {
        button.send.screenshot = picture_on;
    }
    if files_on.is_empty() {
        button.send.files = FilesHow::None;
    } else if button.send.files == FilesHow::None {
        button.send.files = FilesHow::Attach;
    }
    Ok(Selection {
        button,
        mappings,
        files_blocks: files_on,
        left_out,
    })
}

/// A block as a person would call it: its title, or what it is.
fn block_name(b: &Block) -> String {
    let titled = |t: &str| {
        if t.trim().is_empty() {
            b.id.clone()
        } else {
            t.trim().to_string()
        }
    };
    match &b.kind {
        BlockKind::Screenshot => "The screenshot".into(),
        BlockKind::Video => "The video".into(),
        BlockKind::Text(t) => format!("\u{201c}{}\u{201d}", titled(&t.title)),
        BlockKind::Dropdown(d) => format!("\u{201c}{}\u{201d}", titled(&d.title)),
        BlockKind::Files(f) => format!("\u{201c}{}\u{201d}", titled(&f.title)),
        BlockKind::SharingButton(_) => b.id.clone(),
    }
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
    struct Reg(HashMap<String, ConnectionInfo>, Vec<(String, String)>);

    impl Reg {
        /// The service of connection `id` requires `field`.
        fn requiring(mut self, id: &str, field: &str) -> Self {
            self.1.push((id.into(), field.into()));
            self
        }
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
        fn field_required(&self, connection_id: &str, field: &str) -> bool {
            self.1.iter().any(|(c, f)| c == connection_id && f == field)
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

    // ----- required blocks, and what a click sends once boxes are ticked or cleared -----

    fn maps(field: &str, block: &str) -> InfoMapping {
        InfoMapping {
            field: field.into(),
            source: ValueSource::Block(block.into()),
        }
    }

    fn ticked(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn jira() -> Reg {
        Reg::default().with("j", ConnectionKind::Jira, ConnectionState::Connected)
    }

    #[test]
    fn the_prompt_of_an_ai_chat_and_a_field_the_service_requires_are_required_and_nothing_else() {
        let reg = Reg::default()
            .with("c", ConnectionKind::Claude, ConnectionState::Connected)
            .with("j", ConnectionKind::Jira, ConnectionState::Connected)
            .requiring("j", "summary");
        let claude = SharingButton {
            prompt_block_id: Some("p".into()),
            ..to("c")
        };
        let l = layout(
            vec![
                block("shot", BlockKind::Screenshot),
                text("p", TextRole::Prompt),
            ],
            claude,
        );
        let r = reach(&l, "go", &reg).unwrap();
        let req = |id: &str| r.blocks.iter().find(|b| b.block_id == id).unwrap().required;
        assert!(req("p"), "Claude cannot do without the prompt");
        assert!(!req("shot"));

        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            mappings: vec![maps("summary", "t"), maps("priority", "u")],
            ..to("j")
        };
        let l = layout(
            vec![
                text("p", TextRole::Prompt),
                text("t", TextRole::Plain),
                text("u", TextRole::Plain),
            ],
            button,
        );
        let r = reach(&l, "go", &reg).unwrap();
        let req = |id: &str| r.blocks.iter().find(|b| b.block_id == id).unwrap().required;
        assert!(req("t"), "Jira requires a summary");
        assert!(!req("u"));
        assert!(!req("p"), "a prompt is only required by an AI chat");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["blocks"][1]["required"], true);
    }

    #[test]
    fn with_no_boxes_the_click_sends_what_the_layout_says() {
        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            mappings: vec![maps("summary", "t")],
            ..to("j")
        };
        let l = layout(
            vec![
                block("shot", BlockKind::Screenshot),
                files("f"),
                text("p", TextRole::Prompt),
                text("t", TextRole::Plain),
            ],
            button.clone(),
        );
        let s = select(&l, "go", &jira(), None).unwrap();
        assert_eq!(s.button, button);
        assert_eq!(s.mappings, button.mappings);
        assert_eq!(s.files_blocks, ["f".to_string()].into());
        assert!(s.left_out.is_empty());
    }

    #[test]
    fn clearing_a_box_leaves_that_block_out_of_this_click_only() {
        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            mappings: vec![maps("summary", "t"), maps("description", "d")],
            ..to("j")
        };
        let l = layout(
            vec![
                block("shot", BlockKind::Screenshot),
                files("f"),
                text("p", TextRole::Prompt),
                text("t", TextRole::Plain),
                text("d", TextRole::Plain),
            ],
            button.clone(),
        );
        // everything but the screenshot, the files, the prompt and the description
        let s = select(&l, "go", &jira(), Some(&ticked(&["t"]))).unwrap();
        assert!(!s.button.send.screenshot);
        assert_eq!(s.button.send.files, FilesHow::None);
        assert!(s.files_blocks.is_empty());
        assert_eq!(s.button.prompt_block_id, None);
        assert_eq!(s.mappings, vec![maps("summary", "t")]);
        assert_eq!(s.left_out, ["shot", "f", "p", "d"].map(String::from).into());
        // the layout itself is untouched
        assert_eq!(
            l.block("go").map(|b| &b.kind),
            Some(&BlockKind::SharingButton(button))
        );
    }

    #[test]
    fn a_box_can_switch_on_what_the_layout_switched_off_when_the_destination_takes_it() {
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
        let s = select(&l, "go", &jira(), Some(&ticked(&["shot", "f"]))).unwrap();
        assert!(s.button.send.screenshot);
        assert_eq!(s.button.send.files, FilesHow::Attach);
        assert_eq!(s.files_blocks, ["f".to_string()].into());
        // and left alone, they stay off
        let s = select(&l, "go", &jira(), Some(&[])).unwrap();
        assert!(!s.button.send.screenshot);
        assert_eq!(s.button.send.files, FilesHow::None);
    }

    #[test]
    fn only_the_ticked_files_blocks_go() {
        let l = layout(vec![files("a"), files("b")], to("j"));
        let s = select(&l, "go", &jira(), Some(&ticked(&["b"]))).unwrap();
        assert_eq!(s.files_blocks, ["b".to_string()].into());
        assert_eq!(s.left_out, ["a".to_string()].into());
        assert_eq!(s.button.send.files, FilesHow::Attach);
    }

    #[test]
    fn what_the_button_cannot_work_without_cannot_be_cleared() {
        let reg = Reg::default().with("c", ConnectionKind::Claude, ConnectionState::Connected);
        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            ..to("c")
        };
        let l = layout(vec![text("p", TextRole::Prompt)], button);
        let why = select(&l, "go", &reg, Some(&[])).unwrap_err();
        assert_eq!(
            why,
            vec!["\u{201c}p\u{201d} is needed by this button".to_string()]
        );
        assert!(select(&l, "go", &reg, Some(&ticked(&["p"]))).is_ok());
        // a required field, too
        let reg = jira().requiring("j", "summary");
        let button = SharingButton {
            mappings: vec![maps("summary", "t")],
            ..to("j")
        };
        let l = layout(vec![text("t", TextRole::Plain)], button);
        assert!(select(&l, "go", &reg, Some(&[])).is_err());
        assert!(select(&l, "go", &reg, Some(&ticked(&["t"]))).is_ok());
    }

    #[test]
    fn a_box_for_what_cannot_be_sent_is_refused_and_says_why() {
        let reg = Reg::default().with("r", ConnectionKind::Reddit, ConnectionState::Connected);
        let l = layout(vec![files("f"), text("t", TextRole::Plain)], to("r"));
        let why = select(&l, "go", &reg, Some(&ticked(&["f"]))).unwrap_err();
        assert!(
            why[0].starts_with("\u{201c}f\u{201d} cannot be sent"),
            "{why:?}"
        );
        // a text the button has no field for
        let why = select(&l, "go", &reg, Some(&ticked(&["t"]))).unwrap_err();
        assert_eq!(
            why,
            vec!["This button has no field for \u{201c}t\u{201d}".to_string()]
        );
        // a block that is not in the layout at all
        assert!(select(&l, "go", &reg, Some(&ticked(&["nope"]))).is_err());
        // not a sharing button
        assert!(select(&l, "f", &reg, None).is_err());
    }

    #[test]
    fn clearing_the_chat_of_a_claude_button_opens_a_new_chat_even_though_it_is_filled_in_for_you() {
        let reg = Reg::default().with("c", ConnectionKind::Claude, ConnectionState::Connected);
        let button = SharingButton {
            prompt_block_id: Some("p".into()),
            ..to("c")
        };
        let l = layout(vec![text("p", TextRole::Prompt), chats("chat")], button);
        let with = select(&l, "go", &reg, None).unwrap();
        assert_eq!(with.mappings, vec![maps("chat", "chat")]);
        let without = select(&l, "go", &reg, Some(&ticked(&["p"]))).unwrap();
        assert!(without.mappings.is_empty());
        assert_eq!(without.left_out, ["chat".to_string()].into());
    }

    #[test]
    fn a_cleared_zip_name_is_left_out_so_the_zip_gets_its_own_name() {
        let l = layout(vec![text("z", TextRole::ZipName)], SharingButton::default());
        let s = select(&l, "go", &Reg::default(), Some(&[])).unwrap();
        assert_eq!(s.left_out, ["z".to_string()].into());
        let s = select(&l, "go", &Reg::default(), Some(&ticked(&["z"]))).unwrap();
        assert!(s.left_out.is_empty());
    }
}
