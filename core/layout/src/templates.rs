//! Starting points for the layout builder: a ready-made window for each kind of destination,
//! and one for saving a capture on this computer when there is no connection.
//!
//! A template is never a file the person edits. It is built on request, shown in the builder
//! as an unsaved layout, and saving it writes a copy under a name of the person's choosing, so
//! the template itself is always there as it was. It is built for one connection: the
//! dropdowns that fetch lists and the sharing button name it.

use crate::model::{
    Block, BlockKind, Deeper, DropdownBlock, DropdownSource, FilesBlock, InfoMapping, Layout,
    LogViewBlock, OnSuccess, Rect, SharingButton, Tab, TabSide, TabsBlock, TagList, TagsBlock,
    TextBlock, TextRole, TitlePlacement, UpdateMode, ValueSource, VideoBlock, VideoSubject,
};
use crate::validate::ConnectionKind;
use cargoframe_i18n::{msg, t};

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

pub const TEMPLATES: [TemplateInfo; 21] = [
    TemplateInfo {
        key: "local-capture",
        name: msg!("Save a capture locally"),
        description: msg!("The screenshot, a note and the files you tick, saved as a zip in your folder for saved zips. A Logs tab beside the screenshot reads the logs. No account needed."),
        service: None,
    },
    TemplateInfo {
        key: "local-video",
        name: msg!("Save a recording locally"),
        description: msg!("The recording of the window in focus, the screenshot taken from it, a note and the files you tick, saved as a zip in your folder for saved zips. A shortcut that opens it records the window in focus. No account needed."),
        service: None,
    },
    TemplateInfo {
        key: "jira-ticket",
        name: msg!("Jira ticket"),
        description: msg!("Creates a ticket assigned to you (or to whoever you pick), or updates the one you pick in Ticket to update: the summary and the assignee stay, the description goes below the one there and the labels are added. An optional comment goes with either. The screenshot and the ticked files are attached, and a Logs tab beside the screenshot reads the logs."),
        service: Some(ConnectionKind::Jira),
    },
    TemplateInfo {
        key: "jira-video",
        name: msg!("Jira ticket with recording"),
        description: msg!("Like the Jira ticket, with the recording of the window in focus attached; tabs hold the recording (with the time of each moment over it), the screenshot taken from it and the logs, starting on the lines written while it recorded. A shortcut that opens it records the window in focus."),
        service: Some(ConnectionKind::Jira),
    },
    TemplateInfo {
        key: "github-issue",
        name: msg!("GitHub issue"),
        description: msg!("Opens an issue in a repository, or updates the one you pick in Issue to update: the title stays, the description goes below the one there and the labels are added. An optional comment goes with either. The screenshot and the ticked files are shown in it, and a Logs tab beside the screenshot reads the logs."),
        service: Some(ConnectionKind::Github),
    },
    TemplateInfo {
        key: "github-video",
        name: msg!("GitHub issue with recording"),
        description: msg!("Like the GitHub issue, with the recording of the window in focus linked from it beside the screenshot taken from it; tabs hold the recording, the screenshot and the logs. A shortcut that opens it records the window in focus."),
        service: Some(ConnectionKind::Github),
    },
    TemplateInfo {
        key: "cargoframe-report",
        name: msg!("CargoFrame bug report"),
        description: msg!("Debug mode: a GitHub issue about CargoFrame itself, from a capture of one of its windows. The title starts with where it was (window, page, tab, section), the description says what was in focus, and the layout on screen, the data rules, the settings and the end of the log are attached."),
        service: Some(ConnectionKind::Github),
    },
    TemplateInfo {
        key: "google-drive",
        name: msg!("Google Drive upload"),
        description: msg!("Upload the screenshot and a zip of the ticked files to a Drive folder you pick. A Logs tab beside the screenshot reads the logs."),
        service: Some(ConnectionKind::Google),
    },
    TemplateInfo {
        key: "google-drive-video",
        name: msg!("Google Drive upload of a recording"),
        description: msg!("Upload the recording of the window in focus, the screenshot taken from it and a zip of the ticked files to a Drive folder you pick. A shortcut that opens it records the window in focus."),
        service: Some(ConnectionKind::Google),
    },
    TemplateInfo {
        key: "google-sheet",
        name: msg!("Google Sheet row"),
        description: msg!("Add a row to a spreadsheet: the date, two columns you fill, and links to the uploads."),
        service: Some(ConnectionKind::Google),
    },
    TemplateInfo {
        key: "reddit-post",
        name: msg!("Reddit post"),
        description: msg!("Post the screenshot to a subreddit you follow, with a title and text."),
        service: Some(ConnectionKind::Reddit),
    },
    TemplateInfo {
        key: "claude-ask",
        name: msg!("Ask Claude about a capture"),
        description: msg!("Write a prompt, choose a new or existing chat, and hand the screenshot and prompt to Claude. A Logs tab beside the screenshot reads the logs, to copy lines into the prompt."),
        service: Some(ConnectionKind::Claude),
    },
    TemplateInfo {
        key: "slack-message",
        name: msg!("Slack message"),
        description: msg!("Post a message with the screenshot and a zip of the ticked files into a channel the app is in, or reply in a thread. A Logs tab beside the screenshot reads the logs."),
        service: Some(ConnectionKind::Slack),
    },
    TemplateInfo {
        key: "slack-video",
        name: msg!("Slack message with a recording"),
        description: msg!("Post a message with the recording of the window in focus and a zip of the ticked files into a channel the app is in, or reply in a thread; tabs hold the recording (with the time of each moment over it), the screenshot taken from it and the logs, starting on the lines written while it recorded. A shortcut that opens it records the window in focus."),
        service: Some(ConnectionKind::Slack),
    },
    TemplateInfo {
        key: "discord-message",
        name: msg!("Discord message"),
        description: msg!("Post a message with the screenshot and a zip of the ticked files into a channel, or start a post in a forum. Nobody is pinged. A Logs tab beside the screenshot reads the logs."),
        service: Some(ConnectionKind::Discord),
    },
    TemplateInfo {
        key: "discord-video",
        name: msg!("Discord message with a recording"),
        description: msg!("Post a message with the recording of the window in focus and a zip of the ticked files into a channel, or start a post in a forum, within the server's upload limit; tabs hold the recording, the screenshot taken from it and the logs. A shortcut that opens it records the window in focus."),
        service: Some(ConnectionKind::Discord),
    },
    TemplateInfo {
        key: "teams-message",
        name: msg!("Teams message"),
        description: msg!("Post a message with the screenshot inside it into a chat, a channel or a channel thread, as you."),
        service: Some(ConnectionKind::Teams),
    },
    TemplateInfo {
        key: "x-post",
        name: msg!("X post"),
        description: msg!("Post the screenshot to your X account with a short text (280 characters), hashtags and a description of the picture."),
        service: Some(ConnectionKind::X),
    },
    TemplateInfo {
        key: "bluesky-post",
        name: msg!("Bluesky post"),
        description: msg!("Post the screenshot to your Bluesky account with a short text (300 characters), hashtags and a description of the picture."),
        service: Some(ConnectionKind::Bluesky),
    },
    TemplateInfo {
        key: "instagram-post",
        name: msg!("Instagram post"),
        description: msg!("Post the screenshot to your Instagram professional account with a caption and hashtags."),
        service: Some(ConnectionKind::Instagram),
    },
    TemplateInfo {
        key: "custom-post",
        name: msg!("Custom API post"),
        description: msg!("A title and a message with the screenshot and files, sent through the request you described."),
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
        title: t(title).to_string(),
        placement: TitlePlacement::Above,
        role,
        default_text: t(default_text).to_string(),
        formatting: true,
        one_line: false,
    })
}

