//! Checks behind the builder's status chips.
//!
//! Severity maps to the palette: `Error` red (#E04F4F), `Warning` orange (#E0934F), `Debug`
//! purple (#B04FE0, unresolved or unknown). The interface always shows an icon and a label
//! next to the colour.
//!
//! Rules from the brief:
//! * a sharing button with no active connection (none chosen, one that was deleted, or one that
//!   is not signed in) is a warning, and the button reads "Save locally" and saves a zip;
//! * a sharing button to an AI destination needs a bound text block with the Prompt role,
//!   otherwise it is a warning and disabled;
//! * a dropdown whose connection is disconnected is an error;
//! * a `{{variable}}` nothing defines is shown as unresolved (debug).

use crate::model::{
    Block, BlockKind, DropdownSource, Layout, SharingButton, TextRole, ValueSource,
};
use crate::GRID_COLUMNS;
use cargoframe_context::template;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Unresolved or unknown.
    Debug,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    DuplicateId,
    EmptySize,
    OutOfBounds,
    Overlap,
    NoConnection,
    MissingConnection,
    ConnectionDisconnected,
    ConnectionExpired,
    PromptBlockMissing,
    PromptBlockNotPrompt,
    MappingBlockMissing,
    MappingRuleMissing,
    UnresolvedVariable,
    SeveralZipNames,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub block_id: Option<String>,
    pub severity: Severity,
    pub code: Code,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Jira,
    Google,
    Reddit,
    Claude,
    CustomApi,
}

impl ConnectionKind {
    /// Destinations that need a prompt.
    pub fn is_ai_destination(self) -> bool {
        matches!(self, ConnectionKind::Claude)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    /// Sign-in expired; reconnecting fixes it.
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionInfo {
    pub kind: ConnectionKind,
    pub state: ConnectionState,
}

/// What the builder knows about the rest of the configuration.
pub trait Registry {
    fn connection(&self, id: &str) -> Option<ConnectionInfo>;
    fn rule_exists(&self, id: &str) -> bool;
    /// Is `name` something a `{{name}}` can resolve to (a data rule value or a built-in such
    /// as the capture or project-folder values)?
    fn is_known_variable(&self, name: &str) -> bool;
}

/// What a sharing button does when pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharingMode {
    /// Send to the connection the button names.
    Send,
    /// There is no active connection: save a zip in the folder for saved zips instead.
    SaveLocally,
}

/// Whether a sharing button works right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharingState {
    Ready,
    /// Disabled, with the reasons shown on the chip.
    Disabled(Vec<Code>),
}

fn issue(block: &Block, severity: Severity, code: Code, message: impl Into<String>) -> Issue {
    Issue {
        block_id: Some(block.id.clone()),
        severity,
        code,
        message: message.into(),
    }
}

/// All issues in a layout, most severe first.
pub fn validate(layout: &Layout, reg: &dyn Registry) -> Vec<Issue> {
    let mut out = Vec::new();
    check_geometry(layout, &mut out);
    let mut zip_names = 0;
    for block in &layout.blocks {
        match &block.kind {
            BlockKind::Text(t) => {
                check_variables(block, &t.title, reg, &mut out);
                check_variables(block, &t.default_text, reg, &mut out);
                if t.role == TextRole::ZipName {
                    zip_names += 1;
                    if zip_names > 1 {
                        out.push(issue(
                            block,
                            Severity::Warning,
                            Code::SeveralZipNames,
                            "Only the first block with the Zip name role names the zip: this one is ignored",
                        ));
                    }
                }
            }
            BlockKind::SharingButton(s) => {
                check_variables(block, &s.label, reg, &mut out);
                out.extend(sharing_issues(layout, block, s, reg));
            }
            BlockKind::Dropdown(d) => {
                check_variables(block, &d.title, reg, &mut out);
                if let DropdownSource::WebApi { connection_id, .. } = &d.source {
                    match reg.connection(connection_id) {
                        None => out.push(issue(
                            block,
                            Severity::Error,
                            Code::MissingConnection,
                            format!("Connection \"{connection_id}\" no longer exists"),
                        )),
                        Some(c) if c.state != ConnectionState::Connected => out.push(issue(
                            block,
                            Severity::Error,
                            Code::ConnectionDisconnected,
                            "Connection is not signed in, so the options cannot load",
                        )),
                        Some(_) => {}
                    }
                }
            }
            BlockKind::Files(f) => check_variables(block, &f.title, reg, &mut out),
            BlockKind::Screenshot => {}
        }
    }
    out.sort_by_key(|i| std::cmp::Reverse(i.severity));
    out
}

