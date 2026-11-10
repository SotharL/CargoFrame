//! Starting points for the layout builder: a ready-made window for each kind of destination,
//! and one for saving a capture on this computer when there is no connection.
//!
//! A template is never a file the person edits. It is built on request, shown in the builder
//! as an unsaved layout, and saving it writes a copy under a name of the person's choosing, so
//! the template itself is always there as it was. It is built for one connection: the
//! dropdowns that fetch lists and the sharing button name it.

use crate::model::{
    Block, BlockKind, Deeper, DropdownBlock, DropdownSource, FilesBlock, InfoMapping, Layout,
    OnSuccess, Rect, SharingButton, TextBlock, TextRole, TitlePlacement, ValueSource,
};
use crate::validate::ConnectionKind;

/// What the person sees in the list of starting points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateInfo {
    /// Stable name used to ask for the template.
    pub key: &'static str,
    pub name: &'static str,
    /// One sentence on what the window is for.
    pub description: &'static str,
    /// The kind of connection it is built for; `None` for the one that saves locally.
    pub service: Option<ConnectionKind>,
}

pub const TEMPLATES: [TemplateInfo; 20] = [
    TemplateInfo {
        key: "local-capture",
        name: "Save a capture locally",
        description: "The screenshot, a note and the files you tick, saved as a zip in your folder for saved zips. No account needed.",
        service: None,
    },
    TemplateInfo {
        key: "local-video",
        name: "Save a recording locally",
        description: "The recording, a note and the files you tick, saved as a zip in your folder for saved zips. A shortcut that opens it records the screen. No account needed.",
        service: None,
    },
    TemplateInfo {
        key: "jira-bug",
        name: "Jira bug report",
        description: "Project, issue type, summary and description, with the screenshot and any files attached to a new issue.",
        service: Some(ConnectionKind::Jira),
    },
    TemplateInfo {
        key: "jira-video",
        name: "Jira bug report with a recording",
        description: "Like the Jira bug report, with the recording of your screen attached to the new issue. A shortcut that opens it records the screen.",
        service: Some(ConnectionKind::Jira),
    },
    TemplateInfo {
        key: "jira-comment",
        name: "Jira comment on an issue",
        description: "Pick one of your recent issues and add a comment with the screenshot and files.",
        service: Some(ConnectionKind::Jira),
    },
    TemplateInfo {
        key: "github-issue",
        name: "GitHub issue",
        description: "Open an issue in a repository with a title, labels and a description; the screenshot and the ticked files are shown in it.",
        service: Some(ConnectionKind::Github),
    },
    TemplateInfo {
        key: "github-comment",
        name: "GitHub comment on an issue",
        description: "Pick an open issue of the repository and add a comment with the screenshot and files.",
        service: Some(ConnectionKind::Github),
    },
    TemplateInfo {
        key: "google-drive",
        name: "Google Drive upload",
        description: "Upload the screenshot and a zip of the ticked files to a Drive folder you pick.",
        service: Some(ConnectionKind::Google),
    },
    TemplateInfo {
        key: "google-drive-video",
        name: "Google Drive upload of a recording",
        description: "Upload the recording and a zip of the ticked files to a Drive folder you pick. A shortcut that opens it records the screen.",
        service: Some(ConnectionKind::Google),
    },
    TemplateInfo {
        key: "google-sheet",
        name: "Google Sheet row",
        description: "Add a row to a spreadsheet: the date, two columns you fill, and links to the uploads.",
        service: Some(ConnectionKind::Google),
    },
    TemplateInfo {
        key: "reddit-post",
        name: "Reddit post",
        description: "Post the screenshot to a subreddit you follow, with a title and text.",
        service: Some(ConnectionKind::Reddit),
    },
    TemplateInfo {
        key: "claude-ask",
        name: "Ask Claude about a capture",
        description: "Write a prompt, choose a new or existing chat, and hand the screenshot and prompt to Claude.",
        service: Some(ConnectionKind::Claude),
    },
    TemplateInfo {
        key: "slack-message",
        name: "Slack message",
        description: "Post a message with the screenshot and a zip of the ticked files into a channel the app is in, or reply in a thread.",
        service: Some(ConnectionKind::Slack),
    },
    TemplateInfo {
        key: "slack-video",
        name: "Slack message with a recording",
        description: "Post a message with the recording of your screen and a zip of the ticked files into a channel the app is in, or reply in a thread. A shortcut that opens it records the screen.",
        service: Some(ConnectionKind::Slack),
    },
    TemplateInfo {
        key: "discord-message",
        name: "Discord message",
        description: "Post a message with the screenshot and a zip of the ticked files into a channel, or start a post in a forum. Nobody is pinged.",
        service: Some(ConnectionKind::Discord),
    },
    TemplateInfo {
        key: "teams-message",
        name: "Teams message",
        description: "Post a message with the screenshot inside it into a chat, a channel or a channel thread, as you.",
        service: Some(ConnectionKind::Teams),
    },
    TemplateInfo {
        key: "x-post",
        name: "X post",
        description: "Post the screenshot to your X account with a short text (280 characters).",
        service: Some(ConnectionKind::X),
    },
    TemplateInfo {
        key: "bluesky-post",
        name: "Bluesky post",
        description: "Post the screenshot to your Bluesky account with a short text (300 characters) and a description of the picture.",
        service: Some(ConnectionKind::Bluesky),
    },
    TemplateInfo {
        key: "instagram-post",
        name: "Instagram post",
        description: "Post the screenshot to your Instagram professional account with a caption.",
        service: Some(ConnectionKind::Instagram),
    },
    TemplateInfo {
        key: "custom-post",
        name: "Custom API post",
        description: "A title and a message with the screenshot and files, sent through the request you described.",
        service: Some(ConnectionKind::CustomApi),
    },
];