/// A text whose default is built here (from translated parts) rather than one message.
fn text_of(title: &str, default_text: String) -> BlockKind {
    BlockKind::Text(TextBlock {
        title: t(title).to_string(),
        placement: TitlePlacement::Above,
        role: TextRole::Plain,
        default_text,
        formatting: true,
        one_line: false,
    })
}

/// A text of one line: a title, a link. Services do not format those.
fn line(title: &str, default_text: &str) -> BlockKind {
    BlockKind::Text(TextBlock {
        title: t(title).to_string(),
        placement: TitlePlacement::Above,
        role: TextRole::Plain,
        default_text: t(default_text).to_string(),
        formatting: false,
        one_line: true,
    })
}

/// Hashtags added at the end of a post (X, Bluesky, Instagram).
fn hashtags() -> BlockKind {
    BlockKind::Tags(TagsBlock::new(t("Hashtags"), &[]))
}

fn files(title: &str) -> BlockKind {
    BlockKind::Files(FilesBlock {
        title: t(title).to_string(),
        include: vec!["*.log".to_string(), "*.txt".to_string()],
        preselect: Vec::new(),
        look_in: Default::default(),
    })
}

/// The left of a window with much to fill in: a Tabs block, `h` rows high, holding the capture
/// and the logs, one at a time and each at full size. A recording's window has the recording, the
/// screenshot taken when it ends (or from it), and the logs; a picture's, the picture and the
/// logs. The logs are the files to send and a log viewer to read them, to copy lines from.
fn capture_tabs(h: u32, video: bool, files_title: &str) -> Vec<Block> {
    let whole = rect(0, 0, 7, h);
    let tab = |name: &str, ids: &[&str]| Tab {
        name: t(name).into(),
        blocks: ids.iter().map(|id| id.to_string()).collect(),
        locked: false,
    };
    let mut tabs = Vec::new();
    let mut blocks = Vec::new();
    if video {
        tabs.push(tab(msg!("Recording"), &["clip"]));
        blocks.push(block(
            "clip",
            whole,
            BlockKind::Video(VideoBlock {
                // the window in focus, whose program the capture's values come from
                subject: VideoSubject::FocusedWindow,
                screenshot_at_end: true,
                // the time of each moment, to match the lines of the logs
                clock: true,
                ..Default::default()
            }),
        ));
    }
    tabs.push(tab(msg!("Screenshot"), &["shot"]));
    blocks.push(block("shot", whole, BlockKind::Screenshot));
    tabs.push(tab(msg!("Logs"), &["files", "log"]));
    blocks.push(block("files", rect(0, 0, 7, 3), files(files_title)));
    blocks.push(block(
        "log",
        rect(0, 3, 7, h - 3),
        BlockKind::LogView(LogViewBlock {
            title: t("Log").into(),
            // a recording's window starts on the lines written while it recorded
            during: video,
            ..Default::default()
        }),
    ));
    blocks.insert(
        0,
        block(
            "capture",
            whole,
            BlockKind::Tabs(TabsBlock {
                tabs,
                side: TabSide::Top,
            }),
        ),
    );
    blocks
}

