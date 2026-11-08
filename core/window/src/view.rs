//! The JSON the CargoFrame window receives when it opens.

use crate::files::{FileEntry, PlaceFound, Scan};
use cargoframe_context::template;
use cargoframe_handoff::ChatList;
use cargoframe_layout::{
    reach, sharing_mode, sharing_state, validate, Block, BlockKind, Code, DropdownSource, Layout,
    Reach, Registry, Severity, SharingMode, SharingState,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Why the window has no layout to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutProblem {
    /// The trigger has no layout yet: an empty window with an "Edit layout" hint.
    NotSet,
    /// The trigger names a layout whose file is gone.
    Missing(String),
    /// The layout file could not be read.
    Unreadable(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppView {
    pub title: String,
    pub exe_path: Option<String>,
    pub exe_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectView {
    pub folder: String,
    pub name: String,
    /// Plain-language note on how the folder was found.
    pub how: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImageView {
    pub width: u32,
    pub height: u32,
}

/// The recording a video trigger made, as the window needs to know it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VideoView {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub bytes: u64,
    /// The recording has a sound track (the player shows its volume only then).
    pub audio: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TextState {
    pub value: String,
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DropdownOption {
    pub value: String,
    pub label: String,
    /// Group heading ("Pinned", "Recent"), if any.
    pub group: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DropdownStatus {
    Ready,
    /// The options come from a web service and are fetched by the window.
    Loading,
    /// The connection is missing or signed out.
    NeedsConnection,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DropdownState {
    pub status: DropdownStatus,
    pub options: Vec<DropdownOption>,
    pub selected: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FilesState {
    pub entries: Vec<FileEntry>,
    pub truncated: bool,
    pub problem: Option<String>,
    /// What each folder looked in held. One entry (or none) for the default; the window says
    /// where the files came from when there are several.
    pub places: Vec<PlaceFound>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IssueView {
    pub block_id: Option<String>,
    /// `error`, `warning` or `debug`.
    pub severity: &'static str,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SharingView {
    pub enabled: bool,
    /// Why the button is disabled, in words.
    pub reasons: Vec<String>,
    /// `send`, or `save_locally` when the button has no active connection (it then reads
    /// "Save locally" whatever label the layout gives it).
    pub mode: &'static str,
    /// What pressing the button does, when it is not obvious.
    pub note: Option<String>,
    /// The kind of destination (`jira`, `claude`...) the button sends to, which gives it its
    /// colour. `None` when the button saves a zip on this computer.
    pub kind: Option<&'static str>,
    /// What this button does with each other block of the layout, and what its destination can
    /// take at all.
    pub reach: Option<Reach>,
}

/// Why "Save locally" is disabled. The same words are used when the share is refused.
pub const NEEDS_SAVE_FOLDER: &str =
    "Set a \u{201c}Folder for saved zips\u{201d} in Settings, General, Safety to save locally";

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub session_id: u64,
    pub trigger_id: String,
    pub trigger_name: String,
    /// `None` when there is nothing to draw; see `layout_problem`.
    pub layout: Option<Layout>,
    pub layout_problem: Option<String>,
    /// `window`, `display` or `all_displays`.
    pub subject: &'static str,
    pub notes: Vec<String>,
    pub app: Option<AppView>,
    pub project: Option<ProjectView>,
    pub image: ImageView,
    /// The recording, for a capture made by a video trigger; `None` for a screenshot.
    pub video: Option<VideoView>,
    pub variables: BTreeMap<String, String>,
    pub texts: BTreeMap<String, TextState>,
    pub files: BTreeMap<String, FilesState>,
    pub dropdowns: BTreeMap<String, DropdownState>,
    pub issues: Vec<IssueView>,
    pub sharing: BTreeMap<String, SharingView>,
}

pub struct Inputs<'a> {
    pub session_id: u64,
    pub trigger_id: &'a str,
    pub trigger_name: &'a str,
    pub layout: Result<&'a Layout, LayoutProblem>,
    pub subject: &'static str,
    pub notes: Vec<String>,
    pub app: Option<AppView>,
    pub project: Option<ProjectView>,
    pub image: ImageView,
    pub variables: &'a BTreeMap<String, String>,
    pub chats: &'a ChatList,
    pub registry: &'a dyn Registry,
    /// The "Folder for saved zips" setting. A button with no active connection saves there.
    pub local_folder: Option<&'a str>,
    /// Lists the files of a Files block. Called once per block.
    pub scan: &'a dyn Fn(&cargoframe_layout::FilesBlock) -> Scan,
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Debug => "debug",
    }
}

fn code_name(c: Code) -> String {
    let debug = format!("{c:?}");
    let mut out = String::new();
    for (i, ch) in debug.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn problem_text(p: &LayoutProblem) -> String {
    match p {
        LayoutProblem::NotSet => "This trigger has no layout yet.".into(),
        LayoutProblem::Missing(id) => format!("The layout \"{id}\" no longer exists."),
        LayoutProblem::Unreadable(why) => format!("The layout could not be read: {why}"),
    }
}

fn claude_options(chats: &ChatList) -> Vec<DropdownOption> {
    let mut out: Vec<DropdownOption> = chats
        .ordered()
        .into_iter()
        .map(|e| DropdownOption {
            value: e.id.as_str().to_string(),
            label: e.title.clone(),
            group: Some(if e.pinned { "Pinned" } else { "Recent" }.to_string()),
        })
        .collect();
    out.push(DropdownOption {
        value: "new".into(),
        label: "New chat".into(),
        group: None,
    });
    out
}

fn dropdown_state(block: &Block, inputs: &Inputs<'_>) -> Option<DropdownState> {
    let BlockKind::Dropdown(d) = &block.kind else {
        return None;
    };
    Some(match &d.source {
        DropdownSource::ClaudeChats => {
            let options = claude_options(inputs.chats);
            // Re-use the most recently used chat; the first option otherwise.
            let selected = options.first().map(|o| o.value.clone());
            DropdownState {
                status: DropdownStatus::Ready,
                options,
                selected,
                message: None,
            }
        }
        DropdownSource::WebApi { connection_id, .. } => match inputs
            .registry
            .connection(connection_id)
        {
            Some(c) if c.state == cargoframe_layout::ConnectionState::Connected => DropdownState {
                status: DropdownStatus::Loading,
                options: Vec::new(),
                selected: None,
                message: None,
            },
            _ => DropdownState {
                status: DropdownStatus::NeedsConnection,
                options: Vec::new(),
                selected: None,
                message: Some(format!(
                    "Connect \"{connection_id}\" in Settings to load these options"
                )),
            },
        },
    })
}

/// Layout check results in the shape the web view draws.
pub fn issue_views(issues: &[cargoframe_layout::Issue]) -> Vec<IssueView> {
    issues
        .iter()
        .map(|i| IssueView {
            block_id: i.block_id.clone(),
            severity: severity_name(i.severity),
            code: code_name(i.code),
            message: i.message.clone(),
        })
        .collect()
}

pub fn build(inputs: &Inputs<'_>) -> SessionView {
    let mut view = SessionView {
        session_id: inputs.session_id,
        trigger_id: inputs.trigger_id.to_string(),
        trigger_name: inputs.trigger_name.to_string(),
        layout: None,
        layout_problem: None,
        subject: inputs.subject,
        notes: inputs.notes.clone(),
        app: inputs.app.clone(),
        project: inputs.project.clone(),
        image: inputs.image.clone(),
        video: None,
        variables: inputs.variables.clone(),
        texts: BTreeMap::new(),
        files: BTreeMap::new(),
        dropdowns: BTreeMap::new(),
        issues: Vec::new(),
        sharing: BTreeMap::new(),
    };

    let layout = match &inputs.layout {
        Ok(l) => *l,
        Err(p) => {
            view.layout_problem = Some(problem_text(p));
            return view;
        }
    };
    view.layout = Some(layout.clone());

    let lookup = |name: &str| inputs.variables.get(name).cloned();
    let issues = validate(layout, inputs.registry);
    view.issues = issue_views(&issues);

    for block in &layout.blocks {
        match &block.kind {
            BlockKind::Text(t) => {
                let r = template::render(&t.default_text, lookup);
                view.texts.insert(
                    block.id.clone(),
                    TextState {
                        value: r.text,
                        unresolved: r.unresolved,
                    },
                );
            }
            BlockKind::Files(f) => {
                let scan = (inputs.scan)(f);
                view.files.insert(
                    block.id.clone(),
                    FilesState {
                        entries: scan.entries,
                        truncated: scan.truncated,
                        problem: scan.problem,
                        places: scan.places,
                    },
                );
            }
            BlockKind::Dropdown(_) => {
                if let Some(s) = dropdown_state(block, inputs) {
                    view.dropdowns.insert(block.id.clone(), s);
                }
            }
            BlockKind::SharingButton(button) => {
                let kind = button
                    .connection_id
                    .as_deref()
                    .and_then(|id| inputs.registry.connection(id))
                    .filter(|_| {
                        sharing_mode(layout, &block.id, inputs.registry) == SharingMode::Send
                    })
                    .map(|c| c.kind.key());
                let reach = reach(layout, &block.id, inputs.registry);
                let sv = match sharing_mode(layout, &block.id, inputs.registry) {
                    SharingMode::SaveLocally => {
                        match inputs.local_folder.map(str::trim).filter(|f| !f.is_empty()) {
                            Some(folder) => SharingView {
                                enabled: true,
                                reasons: Vec::new(),
                                mode: "save_locally",
                                note: Some(format!(
                                    "No active connection: a zip is saved in {folder}"
                                )),
                                kind,
                                reach,
                            },
                            None => SharingView {
                                enabled: false,
                                reasons: vec![NEEDS_SAVE_FOLDER.to_string()],
                                mode: "save_locally",
                                note: None,
                                kind,
                                reach,
                            },
                        }
                    }
                    SharingMode::Send => match sharing_state(layout, &block.id, inputs.registry) {
                        SharingState::Ready => SharingView {
                            enabled: true,
                            reasons: Vec::new(),
                            mode: "send",
                            note: None,
                            kind,
                            reach,
                        },
                        SharingState::Disabled(codes) => SharingView {
                            enabled: false,
                            reasons: issues
                                .iter()
                                .filter(|i| {
                                    i.block_id.as_deref() == Some(block.id.as_str())
                                        && codes.contains(&i.code)
                                })
                                .map(|i| i.message.clone())
                                .collect(),
                            mode: "send",
                            note: None,
                            kind,
                            reach,
                        },
                    },
                };
                view.sharing.insert(block.id.clone(), sv);
            }
            BlockKind::Screenshot | BlockKind::Video(_) => {}
        }
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::FileEntry;
    use cargoframe_handoff::ChatId;
    use cargoframe_layout::*;

    struct NoConnections;
    impl Registry for NoConnections {
        fn connection(&self, _: &str) -> Option<ConnectionInfo> {
            None
        }
        fn rule_exists(&self, _: &str) -> bool {
            false
        }
        fn is_known_variable(&self, name: &str) -> bool {
            crate::vars::NAMES.contains(&name)
        }
    }

    fn rect(y: u32) -> Rect {
        Rect {
            x: 0,
            y,
            w: 12,
            h: 1,
        }
    }

    fn layout() -> Layout {
        Layout {
            accepts_input: false,
            id: "l".into(),
            name: "L".into(),
            blocks: vec![
                Block {
                    id: "shot".into(),
                    rect: rect(0),
                    kind: BlockKind::Screenshot,
                },
                Block {
                    id: "note".into(),
                    rect: rect(1),
                    kind: BlockKind::Text(TextBlock {
                        title: "Note".into(),
                        placement: TitlePlacement::Above,
                        role: TextRole::Plain,
                        default_text: "{{app.exe}} in {{project.name}} {{rule.missing}}".into(),
                        formatting: true,
                    }),
                },
                Block {
                    id: "files".into(),
                    rect: rect(2),
                    kind: BlockKind::Files(FilesBlock {
                        title: "Logs".into(),
                        include: vec!["*.log".into()],
                        preselect: vec![],
                        look_in: Default::default(),
                    }),
                },
                Block {
                    id: "chat".into(),
                    rect: rect(3),
                    kind: BlockKind::Dropdown(DropdownBlock::new(
                        "Chat",
                        DropdownSource::ClaudeChats,
                    )),
                },
                Block {
                    id: "jira".into(),
                    rect: rect(4),
                    kind: BlockKind::Dropdown(DropdownBlock::new(
                        "Project",
                        DropdownSource::WebApi {
                            connection_id: "jira".into(),
                            options_id: "projects".into(),
                        },
                    )),
                },
                Block {
                    id: "send".into(),
                    rect: rect(5),
                    kind: BlockKind::SharingButton(SharingButton {
                        label: "Share".into(),
                        connection_id: None,
                        prompt_block_id: None,
                        mappings: vec![],
                        on_success: OnSuccess::CloseWindow,
                        send: Default::default(),
                    }),
                },
            ],
        }
    }

    fn inputs<'a>(
        layout: Result<&'a Layout, LayoutProblem>,
        vars: &'a BTreeMap<String, String>,
        chats: &'a ChatList,
        reg: &'a dyn Registry,
        scan: &'a dyn Fn(&FilesBlock) -> Scan,
    ) -> Inputs<'a> {
        Inputs {
            session_id: 3,
            trigger_id: "bug",
            trigger_name: "Bug report",
            layout,
            subject: "window",
            notes: vec![],
            app: None,
            project: None,
            image: ImageView {
                width: 800,
                height: 600,
            },
            variables: vars,
            chats,
            registry: reg,
            local_folder: None,
            scan,
        }
    }

    #[test]
    fn every_block_kind_gets_its_state() {
        let l = layout();
        let mut vars = BTreeMap::new();
        vars.insert("app.exe".to_string(), "Foo.exe".to_string());
        vars.insert("project.name".to_string(), "Foo".to_string());
        let mut chats = ChatList::new();
        chats.add(
            ChatId::parse("01a168ea-ad83-718c-a341-75a44c6984ec").unwrap(),
            "Release",
            true,
            1,
        );
        chats.add(
            ChatId::parse("11a168ea-ad83-718c-a341-75a44c6984ec").unwrap(),
            "Bugs",
            false,
            2,
        );
        let scan = |_: &FilesBlock| Scan {
            entries: vec![FileEntry {
                path: "a.log".into(),
                size: 3,
                selected: true,
            }],
            ..Scan::default()
        };
        let v = build(&inputs(Ok(&l), &vars, &chats, &NoConnections, &scan));

        // text: variables substituted, unknown ones reported
        let t = &v.texts["note"];
        assert_eq!(t.value, "Foo.exe in Foo {{rule.missing}}");
        assert_eq!(t.unresolved, ["rule.missing"]);
        // files
        assert_eq!(v.files["files"].entries.len(), 1);
        // Claude chats: pinned, recent, then New chat
        let labels: Vec<&str> = v.dropdowns["chat"]
            .options
            .iter()
            .map(|o| o.label.as_str())
            .collect();
        assert_eq!(labels, ["Release", "Bugs", "New chat"]);
        assert_eq!(
            v.dropdowns["chat"].options[0].group.as_deref(),
            Some("Pinned")
        );
        assert_eq!(
            v.dropdowns["chat"].selected.as_deref(),
            Some("01a168ea-ad83-718c-a341-75a44c6984ec")
        );
        // web dropdown without a connection
        assert_eq!(v.dropdowns["jira"].status, DropdownStatus::NeedsConnection);
        assert!(v.dropdowns["jira"]
            .message
            .as_deref()
            .unwrap()
            .contains("jira"));
        // a sharing button without a connection saves locally, and needs the folder for that
        assert_eq!(v.sharing["send"].mode, "save_locally");
        assert!(!v.sharing["send"].enabled);
        assert_eq!(v.sharing["send"].reasons, [NEEDS_SAVE_FOLDER]);
        // issues come most severe first with stable names
        assert!(v
            .issues
            .iter()
            .any(|i| i.code == "unresolved_variable" && i.severity == "debug"));
        assert!(v
            .issues
            .iter()
            .any(|i| i.code == "no_connection" && i.severity == "warning"));
        assert!(v
            .issues
            .iter()
            .any(|i| i.code == "missing_connection" && i.severity == "error"));
        let sev: Vec<&str> = v.issues.iter().map(|i| i.severity).collect();
        let mut sorted = sev.clone();
        sorted.sort_by_key(|s| match *s {
            "error" => 0,
            "warning" => 1,
            _ => 2,
        });
        assert_eq!(sev, sorted);
    }

    #[test]
    fn a_missing_layout_gives_a_hint_instead_of_blocks() {
        let vars = BTreeMap::new();
        let chats = ChatList::new();
        let scan = |_: &FilesBlock| Scan::default();
        for (problem, text) in [
            (LayoutProblem::NotSet, "no layout yet"),
            (LayoutProblem::Missing("x".into()), "no longer exists"),
            (LayoutProblem::Unreadable("bad json".into()), "bad json"),
        ] {
            let v = build(&inputs(Err(problem), &vars, &chats, &NoConnections, &scan));
            assert!(v.layout.is_none());
            assert!(v.layout_problem.unwrap().contains(text));
            assert!(v.texts.is_empty() && v.sharing.is_empty());
        }
    }

    #[test]
    fn the_view_is_plain_json() {
        let l = layout();
        let vars = BTreeMap::new();
        let chats = ChatList::new();
        let scan = |_: &FilesBlock| Scan::default();
        let v = build(&inputs(Ok(&l), &vars, &chats, &NoConnections, &scan));
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["trigger_name"], "Bug report");
        assert_eq!(json["layout"]["blocks"][0]["type"], "screenshot");
        assert_eq!(json["dropdowns"]["jira"]["status"], "needs_connection");
        assert_eq!(json["image"]["width"], 800);
    }

    struct Signed(ConnectionState);
    impl Registry for Signed {
        fn connection(&self, id: &str) -> Option<ConnectionInfo> {
            (id == "jira").then_some(ConnectionInfo::new(ConnectionKind::Jira, self.0))
        }
        fn rule_exists(&self, _: &str) -> bool {
            false
        }
        fn is_known_variable(&self, name: &str) -> bool {
            crate::vars::NAMES.contains(&name)
        }
    }

    fn one_button(connection: Option<&str>) -> Layout {
        Layout {
            accepts_input: false,
            id: "l".into(),
            name: "L".into(),
            blocks: vec![Block {
                id: "send".into(),
                rect: rect(0),
                kind: BlockKind::SharingButton(SharingButton {
                    label: "Create ticket".into(),
                    connection_id: connection.map(String::from),
                    prompt_block_id: None,
                    mappings: vec![],
                    on_success: OnSuccess::KeepOpen,
                    send: Default::default(),
                }),
            }],
        }
    }

    #[test]
    fn with_no_active_connection_the_button_saves_locally_once_a_folder_is_set() {
        let vars = BTreeMap::new();
        let chats = ChatList::new();
        let scan = |_: &FilesBlock| Scan::default();
        let no_id = one_button(None);
        let signed_out = one_button(Some("jira"));
        for (layout, reg) in [
            (&no_id, &Signed(ConnectionState::Connected)),
            (&signed_out, &Signed(ConnectionState::Disconnected)),
        ] {
            let mut i = inputs(Ok(layout), &vars, &chats, reg, &scan);
            let v = build(&i);
            assert_eq!(v.sharing["send"].mode, "save_locally");
            assert!(!v.sharing["send"].enabled, "no folder, no saving");
            assert_eq!(v.sharing["send"].reasons, [NEEDS_SAVE_FOLDER]);
            // A blank setting is no folder either.
            i.local_folder = Some("  ");
            assert!(!build(&i).sharing["send"].enabled);
            i.local_folder = Some("/home/me/zips");
            let v = build(&i);
            assert!(v.sharing["send"].enabled && v.sharing["send"].reasons.is_empty());
            assert!(v.sharing["send"]
                .note
                .as_deref()
                .unwrap()
                .contains("/home/me/zips"));
        }
    }

    #[test]
    fn a_signed_in_connection_sends_whatever_the_folder_setting_says() {
        let vars = BTreeMap::new();
        let chats = ChatList::new();
        let scan = |_: &FilesBlock| Scan::default();
        let l = one_button(Some("jira"));
        for state in [ConnectionState::Connected, ConnectionState::Expired] {
            let reg = Signed(state);
            let v = build(&inputs(Ok(&l), &vars, &chats, &reg, &scan));
            assert_eq!(v.sharing["send"].mode, "send");
            assert!(v.sharing["send"].enabled && v.sharing["send"].note.is_none());
        }
    }

    #[test]
    fn a_button_carries_its_destination_kind_and_what_it_sends() {
        let vars = BTreeMap::new();
        let chats = ChatList::new();
        let scan = |_: &FilesBlock| Scan::default();
        let mut l = one_button(Some("jira"));
        l.blocks.push(Block {
            id: "shot".into(),
            rect: Rect::new(6, 0, 6, 1),
            kind: BlockKind::Screenshot,
        });
        let reg = Signed(ConnectionState::Connected);
        let v = build(&inputs(Ok(&l), &vars, &chats, &reg, &scan));
        let s = &v.sharing["send"];
        assert_eq!(s.kind, Some("jira"));
        let reach = s.reach.as_ref().expect("a button always has a reach");
        assert_eq!(
            reach.of("shot"),
            Some(&cargoframe_layout::Verdict::Sent {
                role: cargoframe_layout::Role::Screenshot
            })
        );
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["sharing"]["send"]["kind"], "jira");
        assert_eq!(
            json["sharing"]["send"]["reach"]["blocks"][0]["state"],
            "sent"
        );
        assert_eq!(
            json["sharing"]["send"]["reach"]["blocks"][0]["role"],
            "screenshot"
        );
    }

    #[test]
    fn a_button_that_saves_locally_has_no_destination_kind_but_still_a_reach() {
        let vars = BTreeMap::new();
        let chats = ChatList::new();
        let scan = |_: &FilesBlock| Scan::default();
        let l = one_button(None);
        let reg = Signed(ConnectionState::Connected);
        let v = build(&inputs(Ok(&l), &vars, &chats, &reg, &scan));
        assert_eq!(v.sharing["send"].kind, None);
        assert!(v.sharing["send"].reach.is_some());
    }

    #[test]
    fn code_names_are_snake_case() {
        assert_eq!(code_name(Code::PromptBlockMissing), "prompt_block_missing");
        assert_eq!(code_name(Code::Overlap), "overlap");
    }
}