/// The most severe issue, if any.
pub fn worst(issues: &[Issue]) -> Option<Severity> {
    issues.iter().map(|i| i.severity).max()
}

/// Does the sharing button `block_id` send, or save locally? A connection is active when it
/// exists and is signed in; an expired sign-in still counts, because a failed send keeps a zip
/// and offers to reconnect.
pub fn sharing_mode(layout: &Layout, block_id: &str, reg: &dyn Registry) -> SharingMode {
    let Some(BlockKind::SharingButton(button)) = layout.block(block_id).map(|b| &b.kind) else {
        return SharingMode::Send;
    };
    match button
        .connection_id
        .as_deref()
        .and_then(|id| reg.connection(id))
    {
        Some(c) if c.state != ConnectionState::Disconnected => SharingMode::Send,
        _ => SharingMode::SaveLocally,
    }
}

/// Can the sharing button `block_id` do its job right now? Saving locally needs the folder for
/// saved zips, which the layout cannot know: the caller checks it.
pub fn sharing_state(layout: &Layout, block_id: &str, reg: &dyn Registry) -> SharingState {
    let Some(block) = layout.block(block_id) else {
        return SharingState::Disabled(vec![Code::MissingConnection]);
    };
    let BlockKind::SharingButton(button) = &block.kind else {
        return SharingState::Disabled(vec![Code::MissingConnection]);
    };
    if sharing_mode(layout, block_id, reg) == SharingMode::SaveLocally {
        return SharingState::Ready;
    }
    let blocking: Vec<Code> = sharing_issues(layout, block, button, reg)
        .into_iter()
        .filter(|i| is_blocking(i.code))
        .map(|i| i.code)
        .collect();
    if blocking.is_empty() {
        SharingState::Ready
    } else {
        SharingState::Disabled(blocking)
    }
}

/// An expired sign-in only warns: the button stays usable and a failed send falls back to the
/// safe settings.
fn is_blocking(code: Code) -> bool {
    !matches!(
        code,
        Code::ConnectionExpired | Code::MappingRuleMissing | Code::UnresolvedVariable
    )
}

fn sharing_issues(
    layout: &Layout,
    block: &Block,
    button: &SharingButton,
    reg: &dyn Registry,
) -> Vec<Issue> {
    let mut out = Vec::new();

    let Some(connection_id) = button.connection_id.as_deref() else {
        out.push(issue(
            block,
            Severity::Warning,
            Code::NoConnection,
            "No connection chosen: this button reads \u{201c}Save locally\u{201d} and saves a zip",
        ));
        return out;
    };

    let Some(connection) = reg.connection(connection_id) else {
        out.push(issue(
            block,
            Severity::Warning,
            Code::MissingConnection,
            format!(
                "Connection \"{connection_id}\" no longer exists: this button reads \u{201c}Save locally\u{201d}"
            ),
        ));
        return out;
    };

    match connection.state {
        ConnectionState::Connected => {}
        ConnectionState::Disconnected => {
            out.push(issue(
                block,
                Severity::Warning,
                Code::ConnectionDisconnected,
                "This connection is not signed in: this button reads \u{201c}Save locally\u{201d}",
            ));
            return out;
        }
        ConnectionState::Expired => out.push(issue(
            block,
            Severity::Warning,
            Code::ConnectionExpired,
            "Sign-in expired: reconnect, or a share will be saved locally instead",
        )),
    }

    if connection.kind.is_ai_destination() {
        match button
            .prompt_block_id
            .as_deref()
            .and_then(|id| layout.block(id))
        {
            None => out.push(issue(
                block,
                Severity::Warning,
                Code::PromptBlockMissing,
                "Sharing to an AI tool needs a prompt: add a text block with the Prompt role \
                 and bind it to this button",
            )),
            Some(prompt) => match &prompt.kind {
                BlockKind::Text(t) if t.role == TextRole::Prompt => {}
                _ => out.push(issue(
                    block,
                    Severity::Warning,
                    Code::PromptBlockNotPrompt,
                    "The bound block is not a text block with the Prompt role",
                )),
            },
        }
    }

    for mapping in &button.mappings {
        match &mapping.source {
            ValueSource::Block(id) if layout.block(id).is_none() => out.push(issue(
                block,
                Severity::Error,
                Code::MappingBlockMissing,
                format!(
                    "Field \"{}\" reads from a block that was removed",
                    mapping.field
                ),
            )),
            ValueSource::Rule(id) if !reg.rule_exists(id) => out.push(issue(
                block,
                Severity::Warning,
                Code::MappingRuleMissing,
                format!(
                    "Field \"{}\" reads from a data rule that does not exist",
                    mapping.field
                ),
            )),
            _ => {}
        }
    }
    out
}