fn web_list(title: &str, connection_id: &str, options_id: &str) -> BlockKind {
    BlockKind::Dropdown(DropdownBlock::new(
        t(title),
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

/// Tags that suggest the connection's list as they are typed.
fn tags(title: &str, connection_id: &str, options_id: &str) -> BlockKind {
    BlockKind::Tags(TagsBlock {
        suggest: Some(TagList {
            connection_id: connection_id.to_string(),
            options_id: options_id.to_string(),
        }),
        ..TagsBlock::new(t(title), &[])
    })
}

fn from_block(field: &str, block_id: &str) -> InfoMapping {
    InfoMapping {
        field: field.to_string(),
        source: ValueSource::Block(block_id.to_string()),
        update: None,
    }
}

/// A list from the connection that may stay empty (an issue to update: none picked makes a new one).
fn optional_list(title: &str, connection_id: &str, list: &str) -> BlockKind {
    match web_list(title, connection_id, list) {
        BlockKind::Dropdown(mut d) => {
            d.optional = true;
            BlockKind::Dropdown(d)
        }
        other => other,
    }
}

/// A mapping that says what it does to an issue that is there.
fn updating(field: &str, block_id: &str, how: UpdateMode) -> InfoMapping {
    InfoMapping {
        update: Some(how),
        ..from_block(field, block_id)
    }
}

/// The sharing button of `button`, with the blocks that must be filled before sharing.
fn requiring(mut b: Block, required: &[&str]) -> Block {
    if let BlockKind::SharingButton(s) = &mut b.kind {
        s.required = required.iter().map(|r| r.to_string()).collect();
    }
    b
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
            label: t(label).to_string(),
            connection_id: connection_id.map(String::from),
            prompt_block_id: prompt_block_id.map(String::from),
            mappings,
            on_success: OnSuccess::CloseWindow,
            send: Default::default(),
            required: Vec::new(),
            react: false,
        }),
    )
}

/// What the person sees of the program in the first text boxes.
const WHERE: &str = "{app.title}";
/// Written with formatting: the templates that use it send it where formatting is shown.
const SEEN_IN: &str =
    msg!("Seen in `{app.exe}` **({project.name})** on *{capture.date}* at *{capture.time}*.");

/// The default text of a report where pictures show inside the text (Jira, GitHub): where it
/// was seen, then a table that shows the recording beside the screenshot, or the screenshot.
fn report(video: bool) -> String {
    let table = if video {
        format!(
            "| {} | {} |\n| --- | --- |\n| {{capture.recording}} | {{capture.screenshot}} |",
            t("Recording"),
            t("Screenshot")
        )
    } else {
        format!(
            "| {} |\n| --- |\n| {{capture.screenshot}} |",
            t("Screenshot")
        )
    };
    format!("{}\n\n{table}", t(SEEN_IN))
}