pub fn info(key: &str) -> Option<&'static TemplateInfo> {
    TEMPLATES.iter().find(|t| t.key == key)
}

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

fn block(id: &str, r: Rect, kind: BlockKind) -> Block {
    Block {
        id: id.to_string(),
        rect: r,
        kind,
    }
}

fn text(title: &str, role: TextRole, default_text: &str) -> BlockKind {
    BlockKind::Text(TextBlock {
        title: title.to_string(),
        placement: TitlePlacement::Above,
        role,
        default_text: default_text.to_string(),
        formatting: true,
    })
}

fn files(title: &str) -> BlockKind {
    BlockKind::Files(FilesBlock {
        title: title.to_string(),
        include: vec!["*.log".to_string(), "*.txt".to_string()],
        preselect: Vec::new(),
        look_in: Default::default(),
    })
}

fn web_list(title: &str, connection_id: &str, options_id: &str) -> BlockKind {
    BlockKind::Dropdown(DropdownBlock::new(
        title,
        DropdownSource::WebApi {
            connection_id: connection_id.to_string(),
            options_id: options_id.to_string(),
        },
    ))
}

/// One dropdown for a whole path: a list, then what is inside the choice (`inside_list`), which
/// fills `inside_field` by itself. `narrow_by` narrows the second list.
fn path_list(
    title: &str,
    connection_id: &str,
    options_id: &str,
    inside_list: &str,
    inside_field: &str,
    narrow_by: Option<&str>,
) -> BlockKind {
    let BlockKind::Dropdown(mut d) = web_list(title, connection_id, options_id) else {
        unreachable!("web_list makes a dropdown")
    };
    d.narrow_by = narrow_by.map(String::from);
    d.deeper = Some(Deeper {
        list: inside_list.to_string(),
        field: inside_field.to_string(),
    });
    BlockKind::Dropdown(d)
}

fn from_block(field: &str, block_id: &str) -> InfoMapping {
    InfoMapping {
        field: field.to_string(),
        source: ValueSource::Block(block_id.to_string()),
    }
}

