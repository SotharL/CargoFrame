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

use crate::capabilities::Capabilities;
use crate::model::{
    Block, BlockKind, DropdownBlock, DropdownSource, Layout, SharingButton, TextRole, ValueSource,
};
use crate::reach::{reach, Verdict};
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
    BadSubfolder,
    /// A dropdown narrowed by a text block that is not in the layout.
    NarrowByMissing,
    /// Two dropdowns choose where to post on the same connection.
    TwoPaths,
    /// A channel dropdown and a separate thread dropdown, where one dropdown can hold both.
    SeparateThreads,
    /// A block the destination has no place for, so this button leaves it out.
    CannotSend,
    /// More than [`MAX_SHARING_BUTTONS`] sharing buttons: the window has room for five boxes.
    TooManySharingButtons,
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
    Slack,
    Discord,
    Teams,
    CustomApi,
}

impl ConnectionKind {
    /// The name the interface uses for the kind (the same as the connection's `kind`).
    pub fn key(self) -> &'static str {
        match self {
            ConnectionKind::Jira => "jira",
            ConnectionKind::Google => "google",
            ConnectionKind::Reddit => "reddit",
            ConnectionKind::Claude => "claude",
            ConnectionKind::Slack => "slack",
            ConnectionKind::Discord => "discord",
            ConnectionKind::Teams => "teams",
            ConnectionKind::CustomApi => "custom",
        }
    }

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
    /// What this connection can take. The kind's table, unless the connection's own setup (a
    /// Custom API) or the system (Claude on Linux) narrows it.
    pub capabilities: Capabilities,
}

impl ConnectionInfo {
    /// A connection with the table of its kind, as on Windows.
    pub fn new(kind: ConnectionKind, state: ConnectionState) -> Self {
        Self {
            kind,
            state,
            capabilities: Capabilities::for_kind(kind, true),
        }
    }
}