/// Build the template `key` as a layout, for the connection `connection_id`. Templates made for
/// a service need one (`None` gives no template); the one that saves locally takes none. The
/// layout has no id: the caller chooses it when the person saves a copy.
pub fn build(key: &str, connection_id: Option<&str>) -> Option<Layout> {
    let tpl = info(key)?;
    // A template for a recording is the one for a picture with the picture block swapped for a
    // video block, so the two cannot drift apart.
    let (key, is_video) = match key {
        "local-video" => ("local-capture", true),
        "jira-video" => ("jira-ticket", true),
        "github-video" => ("github-issue", true),
        // the GitHub issue, with its title, description and labels made from what was on screen
        "cargoframe-report" => ("github-issue", false),
        "google-drive-video" => ("google-drive", true),
        "slack-video" => ("slack-message", true),
        "discord-video" => ("discord-message", true),
        other => (other, false),
    };
    let c = match (
        tpl.service,
        connection_id.map(str::trim).filter(|c| !c.is_empty()),
    ) {
        (Some(_), None) => return None,
        (Some(_), Some(c)) => Some(c),
        (None, _) => None,
    };
    let shot = |h| block("shot", rect(0, 0, 7, h), BlockKind::Screenshot);
    let blocks = match (key, c) {
        ("local-capture", _) => [
            capture_tabs(8, is_video, msg!("Files to keep")),
            vec![
                block(
                    "note",
                    rect(7, 0, 5, 7),
                    text(msg!("Note"), TextRole::Plain, SEEN_IN),
                ),
                button(
                    rect(7, 7, 5, 1),
                    msg!("Save locally"),
                    None,
                    None,
                    vec![from_block("note", "note")],
                ),
            ],
        ]
        .concat(),
        ("jira-ticket", Some(c)) => [
            capture_tabs(12, is_video, msg!("Attach files")),
            vec![
                block(
                    "issue",
                    rect(7, 0, 5, 1),
                    optional_list(
                        msg!("Ticket to update (empty: a new one)"),
                        c,
                        "recent_issues",
                    ),
                ),
                block(
                    "project",
                    rect(7, 1, 5, 1),
                    web_list(msg!("Project (for a new ticket)"), c, "projects"),
                ),
                block(
                    "type",
                    rect(7, 2, 5, 1),
                    web_list(msg!("Issue type (for a new ticket)"), c, "issue_types"),
                ),
                block("assignee", rect(7, 3, 5, 1), {
                    // you, unless someone else is picked: a new ticket starts on "Me" each time
                    match web_list(msg!("Assignee (for a new ticket)"), c, "users") {
                        BlockKind::Dropdown(mut d) => {
                            d.remember = false;
                            BlockKind::Dropdown(d)
                        }
                        other => other,
                    }
                }),
                block("summary", rect(7, 4, 5, 1), line(msg!("Summary"), WHERE)),
                block(
                    "description",
                    rect(7, 5, 5, 3),
                    text_of(msg!("Description"), report(is_video)),
                ),
                block(
                    "labels",
                    rect(7, 8, 5, 1),
                    tags(msg!("Labels"), c, "labels"),
                ),
                block(
                    "comment",
                    rect(7, 9, 5, 2),
                    text(msg!("Comment (optional)"), TextRole::Plain, ""),
                ),
                requiring(
                    button(
                        rect(7, 11, 5, 1),
                        msg!("Send to Jira"),
                        Some(c),
                        None,
                        vec![
                            from_block("issue", "issue"),
                            from_block("project", "project"),
                            from_block("issuetype", "type"),
                            // an update keeps who the ticket is assigned to
                            updating("assignee", "assignee", UpdateMode::Skip),
                            updating("summary", "summary", UpdateMode::Skip),
                            updating("description", "description", UpdateMode::Append),
                            updating("labels", "labels", UpdateMode::Add),
                            from_block("comment", "comment"),
                        ],
                    ),
                    &["summary"],
                ),
            ],
        ]
        .concat(),
        ("github-issue", Some(c)) => [
            capture_tabs(9, is_video, msg!("Attach files")),
            vec![
                block(
                    "issue",
                    rect(7, 0, 5, 1),
                    optional_list(
                        msg!("Issue to update (empty: a new one)"),
                        c,
                        "recent_issues",
                    ),
                ),
                block("title", rect(7, 1, 5, 1), line(msg!("Title"), WHERE)),
                block(
                    "labels",
                    rect(7, 2, 5, 1),
                    tags(msg!("Labels"), c, "labels"),
                ),
                block(
                    "description",
                    rect(7, 3, 5, 3),
                    text_of(msg!("Description"), report(is_video)),
                ),
                block(
                    "comment",
                    rect(7, 6, 5, 2),
                    text(msg!("Comment (optional)"), TextRole::Plain, ""),
                ),
                requiring(
                    button(
                        rect(7, 8, 5, 1),
                        msg!("Send to GitHub"),
                        Some(c),
                        None,
                        vec![
                            from_block("issue", "issue"),
                            updating("title", "title", UpdateMode::Skip),
                            updating("labels", "labels", UpdateMode::Add),
                            updating("body", "description", UpdateMode::Append),
                            from_block("comment", "comment"),
                        ],
                    ),
                    &["title"],
                ),
            ],
        ]
        .concat(),
        ("google-drive", Some(c)) => [
            capture_tabs(7, is_video, msg!("Files to upload")),
            vec![
                block(
                    "folder",
                    rect(7, 0, 5, 1),
                    web_list(msg!("Drive folder"), c, "folders"),
                ),
                button(
                    rect(7, 1, 5, 1),
                    msg!("Upload to Drive"),
                    Some(c),
                    None,
                    vec![from_block("folder", "folder")],
                ),
            ],
        ]
        .concat(),
        ("google-sheet", Some(c)) => vec![
            shot(7),
            block(
                "spreadsheet",
                rect(7, 0, 5, 1),
                line(msg!("Spreadsheet (id or link)"), ""),
            ),
            block("sheet", rect(7, 1, 5, 1), line(msg!("Sheet tab"), "Sheet1")),
            block(
                "what",
                rect(7, 2, 5, 3),
                text(msg!("What happened"), TextRole::Plain, WHERE),
            ),
            block(
                "project",
                rect(7, 5, 5, 1),
                line(msg!("Project"), "{project.name}"),
            ),
            button(
                rect(7, 6, 5, 1),
                msg!("Add row"),
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
                web_list(msg!("Subreddit"), c, "subscribed"),
            ),
            block("title", rect(7, 1, 5, 1), line(msg!("Title"), WHERE)),
            block(
                "body",
                rect(7, 2, 5, 5),
                text(msg!("Text"), TextRole::Plain, ""),
            ),
            button(
                rect(7, 7, 5, 1),
                msg!("Post"),
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
                rect(7, 0, 5, 4),
                text(msg!("Post (280 characters)"), TextRole::Plain, WHERE),
            ),
            block("hashtags", rect(7, 4, 5, 1), hashtags()),
            block(
                "alt",
                rect(7, 5, 5, 2),
                text(
                    msg!("Picture description"),
                    TextRole::Plain,
                    msg!("A screenshot of {app.title}"),
                ),
            ),
            button(
                rect(7, 7, 5, 1),
                msg!("Post to X"),
                Some(c),
                None,
                vec![
                    from_block("text", "text"),
                    from_block("hashtags", "hashtags"),
                    from_block("alt", "alt"),
                ],
            ),
        ],
        ("bluesky-post", Some(c)) => vec![
            shot(8),
            block(
                "text",
                rect(7, 0, 5, 4),
                text(msg!("Post (300 characters)"), TextRole::Plain, WHERE),
            ),
            block("hashtags", rect(7, 4, 5, 1), hashtags()),
            block(
                "alt",
                rect(7, 5, 5, 2),
                text(
                    msg!("Picture description"),
                    TextRole::Plain,
                    msg!("A screenshot of {app.title}"),
                ),
            ),
            button(
                rect(7, 7, 5, 1),
                msg!("Post to Bluesky"),
                Some(c),
                None,
                vec![
                    from_block("text", "text"),
                    from_block("hashtags", "hashtags"),
                    from_block("alt", "alt"),
                ],
            ),
        ],
        ("instagram-post", Some(c)) => vec![
            shot(8),
            block(
                "caption",
                rect(7, 0, 5, 6),
                text(msg!("Caption"), TextRole::Plain, WHERE),
            ),
            block("hashtags", rect(7, 6, 5, 1), hashtags()),
            button(
                rect(7, 7, 5, 1),
                msg!("Post to Instagram"),
                Some(c),
                None,
                vec![
                    from_block("caption", "caption"),
                    from_block("hashtags", "hashtags"),
                ],
            ),
        ],
        ("claude-ask", Some(c)) => [
            capture_tabs(9, is_video, msg!("Files to mention")),
            vec![
                block(
                    "chat",
                    rect(7, 0, 5, 1),
                    BlockKind::Dropdown(DropdownBlock::new(t("Chat"), DropdownSource::ClaudeChats)),
                ),
                block(
                    "prompt",
                    rect(7, 1, 5, 7),
                    text(
                        msg!("Prompt"),
                        TextRole::Prompt,
                        msg!("What is wrong in {app.title}? The screenshot shows it."),
                    ),
                ),
                button(
                    rect(7, 8, 5, 1),
                    msg!("Ask Claude"),
                    Some(c),
                    Some("prompt"),
                    vec![from_block("chat", "chat")],
                ),
            ],
        ]
        .concat(),
        ("slack-message", Some(c)) => [
            capture_tabs(10, is_video, msg!("Attach files")),
            vec![
                block(
                    "channel",
                    rect(7, 0, 5, 1),
                    web_list(msg!("Channel"), c, "channels"),
                ),
                block(
                    "message",
                    rect(7, 1, 5, 7),
                    text(msg!("Message"), TextRole::Plain, SEEN_IN),
                ),
                block(
                    "thread",
                    rect(7, 8, 5, 1),
                    line(msg!("Reply in a thread (message link, optional)"), ""),
                ),
                button(
                    rect(7, 9, 5, 1),
                    msg!("Post to Slack"),
                    Some(c),
                    Some("message"),
                    vec![
                        from_block("channel", "channel"),
                        from_block("thread", "thread"),
                    ],
                ),
            ],
        ]
        .concat(),
        ("discord-message", Some(c)) => [
            capture_tabs(10, is_video, msg!("Attach files")),
            // The whole path in one dropdown: the channel, then (when there is one to choose) the
            // thread or forum post inside it. The title narrows the second list.
            vec![
                block(
                    "channel",
                    rect(7, 0, 5, 2),
                    path_list(
                        msg!("Where to post"),
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
                    line(msg!("Title (a forum post's name)"), WHERE),
                ),
                block(
                    "message",
                    rect(7, 3, 5, 6),
                    text(msg!("Message"), TextRole::Plain, SEEN_IN),
                ),
                button(
                    rect(7, 9, 5, 1),
                    msg!("Post to Discord"),
                    Some(c),
                    Some("message"),
                    vec![
                        from_block("channel", "channel"),
                        from_block("title", "title"),
                    ],
                ),
            ],
        ]
        .concat(),
        ("teams-message", Some(c)) => vec![
            shot(8),
            block(
                "target",
                rect(7, 0, 5, 1),
                web_list(msg!("Chat or channel"), c, "targets"),
            ),
            block(
                "title",
                rect(7, 1, 5, 1),
                line(msg!("Headline (optional)"), ""),
            ),
            block(
                "message",
                rect(7, 2, 5, 4),
                text(msg!("Message"), TextRole::Plain, SEEN_IN),
            ),
            block(
                "thread",
                rect(7, 6, 5, 1),
                line(
                    msg!("Reply in a channel thread (message link, optional)"),
                    "",
                ),
            ),
            button(
                rect(7, 7, 5, 1),
                msg!("Post to Teams"),
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
            block("title", rect(7, 0, 5, 1), line(msg!("Title"), WHERE)),
            block(
                "message",
                rect(7, 1, 5, 4),
                text(msg!("Message"), TextRole::Plain, SEEN_IN),
            ),
            block("files", rect(7, 5, 5, 3), files(msg!("Attach files"))),
            button(
                rect(7, 8, 5, 1),
                msg!("Send"),
                Some(c),
                Some("message"),
                vec![from_block("title", "title")],
            ),
        ],
        _ => return None,
    };
    let blocks = if tpl.key == "cargoframe-report" {
        blocks.into_iter().map(debug_report).collect()
    } else {
        blocks
    };
    Some(Layout {
        accepts_input: false,
        columns: crate::GRID_COLUMNS,
        id: String::new(),
        name: t(tpl.name).to_string(),
        blocks,
    })
}

/// The CargoFrame bug report (debug mode): the GitHub issue's title starts with where it was,
/// as CargoFrame's issues are titled; its description says what was on screen and on which
/// computer; its labels start with what it is about.
fn debug_report(mut b: Block) -> Block {
    match (b.id.as_str(), &mut b.kind) {
        ("title", BlockKind::Text(tb)) => tb.default_text = "{cargoframe.where} - ".into(),
        ("description", BlockKind::Text(tb)) => {
            tb.default_text = format!(
                "{}\n\n| {} |\n| --- |\n| {{capture.screenshot}} |\n\n\
                 **CargoFrame {{cargoframe.version}}** · {{system.os}} · {{system.screens}}\n\
                 - {}: {{cargoframe.where}}\n\
                 - {}: {{cargoframe.focus}}\n\
                 - {}: {{cargoframe.layout}} ({{cargoframe.block}})\n\
                 - {}: {{cargoframe.guide}}",
                t(SEEN_IN),
                t("Screenshot"),
                t("Where"),
                t("In focus"),
                t("Layout"),
                t("Guide"),
            )
        }
        ("labels", BlockKind::Tags(tags)) => {
            tags.tags = vec!["bug".into(), "{app.exe}".into(), "{cargoframe.page}".into()];
        }
        _ => {}
    }
    b
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
                "capture.screenshot",
                "capture.recording",
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
            // A picture, or a recording (with the screenshot taken from it when the template
            // has tabs), one sharing button, and the button is named for the connection.
            let count = |video: bool| {
                l.blocks
                    .iter()
                    .filter(|b| match b.kind {
                        BlockKind::Video(_) => video,
                        BlockKind::Screenshot => !video,
                        _ => false,
                    })
                    .count()
            };
            let tabbed = l
                .blocks
                .iter()
                .any(|b| matches!(b.kind, BlockKind::Tabs(_)));
            let (videos, pictures) = (count(true), count(false));
            match (t.key.ends_with("-video"), tabbed) {
                (true, true) => assert_eq!((videos, pictures), (1, 1), "{}", t.key),
                (true, false) => panic!("{}: a recording without its screenshot", t.key),
                (false, _) => assert_eq!((videos, pictures), (0, 1), "{}", t.key),
            }
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
    fn a_template_for_a_recording_records_the_window_in_focus_beside_its_screenshot() {
        for t in TEMPLATES.iter().filter(|t| t.key.ends_with("-video")) {
            let l = build(t.key, Some("c")).unwrap();
            let Some(BlockKind::Video(v)) = l.block("clip").map(|b| &b.kind) else {
                panic!("{}: no recording", t.key)
            };
            assert_eq!(v.subject, VideoSubject::FocusedWindow, "{}", t.key);
            assert!(v.screenshot_at_end, "{}", t.key);
            assert!(l.block("shot").is_some(), "{}: no screenshot", t.key);
            // the connection takes a video
            if let Some(kind) = t.service {
                let caps = crate::capabilities::Capabilities::for_kind(kind, true);
                assert_ne!(
                    caps.video.support,
                    crate::capabilities::Support::No,
                    "{}",
                    t.key
                );
            }
        }
    }

    #[test]
    fn a_report_shows_the_recording_and_the_screenshot_in_a_table_and_says_where_it_was_seen() {
        let default =
            |key: &str, id: &str| match &build(key, Some("c")).unwrap().block(id).unwrap().kind {
                BlockKind::Text(tb) => tb.default_text.clone(),
                _ => panic!("{key}: {id} is not a text"),
            };
        let d = default("jira-video", "description");
        assert!(d.starts_with("Seen in `{app.exe}`"), "{d}");
        assert!(d.ends_with("| Recording | Screenshot |\n| --- | --- |\n| {capture.recording} | {capture.screenshot} |"), "{d}");
        let d = default("github-issue", "description");
        assert!(
            d.ends_with("| Screenshot |\n| --- |\n| {capture.screenshot} |"),
            "{d}"
        );
    }

    #[test]
    fn create_or_update_keeps_the_title_adds_below_and_adds_labels_and_says_what_must_be_filled() {
        for (key, title, body) in [
            ("jira-ticket", "summary", "description"),
            ("jira-video", "summary", "description"),
            ("github-issue", "title", "body"),
            ("github-video", "title", "body"),
        ] {
            let l = build(key, Some("c")).unwrap();
            let BlockKind::SharingButton(b) = &l.block("send").unwrap().kind else {
                panic!("{key}: no button")
            };
            // one button that creates or updates, named for where it sends
            let send = if key.starts_with("jira") {
                "Send to Jira"
            } else {
                "Send to GitHub"
            };
            assert_eq!(b.label, send, "{key}");
            let how = |f: &str| {
                b.mappings
                    .iter()
                    .find(|m| m.field == f)
                    .and_then(|m| m.update)
            };
            assert_eq!(how(title), Some(UpdateMode::Skip), "{key}");
            assert_eq!(how(body), Some(UpdateMode::Append), "{key}");
            assert_eq!(how("labels"), Some(UpdateMode::Add), "{key}");
            assert!(b.mappings.iter().any(|m| m.field == "comment"), "{key}");
            if key.starts_with("jira") {
                // a new ticket is yours unless someone else is picked; an update keeps its assignee
                let Some(BlockKind::Dropdown(a)) = l.block("assignee").map(|b| &b.kind) else {
                    panic!("{key}: no assignee")
                };
                assert!(!a.remember, "{key}: starts on Me each time");
                assert!(
                    matches!(&a.source, DropdownSource::WebApi { options_id, .. } if options_id == "users")
                );
                assert_eq!(how("assignee"), Some(UpdateMode::Skip), "{key}");
            }
            // no issue picked is a new one: the list may stay empty
            let Some(BlockKind::Dropdown(d)) = l.block("issue").map(|b| &b.kind) else {
                panic!("{key}: no issue list")
            };
            assert!(d.optional, "{key}");
            assert!(!b.required.is_empty(), "{key}");
            for r in &b.required {
                assert!(l.block(r).is_some(), "{key}: {r}");
            }
        }
    }

    #[test]
    fn a_window_with_much_to_fill_holds_the_capture_and_the_logs_in_tabs() {
        let names = |l: &Layout| match &l.block("capture").map(|b| &b.kind) {
            Some(BlockKind::Tabs(t)) => t
                .tabs
                .iter()
                .map(|t| (t.name.clone(), t.blocks.clone()))
                .collect::<Vec<_>>(),
            _ => vec![],
        };
        let tab = |n: &str, ids: &[&str]| {
            (
                n.to_string(),
                ids.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            )
        };
        for key in [
            "local-capture",
            "jira-ticket",
            "github-issue",
            "google-drive",
            "slack-message",
            "discord-message",
            "claude-ask",
        ] {
            let l = build(key, Some("c")).unwrap();
            assert_eq!(
                names(&l),
                vec![tab("Screenshot", &["shot"]), tab("Logs", &["files", "log"])],
                "{key}"
            );
            // the picture fills its tab
            assert_eq!(
                l.block("shot").unwrap().rect,
                l.block("capture").unwrap().rect
            );
        }
        for key in [
            "local-video",
            "jira-video",
            "github-video",
            "google-drive-video",
            "slack-video",
            "discord-video",
        ] {
            let l = build(key, Some("c")).unwrap();
            assert_eq!(
                names(&l),
                vec![
                    tab("Recording", &["clip"]),
                    tab("Screenshot", &["shot"]),
                    tab("Logs", &["files", "log"])
                ],
                "{key}"
            );
            // the screenshot tab starts with the picture taken when the recording ends
            assert!(matches!(
                l.block("clip").unwrap().kind,
                BlockKind::Video(VideoBlock {
                    screenshot_at_end: true,
                    ..
                })
            ));
        }
        // a window with little to fill keeps it all in view
        for key in ["x-post", "reddit-post", "teams-message"] {
            assert!(
                build(key, Some("c")).unwrap().block("capture").is_none(),
                "{key}"
            );
        }
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
        assert!(build("jira-ticket", None).is_none());
        assert!(build("jira-ticket", Some("  ")).is_none());
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

    #[test]
    fn the_cargoframe_bug_report_is_the_github_issue_titled_where_it_was() {
        let l = build("cargoframe-report", Some("c")).unwrap();
        let issue = build("github-issue", Some("c")).unwrap();
        // the same blocks, so the two cannot drift apart
        let ids = |l: &Layout| l.blocks.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&l), ids(&issue));
        let text = |id: &str| match &l.blocks.iter().find(|b| b.id == id).unwrap().kind {
            BlockKind::Text(t) => t.default_text.clone(),
            _ => panic!("{id} is not a text"),
        };
        assert_eq!(text("title"), "{cargoframe.where} - ");
        assert!(text("description").contains("{cargoframe.focus}"));
        assert!(text("description").contains("{capture.screenshot}"));
        let BlockKind::Tags(tags) = &l.blocks.iter().find(|b| b.id == "labels").unwrap().kind
        else {
            panic!("no labels");
        };
        assert_eq!(tags.tags, ["bug", "{app.exe}", "{cargoframe.page}"]);
        assert_eq!(l.name, "CargoFrame bug report");
    }
}