fn check_geometry(layout: &Layout, out: &mut Vec<Issue>) {
    for (i, block) in layout.blocks.iter().enumerate() {
        if layout.blocks[..i].iter().any(|b| b.id == block.id) {
            out.push(issue(
                block,
                Severity::Error,
                Code::DuplicateId,
                "Two blocks share an id",
            ));
        }
        if block.rect.w == 0 || block.rect.h == 0 {
            out.push(issue(
                block,
                Severity::Error,
                Code::EmptySize,
                "Block has no size",
            ));
        } else if block.rect.right() > GRID_COLUMNS {
            out.push(issue(
                block,
                Severity::Error,
                Code::OutOfBounds,
                "Block extends past the right edge of the grid",
            ));
        }
        if let Some(other) = layout.blocks[..i]
            .iter()
            .find(|b| b.rect.overlaps(&block.rect))
        {
            out.push(issue(
                block,
                Severity::Error,
                Code::Overlap,
                format!("Overlaps block \"{}\"", other.id),
            ));
        }
    }
}

fn check_variables(block: &Block, text: &str, reg: &dyn Registry, out: &mut Vec<Issue>) {
    for name in template::variables(text) {
        if !reg.is_known_variable(&name) {
            out.push(issue(
                block,
                Severity::Debug,
                Code::UnresolvedVariable,
                format!("{{{{{name}}}}} is not defined by any data rule"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Reg {
        connections: HashMap<String, ConnectionInfo>,
        rules: Vec<String>,
    }

    impl Reg {
        fn with(mut self, id: &str, kind: ConnectionKind, state: ConnectionState) -> Self {
            self.connections
                .insert(id.into(), ConnectionInfo { kind, state });
            self
        }
        fn rule(mut self, id: &str) -> Self {
            self.rules.push(id.into());
            self
        }
    }

    impl Registry for Reg {
        fn connection(&self, id: &str) -> Option<ConnectionInfo> {
            self.connections.get(id).copied()
        }
        fn rule_exists(&self, id: &str) -> bool {
            self.rules.iter().any(|r| r == id)
        }
        fn is_known_variable(&self, name: &str) -> bool {
            self.rules.iter().any(|r| name == format!("rule.{r}"))
        }
    }

    fn at(id: &str, y: u32, kind: BlockKind) -> Block {
        Block {
            id: id.into(),
            rect: Rect::new(0, y, 6, 1),
            kind,
        }
    }

    fn button(connection: Option<&str>, prompt: Option<&str>) -> BlockKind {
        BlockKind::SharingButton(SharingButton {
            label: "Send".into(),
            connection_id: connection.map(String::from),
            prompt_block_id: prompt.map(String::from),
            mappings: vec![],
            on_success: OnSuccess::CloseWindow,
        })
    }

    fn text(role: TextRole, body: &str) -> BlockKind {
        BlockKind::Text(TextBlock {
            title: "T".into(),
            placement: TitlePlacement::Above,
            role,
            default_text: body.into(),
        })
    }

    fn layout(blocks: Vec<Block>) -> Layout {
        Layout {
            id: "l".into(),
            name: "L".into(),
            blocks,
        }
    }

    fn codes(issues: &[Issue]) -> Vec<Code> {
        issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn empty_layout_is_clean() {
        assert!(validate(&Layout::empty("l", "L"), &Reg::default()).is_empty());
    }

    #[test]
    fn button_without_connection_warns_and_saves_locally() {
        let l = layout(vec![at("s", 0, button(None, None))]);
        let issues = validate(&l, &Reg::default());
        assert_eq!(codes(&issues), vec![Code::NoConnection]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert!(issues[0].message.contains("Save locally"));
        assert_eq!(
            sharing_mode(&l, "s", &Reg::default()),
            SharingMode::SaveLocally
        );
        // The layout itself has nothing to block; the folder for saved zips is the caller's check.
        assert_eq!(sharing_state(&l, "s", &Reg::default()), SharingState::Ready);
    }

    #[test]
    fn button_to_a_connected_service_is_ready() {
        let reg = Reg::default().with("jira", ConnectionKind::Jira, ConnectionState::Connected);
        let l = layout(vec![at("s", 0, button(Some("jira"), None))]);
        assert!(validate(&l, &reg).is_empty());
        assert_eq!(sharing_state(&l, "s", &reg), SharingState::Ready);
    }

    #[test]
    fn ai_destination_needs_a_prompt_block() {
        let reg = Reg::default().with("claude", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![at("s", 0, button(Some("claude"), None))]);
        assert_eq!(codes(&validate(&l, &reg)), vec![Code::PromptBlockMissing]);
        assert!(matches!(
            sharing_state(&l, "s", &reg),
            SharingState::Disabled(_)
        ));
    }

    #[test]
    fn plain_text_block_is_not_a_prompt() {
        let reg = Reg::default().with("claude", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![
            at("t", 0, text(TextRole::Plain, "")),
            at("s", 1, button(Some("claude"), Some("t"))),
        ]);
        assert_eq!(codes(&validate(&l, &reg)), vec![Code::PromptBlockNotPrompt]);
    }

    #[test]
    fn bound_prompt_block_enables_the_button() {
        let reg = Reg::default().with("claude", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![
            at("p", 0, text(TextRole::Prompt, "")),
            at("s", 1, button(Some("claude"), Some("p"))),
        ]);
        assert!(validate(&l, &reg).is_empty());
        assert_eq!(sharing_state(&l, "s", &reg), SharingState::Ready);
    }

    #[test]
    fn one_zip_name_block_is_fine_and_a_second_is_ignored_with_a_warning() {
        let reg = Reg::default();
        let one = layout(vec![at("z", 0, text(TextRole::ZipName, ""))]);
        assert!(validate(&one, &reg).is_empty());
        let two = layout(vec![
            at("z1", 0, text(TextRole::ZipName, "")),
            at("z2", 1, text(TextRole::ZipName, "")),
        ]);
        let issues = validate(&two, &reg);
        assert_eq!(codes(&issues), vec![Code::SeveralZipNames]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].block_id.as_deref(), Some("z2"));
    }

    #[test]
    fn a_zip_name_block_is_not_a_prompt() {
        let reg = Reg::default().with("claude", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![
            at("z", 0, text(TextRole::ZipName, "")),
            at("s", 1, button(Some("claude"), Some("z"))),
        ]);
        assert_eq!(codes(&validate(&l, &reg)), vec![Code::PromptBlockNotPrompt]);
    }

    #[test]
    fn removed_prompt_block_disables_the_button_again() {
        let reg = Reg::default().with("claude", ConnectionKind::Claude, ConnectionState::Connected);
        let l = layout(vec![at("s", 1, button(Some("claude"), Some("gone")))]);
        assert_eq!(
            sharing_state(&l, "s", &reg),
            SharingState::Disabled(vec![Code::PromptBlockMissing])
        );
    }

    #[test]
    fn expired_sign_in_warns_but_stays_enabled() {
        let reg = Reg::default().with("jira", ConnectionKind::Jira, ConnectionState::Expired);
        let l = layout(vec![at("s", 0, button(Some("jira"), None))]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::ConnectionExpired]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(sharing_state(&l, "s", &reg), SharingState::Ready);
    }

    #[test]
    fn a_connection_that_is_not_signed_in_is_not_active() {
        let reg = Reg::default().with("jira", ConnectionKind::Jira, ConnectionState::Disconnected);
        let l = layout(vec![at("s", 0, button(Some("jira"), None))]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::ConnectionDisconnected]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(sharing_mode(&l, "s", &reg), SharingMode::SaveLocally);
        assert_eq!(sharing_state(&l, "s", &reg), SharingState::Ready);
    }

    #[test]
    fn an_ai_button_without_a_prompt_still_saves_locally_when_not_signed_in() {
        // The prompt is for the AI tool; a local zip does not need one.
        let reg = Reg::default().with(
            "claude",
            ConnectionKind::Claude,
            ConnectionState::Disconnected,
        );
        let l = layout(vec![at("s", 0, button(Some("claude"), None))]);
        assert_eq!(
            codes(&validate(&l, &reg)),
            vec![Code::ConnectionDisconnected]
        );
        assert_eq!(sharing_state(&l, "s", &reg), SharingState::Ready);
    }

    #[test]
    fn a_deleted_connection_warns_and_saves_locally() {
        let l = layout(vec![at("s", 0, button(Some("old"), None))]);
        let issues = validate(&l, &Reg::default());
        assert_eq!(codes(&issues), vec![Code::MissingConnection]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(
            sharing_mode(&l, "s", &Reg::default()),
            SharingMode::SaveLocally
        );
    }

    #[test]
    fn signed_in_and_expired_connections_send() {
        let reg = Reg::default()
            .with("a", ConnectionKind::Jira, ConnectionState::Connected)
            .with("b", ConnectionKind::Jira, ConnectionState::Expired);
        let l = layout(vec![
            at("s", 0, button(Some("a"), None)),
            at("t", 1, button(Some("b"), None)),
        ]);
        assert_eq!(sharing_mode(&l, "s", &reg), SharingMode::Send);
        assert_eq!(sharing_mode(&l, "t", &reg), SharingMode::Send);
    }

    #[test]
    fn disconnected_dropdown_is_an_error() {
        let reg = Reg::default().with("jira", ConnectionKind::Jira, ConnectionState::Disconnected);
        let l = layout(vec![at(
            "d",
            0,
            BlockKind::Dropdown(DropdownBlock {
                title: "Project".into(),
                source: DropdownSource::WebApi {
                    connection_id: "jira".into(),
                    options_id: "projects".into(),
                },
            }),
        )]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::ConnectionDisconnected]);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn claude_chat_dropdown_needs_no_connection() {
        let l = layout(vec![at(
            "d",
            0,
            BlockKind::Dropdown(DropdownBlock {
                title: "Chat".into(),
                source: DropdownSource::ClaudeChats,
            }),
        )]);
        assert!(validate(&l, &Reg::default()).is_empty());
    }

    #[test]
    fn unresolved_variables_use_the_debug_severity() {
        let reg = Reg::default().rule("build");
        let l = layout(vec![at(
            "t",
            0,
            text(TextRole::Plain, "{{rule.build}} {{rule.nope}}"),
        )]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::UnresolvedVariable]);
        assert_eq!(issues[0].severity, Severity::Debug);
        assert!(issues[0].message.contains("rule.nope"));
    }

    #[test]
    fn mapping_to_a_removed_block_is_an_error() {
        let reg = Reg::default().with("jira", ConnectionKind::Jira, ConnectionState::Connected);
        let l = layout(vec![at(
            "s",
            0,
            BlockKind::SharingButton(SharingButton {
                label: "Send".into(),
                connection_id: Some("jira".into()),
                prompt_block_id: None,
                mappings: vec![
                    InfoMapping {
                        field: "summary".into(),
                        source: ValueSource::Block("gone".into()),
                    },
                    InfoMapping {
                        field: "build".into(),
                        source: ValueSource::Rule("nope".into()),
                    },
                ],
                on_success: OnSuccess::KeepOpen,
            }),
        )]);
        let issues = validate(&l, &reg);
        assert_eq!(
            codes(&issues),
            vec![Code::MappingBlockMissing, Code::MappingRuleMissing]
        );
        // A missing block blocks sending; a missing rule alone does not.
        assert!(
            matches!(sharing_state(&l, "s", &reg), SharingState::Disabled(ref c) if c == &[Code::MappingBlockMissing])
        );
    }

    #[test]
    fn geometry_problems() {
        let mut wide = at("wide", 5, BlockKind::Screenshot);
        wide.rect = Rect::new(8, 5, 6, 1);
        let mut flat = at("flat", 6, BlockKind::Screenshot);
        flat.rect = Rect::new(0, 6, 0, 1);
        let l = layout(vec![
            at("a", 0, BlockKind::Screenshot),
            at("a", 0, BlockKind::Screenshot), // same id and same cells
            wide,
            flat,
        ]);
        let c = codes(&validate(&l, &Reg::default()));
        assert!(c.contains(&Code::DuplicateId));
        assert!(c.contains(&Code::Overlap));
        assert!(c.contains(&Code::OutOfBounds));
        assert!(c.contains(&Code::EmptySize));
    }

    #[test]
    fn issues_are_sorted_most_severe_first() {
        let reg = Reg::default();
        let l = layout(vec![
            at("t", 0, text(TextRole::Plain, "{{x}}")),
            at("s", 1, button(None, None)),
            at("e", 1, BlockKind::Screenshot), // overlaps the button: an error
        ]);
        let issues = validate(&l, &reg);
        let sev: Vec<Severity> = issues.iter().map(|i| i.severity).collect();
        assert_eq!(
            sev,
            vec![Severity::Error, Severity::Warning, Severity::Debug]
        );
        assert_eq!(worst(&issues), Some(Severity::Error));
        assert_eq!(worst(&[]), None);
    }
}