/// What the builder knows about the rest of the configuration.
pub trait Registry {
    fn connection(&self, id: &str) -> Option<ConnectionInfo>;
    fn rule_exists(&self, id: &str) -> bool;
    /// Is `name` something a `{{name}}` can resolve to (a data rule value or a built-in such
    /// as the capture or project-folder values)?
    fn is_known_variable(&self, name: &str) -> bool;
    /// Does the service of connection `connection_id` need a value for `field`? Used to keep the
    /// block that fills it from being cleared in the window. The default knows no such field.
    fn field_required(&self, _connection_id: &str, _field: &str) -> bool {
        false
    }
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

/// Is `block_id` one of the sharing buttons past the [`MAX_SHARING_BUTTONS`] the window shows?
fn beyond_the_limit(layout: &Layout, block_id: &str) -> bool {
    layout
        .blocks
        .iter()
        .filter(|b| matches!(b.kind, BlockKind::SharingButton(_)))
        .position(|b| b.id == block_id)
        .is_some_and(|at| at >= crate::MAX_SHARING_BUTTONS)
}

const TOO_MANY_BUTTONS: &str =
    "A window has room for 5 sharing buttons: this one cannot send. Remove a button";

fn issue(block: &Block, severity: Severity, code: Code, message: impl Into<String>) -> Issue {
    Issue {
        block_id: Some(block.id.clone()),
        severity,
        code,
        message: message.into(),
    }
}

/// All issues in a layout, most severe first.
/// The service lists a dropdown chooses from: its own, and the one it goes on to.
fn lists_of(d: &DropdownBlock) -> Vec<&str> {
    let mut out = Vec::new();
    if let DropdownSource::WebApi { options_id, .. } = &d.source {
        out.push(options_id.as_str());
    }
    out.extend(d.deeper.iter().map(|x| x.list.as_str()));
    out
}

/// One dropdown holds the path to post in. A second one for the same connection that chooses from
/// a list the first one already covers is refused (the later of the two says so); and the old way
/// of a channel dropdown with a thread dropdown beside it is pointed out.
fn check_paths(
    layout: &Layout,
    block: &Block,
    d: &DropdownBlock,
    connection_id: &str,
    reg: &dyn Registry,
    out: &mut Vec<Issue>,
) {
    let mine = lists_of(d);
    let earlier = layout.blocks.iter().take_while(|b| b.id != block.id);
    for other in earlier {
        let BlockKind::Dropdown(o) = &other.kind else {
            continue;
        };
        let DropdownSource::WebApi {
            connection_id: theirs,
            ..
        } = &o.source
        else {
            continue;
        };
        if theirs != connection_id || !lists_of(o).iter().any(|l| mine.contains(l)) {
            continue;
        }
        out.push(issue(
            block,
            Severity::Error,
            Code::TwoPaths,
            format!(
                "\u{201c}{}\u{201d} already chooses from the same list of this connection. One dropdown holds the whole path to post in: remove this one",
                if o.title.trim().is_empty() { &other.id } else { &o.title }
            ),
        ));
        return;
    }
    let is_discord = reg
        .connection(connection_id)
        .is_some_and(|c| c.kind == ConnectionKind::Discord);
    if is_discord && d.deeper.is_none() {
        let is = |b: &Block, list: &str| {
            matches!(&b.kind, BlockKind::Dropdown(x) if matches!(&x.source,
                DropdownSource::WebApi { connection_id: c, options_id } if c == connection_id && options_id == list))
        };
        if is(block, "threads") && layout.blocks.iter().any(|b| is(b, "channels")) {
            out.push(issue(
                block,
                Severity::Warning,
                Code::SeparateThreads,
                "A channel dropdown and a thread dropdown for the same Discord connection: use one \u{201c}Where to post\u{201d} dropdown that goes on to the threads by itself",
            ));
        }
    }
}

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
                if beyond_the_limit(layout, &block.id) {
                    out.push(issue(
                        block,
                        Severity::Error,
                        Code::TooManySharingButtons,
                        TOO_MANY_BUTTONS,
                    ));
                }
                out.extend(sharing_issues(layout, block, s, reg));
            }
            BlockKind::Dropdown(d) => {
                check_variables(block, &d.title, reg, &mut out);
                if let Some(by) = d.narrow_by.as_deref().filter(|b| !b.is_empty()) {
                    let is_text =
                        matches!(layout.block(by).map(|b| &b.kind), Some(BlockKind::Text(_)));
                    if !is_text {
                        out.push(issue(
                            block,
                            Severity::Warning,
                            Code::NarrowByMissing,
                            format!("This list is narrowed by the text block \"{by}\", which is not in this layout: it shows everything"),
                        ));
                    }
                }
                if let DropdownSource::WebApi { connection_id, .. } = &d.source {
                    check_paths(layout, block, d, connection_id, reg, &mut out);
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
            BlockKind::Files(f) => {
                check_variables(block, &f.title, reg, &mut out);
                if f.look_in.clean_subfolder().is_err() {
                    out.push(issue(
                        block,
                        Severity::Warning,
                        Code::BadSubfolder,
                        "The subfolder must be a plain folder name inside each folder, like logs: no drive, no leading slash, no ..",
                    ));
                }
            }
            BlockKind::Screenshot => {}
            BlockKind::Video(_) => {}
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
    if beyond_the_limit(layout, block_id) {
        return SharingState::Disabled(vec![Code::TooManySharingButtons]);
    }
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
        Code::ConnectionExpired
            | Code::MappingRuleMissing
            | Code::UnresolvedVariable
            | Code::CannotSend
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
    out.extend(cannot_send_issues(layout, block, reg));
    out
}

/// One warning for each block this button's destination has no place for, so the builder can say
/// what a button leaves out and why.
fn cannot_send_issues(layout: &Layout, button_block: &Block, reg: &dyn Registry) -> Vec<Issue> {
    let Some(r) = reach(layout, &button_block.id, reg) else {
        return Vec::new();
    };
    r.blocks
        .iter()
        .filter_map(|b| match &b.verdict {
            Verdict::CannotSend { why } => {
                let name = layout
                    .block(&b.block_id)
                    .map(|blk| match &blk.kind {
                        BlockKind::Screenshot => "the screenshot".to_string(),
                        BlockKind::Video(_) => "the video".to_string(),
                        BlockKind::Files(f) if f.title.trim().is_empty() => "the files".to_string(),
                        BlockKind::Files(f) => {
                            format!("the files in \u{201c}{}\u{201d}", f.title.trim())
                        }
                        BlockKind::Text(t) if !t.title.trim().is_empty() => {
                            format!("\u{201c}{}\u{201d}", t.title.trim())
                        }
                        BlockKind::Dropdown(d) if !d.title.trim().is_empty() => {
                            format!("\u{201c}{}\u{201d}", d.title.trim())
                        }
                        _ => format!("block \"{}\"", blk.id),
                    })
                    .unwrap_or_else(|| b.block_id.clone());
                Some(issue(
                    button_block,
                    Severity::Warning,
                    Code::CannotSend,
                    format!("This button does not send {name}: {why}"),
                ))
            }
            _ => None,
        })
        .collect()
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
                .insert(id.into(), ConnectionInfo::new(kind, state));
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
            send: Default::default(),
        })
    }

    fn text(role: TextRole, body: &str) -> BlockKind {
        BlockKind::Text(TextBlock {
            title: "T".into(),
            placement: TitlePlacement::Above,
            role,
            default_text: body.into(),
            formatting: true,
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
    fn a_video_block_is_fine_alone_and_beside_a_screenshot() {
        let alone = layout(vec![at("clip", 0, BlockKind::Video(Default::default()))]);
        assert!(validate(&alone, &Reg::default()).is_empty());

        let both = layout(vec![
            at("shot", 0, BlockKind::Screenshot),
            at("clip", 1, BlockKind::Video(Default::default())),
        ]);
        assert!(validate(&both, &Reg::default()).is_empty());
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
    fn a_window_has_room_for_five_sharing_buttons_and_the_sixth_cannot_send() {
        let reg = Reg::default().with("jira", ConnectionKind::Jira, ConnectionState::Connected);
        let buttons = |n: u32| {
            layout(
                (0..n)
                    .map(|i| at(&format!("s{i}"), i, button(Some("jira"), None)))
                    .collect(),
            )
        };
        let five = buttons(5);
        assert!(validate(&five, &reg).is_empty());
        assert_eq!(sharing_state(&five, "s4", &reg), SharingState::Ready);

        let six = buttons(6);
        let issues = validate(&six, &reg);
        assert_eq!(codes(&issues), vec![Code::TooManySharingButtons]);
        assert_eq!(issues[0].block_id.as_deref(), Some("s5"));
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(sharing_state(&six, "s4", &reg), SharingState::Ready);
        assert_eq!(
            sharing_state(&six, "s5", &reg),
            SharingState::Disabled(vec![Code::TooManySharingButtons])
        );
        // even a button that only saves locally cannot go past the limit
        let local = layout(
            (0..6)
                .map(|i| at(&format!("s{i}"), i, button(None, None)))
                .collect(),
        );
        assert_eq!(
            sharing_state(&local, "s5", &Reg::default()),
            SharingState::Disabled(vec![Code::TooManySharingButtons])
        );
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
    fn a_subfolder_that_could_leave_the_folder_warns() {
        let files = |sub: &str| {
            BlockKind::Files(FilesBlock {
                title: "Logs".into(),
                include: vec!["*.log".into()],
                preselect: vec![],
                look_in: LookIn {
                    folders: vec![LookFolder::Program],
                    subfolder: sub.into(),
                },
            })
        };
        let reg = Reg::default();
        assert!(validate(&layout(vec![at("f", 0, files("logs"))]), &reg).is_empty());
        assert!(validate(&layout(vec![at("f", 0, files(""))]), &reg).is_empty());
        let issues = validate(&layout(vec![at("f", 0, files("../secrets"))]), &reg);
        assert_eq!(codes(&issues), vec![Code::BadSubfolder]);
        assert_eq!(issues[0].severity, Severity::Warning);
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
            BlockKind::Dropdown(DropdownBlock::new(
                "Project",
                DropdownSource::WebApi {
                    connection_id: "jira".into(),
                    options_id: "projects".into(),
                },
            )),
        )]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::ConnectionDisconnected]);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn a_list_narrowed_by_a_missing_text_block_is_a_warning() {
        let reg = Reg::default().with("d", ConnectionKind::Discord, ConnectionState::Connected);
        let list = |narrow: Option<&str>| {
            let mut d = DropdownBlock::new(
                "Thread",
                DropdownSource::WebApi {
                    connection_id: "d".into(),
                    options_id: "threads".into(),
                },
            );
            d.narrow_by = narrow.map(String::from);
            at("thread", 0, BlockKind::Dropdown(d))
        };
        let title = at(
            "title",
            1,
            BlockKind::Text(TextBlock {
                title: "Title".into(),
                placement: TitlePlacement::Above,
                role: TextRole::Plain,
                default_text: String::new(),
                formatting: true,
            }),
        );
        assert!(validate(&layout(vec![list(Some("title")), title.clone()]), &reg).is_empty());
        assert!(validate(&layout(vec![list(None), title]), &reg).is_empty());
        let issues = validate(&layout(vec![list(Some("nope"))]), &reg);
        assert_eq!(codes(&issues), vec![Code::NarrowByMissing]);
        assert_eq!(issues[0].severity, Severity::Warning);
    }

    fn web(id: &str, title: &str, connection: &str, list: &str, y: u32) -> Block {
        at(
            id,
            y,
            BlockKind::Dropdown(DropdownBlock::new(
                title,
                DropdownSource::WebApi {
                    connection_id: connection.into(),
                    options_id: list.into(),
                },
            )),
        )
    }

    fn deeper(mut b: Block, list: &str, field: &str) -> Block {
        if let BlockKind::Dropdown(d) = &mut b.kind {
            d.deeper = Some(crate::model::Deeper {
                list: list.into(),
                field: field.into(),
            });
        }
        b
    }

    #[test]
    fn one_dropdown_holds_the_path_and_a_second_for_the_same_list_is_refused() {
        let reg = Reg::default()
            .with("d", ConnectionKind::Discord, ConnectionState::Connected)
            .with("e", ConnectionKind::Discord, ConnectionState::Connected);
        let path = || {
            deeper(
                web("where", "Where to post", "d", "channels", 0),
                "threads",
                "thread",
            )
        };
        // one path dropdown is fine
        assert!(validate(&layout(vec![path()]), &reg).is_empty());
        // the same list twice, for the same connection: the later one is refused
        let twice = layout(vec![path(), web("again", "Channel", "d", "channels", 1)]);
        let issues = validate(&twice, &reg);
        assert_eq!(codes(&issues), vec![Code::TwoPaths]);
        assert_eq!(issues[0].block_id.as_deref(), Some("again"));
        assert_eq!(issues[0].severity, Severity::Error);
        // a separate dropdown for the list the path already goes on to is refused as well
        let inner = layout(vec![path(), web("t", "Thread", "d", "threads", 1)]);
        assert_eq!(codes(&validate(&inner, &reg)), vec![Code::TwoPaths]);
        // another connection has its own path
        let other = layout(vec![path(), web("x", "Channel", "e", "channels", 1)]);
        assert!(validate(&other, &reg).is_empty());
    }

    #[test]
    fn the_old_channel_and_thread_dropdowns_still_work_but_are_pointed_out() {
        let reg = Reg::default().with("d", ConnectionKind::Discord, ConnectionState::Connected);
        let l = layout(vec![
            web("c", "Channel", "d", "channels", 0),
            web("t", "Thread", "d", "threads", 1),
        ]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::SeparateThreads]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].block_id.as_deref(), Some("t"));
    }

    #[test]
    fn claude_chat_dropdown_needs_no_connection() {
        let l = layout(vec![at(
            "d",
            0,
            BlockKind::Dropdown(DropdownBlock::new("Chat", DropdownSource::ClaudeChats)),
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
                send: Default::default(),
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

    fn files_block() -> BlockKind {
        BlockKind::Files(FilesBlock {
            title: "Logs".into(),
            include: vec![],
            preselect: vec![],
            look_in: LookIn::default(),
        })
    }

    #[test]
    fn a_button_says_which_block_its_destination_cannot_take_and_still_works() {
        let reg = Reg::default().with("r", ConnectionKind::Reddit, ConnectionState::Connected);
        let l = layout(vec![
            at("shot", 0, BlockKind::Screenshot),
            at("files", 1, files_block()),
            at("go", 2, button(Some("r"), None)),
        ]);
        let issues = validate(&l, &reg);
        assert_eq!(codes(&issues), vec![Code::CannotSend]);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].block_id.as_deref(), Some("go"));
        assert!(
            issues[0].message.contains("files in \u{201c}Logs\u{201d}")
                && issues[0].message.contains("Reddit"),
            "{}",
            issues[0].message
        );
        // Only a warning: the rest of the share still goes.
        assert_eq!(sharing_state(&l, "go", &reg), SharingState::Ready);
    }

    #[test]
    fn a_destination_that_takes_the_block_gets_no_such_warning() {
        let reg = Reg::default().with("j", ConnectionKind::Jira, ConnectionState::Connected);
        let l = layout(vec![
            at("files", 0, files_block()),
            at("go", 1, button(Some("j"), None)),
        ]);
        assert!(validate(&l, &reg).is_empty());
    }

    #[test]
    fn switching_the_files_off_on_the_button_removes_nothing_to_warn_about() {
        let reg = Reg::default().with("r", ConnectionKind::Reddit, ConnectionState::Connected);
        let BlockKind::SharingButton(mut b) = button(Some("r"), None) else {
            unreachable!()
        };
        b.send.files = FilesHow::None;
        let l = layout(vec![
            at("files", 0, files_block()),
            at("go", 1, BlockKind::SharingButton(b)),
        ]);
        assert!(validate(&l, &reg).is_empty());
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