fn button(
    r: Rect,
    label: &str,
    connection_id: Option<&str>,
    prompt_block_id: Option<&str>,
    mappings: Vec<InfoMapping>,
) -> Block {
    block(
        "send",
        r,
        BlockKind::SharingButton(SharingButton {
            label: label.to_string(),
            connection_id: connection_id.map(String::from),
            prompt_block_id: prompt_block_id.map(String::from),
            mappings,
            on_success: OnSuccess::CloseWindow,
            send: Default::default(),
        }),
    )
}

/// What the person sees of the program in the first text boxes.
const WHERE: &str = "{{app.title}}";
/// Written with formatting: the templates that use it send it where formatting is shown.
const SEEN_IN: &str =
    "Seen in `{{app.exe}}` **({{project.name}})** on *{{capture.date}}* at *{{capture.time}}*.";

/// Build the template `key` as a layout, for the connection `connection_id`. Templates made for
/// a service need one (`None` gives no template); the one that saves locally takes none. The
/// layout has no id: the caller chooses it when the person saves a copy.
pub fn build(key: &str, connection_id: Option<&str>) -> Option<Layout> {
    let t = info(key)?;
    // A template for a recording is the one for a picture with the picture block swapped for a
    // video block, so the two cannot drift apart.
    let (key, is_video) = match key {
        "local-video" => ("local-capture", true),
        "jira-video" => ("jira-bug", true),
        "google-drive-video" => ("google-drive", true),
        "slack-video" => ("slack-message", true),
        other => (other, false),
    };
    let c = match (
        t.service,
        connection_id.map(str::trim).filter(|c| !c.is_empty()),
    ) {
        (Some(_), None) => return None,
        (Some(_), Some(c)) => Some(c),
        (None, _) => None,
    };
    let shot = |h| block("shot", rect(0, 0, 7, h), BlockKind::Screenshot);
    let blocks = match (key, c) {
        ("local-capture", _) => vec![
            shot(7),
            block(
                "note",
                rect(7, 0, 5, 3),
                text("Note", TextRole::Plain, WHERE),
            ),
            block("files", rect(7, 3, 5, 3), files("Files to keep")),
            button(
                rect(7, 6, 5, 1),
                "Save locally",
                None,
                None,
                vec![from_block("note", "note")],
            ),
        ],
        ("jira-bug", Some(c)) => vec![
            shot(8),
            block(
                "project",
                rect(7, 0, 5, 1),
                web_list("Project", c, "projects"),
            ),
            block(
                "type",
                rect(7, 1, 5, 1),
                web_list("Issue type", c, "issue_types"),
            ),
            block(
                "summary",
                rect(7, 2, 5, 2),
                text("Summary", TextRole::Plain, WHERE),
            ),
            block(
                "description",
                rect(7, 4, 5, 3),
                text("Description", TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(0, 8, 7, 3), files("Attach files")),
            button(
                rect(7, 7, 5, 1),
                "Create issue",
                Some(c),
                None,
                vec![
                    from_block("project", "project"),
                    from_block("issuetype", "type"),
                    from_block("summary", "summary"),
                    from_block("description", "description"),
                ],
            ),
        ],
        ("jira-comment", Some(c)) => vec![
            shot(8),
            block(
                "issue",
                rect(7, 0, 5, 1),
                web_list("Issue", c, "recent_issues"),
            ),
            block(
                "comment",
                rect(7, 1, 5, 4),
                text("Comment", TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(7, 5, 5, 3), files("Attach files")),
            button(
                rect(7, 8, 5, 1),
                "Add comment",
                Some(c),
                None,
                vec![
                    from_block("issue", "issue"),
                    from_block("description", "comment"),
                ],
            ),
        ],
        ("github-issue", Some(c)) => vec![
            shot(8),
            block(
                "title",
                rect(7, 0, 5, 2),
                text("Title", TextRole::Plain, WHERE),
            ),
            block("labels", rect(7, 2, 5, 1), web_list("Label", c, "labels")),
            block(
                "description",
                rect(7, 3, 5, 4),
                text("Description", TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(0, 8, 7, 3), files("Attach files")),
            button(
                rect(7, 7, 5, 1),
                "Open issue",
                Some(c),
                None,
                vec![
                    from_block("title", "title"),
                    from_block("labels", "labels"),
                    from_block("body", "description"),
                ],
            ),
        ],
        ("github-comment", Some(c)) => vec![
            shot(8),
            block(
                "issue",
                rect(7, 0, 5, 1),
                web_list("Issue", c, "recent_issues"),
            ),
            block(
                "comment",
                rect(7, 1, 5, 4),
                text("Comment", TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(7, 5, 5, 3), files("Attach files")),
            button(
                rect(7, 8, 5, 1),
                "Add comment",
                Some(c),
                None,
                vec![from_block("issue", "issue"), from_block("body", "comment")],
            ),
        ],
        ("google-drive", Some(c)) => vec![
            shot(7),
            block(
                "folder",
                rect(7, 0, 5, 1),
                web_list("Drive folder", c, "folders"),
            ),
            block("files", rect(7, 1, 5, 4), files("Files to upload")),
            button(
                rect(7, 5, 5, 1),
                "Upload to Drive",
                Some(c),
                None,
                vec![from_block("folder", "folder")],
            ),
        ],
        ("google-sheet", Some(c)) => vec![
            shot(9),
            block(
                "spreadsheet",
                rect(7, 0, 5, 2),
                text("Spreadsheet (id or link)", TextRole::Plain, ""),
            ),
            block(
                "sheet",
                rect(7, 2, 5, 2),
                text("Sheet tab", TextRole::Plain, "Sheet1"),
            ),
            block(
                "what",
                rect(7, 4, 5, 2),
                text("What happened", TextRole::Plain, WHERE),
            ),
            block(
                "project",
                rect(7, 6, 5, 2),
                text("Project", TextRole::Plain, "{{project.name}}"),
            ),
            button(
                rect(7, 8, 5, 1),
                "Add row",
                Some(c),
                None,
                vec![
                    from_block("spreadsheet", "spreadsheet"),
                    from_block("sheet", "sheet"),
                    from_block("col1", "what"),
                    from_block("col2", "project"),
                ],
            ),
        ],
        ("reddit-post", Some(c)) => vec![
            shot(8),
            block(
                "subreddit",
                rect(7, 0, 5, 1),
                web_list("Subreddit", c, "subscribed"),
            ),
            block(
                "title",
                rect(7, 1, 5, 2),
                text("Title", TextRole::Plain, WHERE),
            ),
            block("body", rect(7, 3, 5, 4), text("Text", TextRole::Plain, "")),
            button(
                rect(7, 7, 5, 1),
                "Post",
                Some(c),
                None,
                vec![
                    from_block("subreddit", "subreddit"),
                    from_block("title", "title"),
                    from_block("text", "body"),
                ],
            ),
        ],
        ("x-post", Some(c)) => vec![
            shot(8),
            block(
                "text",
                rect(7, 0, 5, 5),
                text("Post (280 characters)", TextRole::Plain, WHERE),
            ),
            block(
                "alt",
                rect(7, 5, 5, 2),
                text(
                    "Picture description",
                    TextRole::Plain,
                    "A screenshot of {{app.title}}",
                ),
            ),
            button(
                rect(7, 7, 5, 1),
                "Post to X",
                Some(c),
                None,
                vec![from_block("text", "text"), from_block("alt", "alt")],
            ),
        ],
        ("bluesky-post", Some(c)) => vec![
            shot(8),
            block(
                "text",
                rect(7, 0, 5, 5),
                text("Post (300 characters)", TextRole::Plain, WHERE),
            ),
            block(
                "alt",
                rect(7, 5, 5, 2),
                text(
                    "Picture description",
                    TextRole::Plain,
                    "A screenshot of {{app.title}}",
                ),
            ),
            button(
                rect(7, 7, 5, 1),
                "Post to Bluesky",
                Some(c),
                None,
                vec![from_block("text", "text"), from_block("alt", "alt")],
            ),
        ],
        ("instagram-post", Some(c)) => vec![
            shot(8),
            block(
                "caption",
                rect(7, 0, 5, 7),
                text("Caption", TextRole::Plain, WHERE),
            ),
            button(
                rect(7, 7, 5, 1),
                "Post to Instagram",
                Some(c),
                None,
                vec![from_block("caption", "caption")],
            ),
        ],
        ("claude-ask", Some(c)) => vec![
            shot(8),
            block(
                "chat",
                rect(7, 0, 5, 1),
                BlockKind::Dropdown(DropdownBlock::new("Chat", DropdownSource::ClaudeChats)),
            ),
            block(
                "prompt",
                rect(7, 1, 5, 4),
                text(
                    "Prompt",
                    TextRole::Prompt,
                    "What is wrong in {{app.title}}? The screenshot shows it.",
                ),
            ),
            block("files", rect(7, 5, 5, 3), files("Files to mention")),
            button(
                rect(7, 8, 5, 1),
                "Ask Claude",
                Some(c),
                Some("prompt"),
                vec![from_block("chat", "chat")],
            ),
        ],
        ("slack-message", Some(c)) => vec![
            shot(10),
            block(
                "channel",
                rect(7, 0, 5, 1),
                web_list("Channel", c, "channels"),
            ),
            block(
                "message",
                rect(7, 1, 5, 4),
                text("Message", TextRole::Plain, SEEN_IN),
            ),
            block(
                "thread",
                rect(7, 5, 5, 1),
                text(
                    "Reply in a thread (message link, optional)",
                    TextRole::Plain,
                    "",
                ),
            ),
            block("files", rect(7, 6, 5, 3), files("Attach files")),
            button(
                rect(7, 9, 5, 1),
                "Post to Slack",
                Some(c),
                Some("message"),
                vec![
                    from_block("channel", "channel"),
                    from_block("thread", "thread"),
                ],
            ),
        ],
        ("discord-message", Some(c)) => vec![
            shot(10),
            // The whole path in one dropdown: the channel, then (when there is one to choose) the
            // thread or forum post inside it. The title narrows the second list.
            block(
                "channel",
                rect(7, 0, 5, 2),
                path_list(
                    "Where to post",
                    c,
                    "channels",
                    "threads",
                    "thread",
                    Some("title"),
                ),
            ),
            block(
                "title",
                rect(7, 2, 5, 1),
                text("Title (a forum post's name)", TextRole::Plain, WHERE),
            ),
            block(
                "message",
                rect(7, 3, 5, 3),
                text("Message", TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(7, 6, 5, 3), files("Attach files")),
            button(
                rect(7, 9, 5, 1),
                "Post to Discord",
                Some(c),
                Some("message"),
                vec![
                    from_block("channel", "channel"),
                    from_block("title", "title"),
                ],
            ),
        ],
        ("teams-message", Some(c)) => vec![
            shot(8),
            block(
                "target",
                rect(7, 0, 5, 1),
                web_list("Chat or channel", c, "targets"),
            ),
            block(
                "title",
                rect(7, 1, 5, 1),
                text("Headline (optional)", TextRole::Plain, ""),
            ),
            block(
                "message",
                rect(7, 2, 5, 4),
                text("Message", TextRole::Plain, SEEN_IN),
            ),
            block(
                "thread",
                rect(7, 6, 5, 1),
                text(
                    "Reply in a channel thread (message link, optional)",
                    TextRole::Plain,
                    "",
                ),
            ),
            button(
                rect(7, 7, 5, 1),
                "Post to Teams",
                Some(c),
                Some("message"),
                vec![
                    from_block("target", "target"),
                    from_block("title", "title"),
                    from_block("thread", "thread"),
                ],
            ),
        ],
        ("custom-post", Some(c)) => vec![
            shot(8),
            block(
                "title",
                rect(7, 0, 5, 2),
                text("Title", TextRole::Plain, WHERE),
            ),
            block(
                "message",
                rect(7, 2, 5, 4),
                text("Message", TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(7, 6, 5, 2), files("Attach files")),
            button(
                rect(7, 8, 5, 1),
                "Send",
                Some(c),
                Some("message"),
                vec![from_block("title", "title")],
            ),
        ],
        _ => return None,
    };
    let blocks = if is_video {
        blocks
            .into_iter()
            .map(|mut b| {
                if matches!(b.kind, BlockKind::Screenshot) {
                    b.id = "clip".to_string();
                    b.kind = BlockKind::Video(Default::default());
                }
                b
            })
            .collect()
    } else {
        blocks
    };
    Some(Layout {
        accepts_input: false,
        columns: crate::GRID_COLUMNS,
        id: String::new(),
        name: t.name.to_string(),
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::{
        sharing_mode, sharing_state, validate, Code, ConnectionInfo, ConnectionState, Registry,
        SharingMode, SharingState,
    };
    use std::collections::BTreeSet;

    struct One(ConnectionKind, ConnectionState);
    impl Registry for One {
        fn connection(&self, id: &str) -> Option<ConnectionInfo> {
            (id == "c").then_some(ConnectionInfo {
                // A Custom API carries what its request describes: this one describes everything.
                capabilities: if self.0 == ConnectionKind::CustomApi {
                    crate::capabilities::Capabilities::custom(crate::capabilities::CustomShape {
                        has_body: true,
                        screenshot_field: true,
                        files_field: true,
                        has_lists: true,
                    })
                } else {
                    crate::capabilities::Capabilities::for_kind(self.0, true)
                },
                ..ConnectionInfo::new(self.0, self.1)
            })
        }
        fn rule_exists(&self, _: &str) -> bool {
            false
        }
        fn is_known_variable(&self, name: &str) -> bool {
            [
                "app.exe",
                "app.path",
                "app.title",
                "project.folder",
                "project.name",
                "capture.date",
                "capture.time",
            ]
            .contains(&name)
        }
    }

    #[test]
    fn keys_are_unique_and_every_kind_of_destination_has_a_starting_point() {
        let keys: BTreeSet<_> = TEMPLATES.iter().map(|t| t.key).collect();
        assert_eq!(keys.len(), TEMPLATES.len());
        for kind in [
            ConnectionKind::Jira,
            ConnectionKind::Google,
            ConnectionKind::Reddit,
            ConnectionKind::Claude,
            ConnectionKind::Slack,
            ConnectionKind::Discord,
            ConnectionKind::Teams,
            ConnectionKind::CustomApi,
            ConnectionKind::X,
            ConnectionKind::Bluesky,
            ConnectionKind::Instagram,
            ConnectionKind::Github,
        ] {
            assert!(
                TEMPLATES.iter().any(|t| t.service == Some(kind)),
                "{kind:?} has no template"
            );
        }
        assert!(TEMPLATES.iter().any(|t| t.service.is_none()));
    }

    #[test]
    fn every_service_template_is_clean_for_a_signed_in_connection_and_ready_to_send() {
        for t in TEMPLATES.iter().filter(|t| t.service.is_some()) {
            let kind = t.service.unwrap();
            let reg = One(kind, ConnectionState::Connected);
            let l = build(t.key, Some("c")).unwrap_or_else(|| panic!("{} did not build", t.key));
            assert_eq!(l.name, t.name);
            assert!(!l.blocks.is_empty());
            let issues = validate(&l, &reg);
            assert!(issues.is_empty(), "{}: {issues:?}", t.key);
            assert_eq!(
                sharing_mode(&l, "send", &reg),
                SharingMode::Send,
                "{}",
                t.key
            );
            assert_eq!(
                sharing_state(&l, "send", &reg),
                SharingState::Ready,
                "{}",
                t.key
            );
            // One capture (a picture or a recording), one sharing button, and the button is
            // named for the connection.
            assert_eq!(
                l.blocks
                    .iter()
                    .filter(|b| matches!(b.kind, BlockKind::Screenshot | BlockKind::Video(_)))
                    .count(),
                1,
                "{}",
                t.key
            );
            let BlockKind::SharingButton(b) = &l.block("send").unwrap().kind else {
                panic!("{}: no button", t.key);
            };
            assert_eq!(b.connection_id.as_deref(), Some("c"));
            // Every dropdown that fetches a list asks the same connection.
            for blk in &l.blocks {
                if let BlockKind::Dropdown(DropdownBlock {
                    source: DropdownSource::WebApi { connection_id, .. },
                    ..
                }) = &blk.kind
                {
                    assert_eq!(connection_id, "c", "{}", t.key);
                }
            }
        }
    }

    #[test]
    fn a_template_for_a_recording_has_a_video_block_where_the_picture_would_be() {
        for (video, picture) in [
            ("local-video", "local-capture"),
            ("jira-video", "jira-bug"),
            ("google-drive-video", "google-drive"),
            ("slack-video", "slack-message"),
        ] {
            let v = build(video, Some("c")).unwrap();
            let p = build(picture, Some("c")).unwrap();
            assert_eq!(v.blocks.len(), p.blocks.len(), "{video}");
            assert!(
                !v.blocks
                    .iter()
                    .any(|b| matches!(b.kind, BlockKind::Screenshot)),
                "{video} still has a picture"
            );
            let clip = v
                .block("clip")
                .unwrap_or_else(|| panic!("{video}: no video block"));
            assert!(matches!(clip.kind, BlockKind::Video(_)));
            // same place, same size, as the picture had
            assert_eq!(clip.rect, p.block("shot").unwrap().rect, "{video}");
            // everything else is the same as the picture's template
            for (a, b) in v.blocks.iter().zip(&p.blocks) {
                if a.id != "clip" {
                    assert_eq!(a, b, "{video}");
                }
            }
            assert_eq!(v.name, info(video).unwrap().name);
        }
        // A picture template is not changed by this.
        assert!(build("slack-message", Some("c"))
            .unwrap()
            .block("shot")
            .is_some());
    }

    #[test]
    fn a_connection_that_is_not_signed_in_yet_only_warns() {
        for t in TEMPLATES.iter().filter(|t| t.service.is_some()) {
            let reg = One(t.service.unwrap(), ConnectionState::Disconnected);
            let l = build(t.key, Some("c")).unwrap();
            assert_eq!(sharing_mode(&l, "send", &reg), SharingMode::SaveLocally);
        }
    }

    #[test]
    fn the_local_template_needs_no_connection_and_saves_locally() {
        let l = build("local-capture", None).unwrap();
        let reg = One(ConnectionKind::Jira, ConnectionState::Connected);
        let codes: Vec<Code> = validate(&l, &reg).iter().map(|i| i.code).collect();
        assert_eq!(
            codes,
            [Code::NoConnection],
            "only the reminder that it saves locally"
        );
        assert_eq!(sharing_mode(&l, "send", &reg), SharingMode::SaveLocally);
        // A connection handed to it is ignored: it is not built for one.
        assert_eq!(build("local-capture", Some("c")), Some(l));
    }

    #[test]
    fn a_service_template_needs_a_connection_and_unknown_keys_give_nothing() {
        assert!(build("jira-bug", None).is_none());
        assert!(build("jira-bug", Some("  ")).is_none());
        assert!(build("nope", Some("c")).is_none());
        assert!(info("nope").is_none());
    }

    #[test]
    fn a_template_survives_being_written_as_a_layout_file() {
        for t in TEMPLATES {
            let l = build(t.key, Some("c")).unwrap();
            let json = serde_json::to_string(&l).unwrap();
            assert_eq!(
                serde_json::from_str::<Layout>(&json).unwrap(),
                l,
                "{}",
                t.key
            );
        }
    }
}
