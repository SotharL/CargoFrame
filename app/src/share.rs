//! Sharing: turn the state of the CargoFrame window into a payload, send it through the chosen
//! connection with retries, and when that fails keep everything in a zip (the safe folder, a
//! folder the person picks, or CargoFrame's own data folder) so nothing is lost.
//!
//! Nothing here knows about Tauri: the browser, desktop, folder picker, clock and sleeping are
//! all passed in, so the whole path is tested against a local mock server.
//!
//! What a share trusts, and what it does not. The window sends what the person typed, picked
//! and ticked. The layout (which button, which connection, which fields), the project folder,
//! the file list and the variables all come from the capture held here, never from the window,
//! and a ticked file that the capture did not offer stops the share.

use crate::connections::Connections;
use crate::registry::AppRegistry;
use crate::session::Session;
use cargoframe_bundle::{
    build_zip, copy_atomic, BundleError, Created, Item, Limits, ManifestInfo, Source, Workspace,
};
use cargoframe_config::infotype::{act, holds};
use cargoframe_config::{InfoKind, InfoOptions, InfoType, Kind, Safe, Store};
use cargoframe_connectors::{
    claude::{MAX_FILES, MAX_FILE_BYTES},
    Attachment, Browser, Connection, Deliver, Field, FieldKind, InlineFile, LocalCopies, Payload,
    Progress, Settings,
};
use cargoframe_context::template;
use cargoframe_layout::{
    select, sharing_mode, sharing_state, validate, BlockKind, ConnectionKind, DropdownBlock,
    DropdownSource, FilesHow, Layout, OnSuccess, SendRules, SharingButton, SharingMode,
    SharingState, TextRole, ValueSource,
};
use cargoframe_safe::ladder::{file_name_for, run_ladder, FallbackIo, LadderConfig, Saved};
use cargoframe_safe::ladder::{PickedFolder, WriteFailure};
use cargoframe_safe::naming::custom_zip_file_name;
use cargoframe_safe::retry::{run_with_retry, RetryPolicy, RetryResult};
use cargoframe_safe::SendError;
use cargoframe_window::vars::Moment;
use cargoframe_window::view::{SessionView, NEEDS_SAVE_FOLDER};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What the window sends when a sharing button is pressed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Request {
    pub session_id: u64,
    pub button_id: String,
    /// What the text blocks hold now, by block id.
    pub texts: BTreeMap<String, String>,
    /// The chosen option of each dropdown (its value), by block id.
    pub dropdowns: BTreeMap<String, Option<String>>,
    /// The tags of each Tags block, by block id. Checked again here: at most 25, cleaned.
    #[serde(default)]
    pub tags: BTreeMap<String, Vec<String>>,
    /// For a dropdown that goes deeper (a channel, then a thread in it): the second choice, by
    /// block id. Empty or missing is "nothing inside it".
    pub deeper: BTreeMap<String, Option<String>>,
    /// The ticked files of each files block, relative to the project folder.
    pub files: BTreeMap<String, Vec<String>>,
    /// The picture was edited: send the edited one the window uploaded.
    pub image_edited: bool,
    /// For a video capture whose layout has a screenshot block: that block holds a picture (taken
    /// from the recording, and edited or not, the window uploaded it).
    pub still: bool,
    /// For a video capture: what was done to the recording (cut, cropped). `None` sends it as it is.
    #[serde(default)]
    pub clip_edit: Option<crate::video::ClipEdit>,
    /// The blocks whose boxes were ticked for this button when it was pressed. `None` sends what
    /// the layout says; a list sends exactly those blocks (see `cargoframe_layout::select`).
    #[serde(default)]
    pub send_blocks: Option<Vec<String>>,
    /// The window found nowhere to post (a list the service answered with nothing, such as a
    /// bot in no server): keep a zip here instead of sending, as with no connection.
    pub save_here: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SavedCopy {
    /// `safe_folder`, `picked_folder`, `kept_locally` or `not_saved`.
    pub place: &'static str,
    pub path: String,
    /// Why the first choices did not work, when it matters.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ShareResult {
    Sent {
        summary: String,
        url: Option<String>,
        notes: Vec<String>,
        /// The layout asks to close the window after a successful share.
        close: bool,
        /// A place the share made or used that the service cannot list (a Discord forum post made
        /// through a webhook): the window offers it next time.
        place: Option<cargoframe_connectors::OptionItem>,
    },
    /// There was no active connection: a zip was saved in the folder for saved zips.
    SavedLocally {
        path: String,
        notes: Vec<String>,
        /// The layout asks to close the window after a successful share.
        close: bool,
    },
    /// Nothing was sent; the person can fix these and press the button again.
    Blocked { reasons: Vec<String> },
    /// The service could not be reached or refused. A copy was kept (see `saved`).
    Failed {
        label: String,
        message: String,
        /// Offer "Reconnect": the sign-in is no longer accepted.
        reconnect: bool,
        attempts: u32,
        saved: Option<SavedCopy>,
    },
}

fn blocked(reason: impl Into<String>) -> ShareResult {
    ShareResult::Blocked {
        reasons: vec![reason.into()],
    }
}

/// Everything a share needs from outside.
pub struct Services<'a> {
    pub store: &'a Store,
    pub connections: &'a Connections,
    pub browser: &'a dyn Browser,
    /// Pastes into another program's window (a Claude chat).
    pub deliver: &'a dyn Deliver,
    /// Where this capture's temporary copies go.
    pub workspace: &'a Workspace,
    /// True once the person gave up (closed the window): no more retries.
    pub cancelled: &'a dyn Fn() -> bool,
    /// Local time, for file names.
    pub now: Moment,
}

/// A file larger than this is not copied ahead of time (it is also the most a Claude message
/// takes for one file).
pub const MAX_STAGED_FILE_BYTES: u64 = MAX_FILE_BYTES;
/// Copies made ahead of time stop here: the same total a zip may hold.
pub const MAX_STAGED_TOTAL_BYTES: u64 = 200 * 1024 * 1024;

fn created(m: &Moment) -> Created {
    Created {
        year: m.year.clamp(1980, 2107) as u16,
        month: m.month,
        day: m.day,
        hour: m.hour,
        minute: m.minute,
        second: m.second,
    }
}

/// A sleep that notices the person giving up.
pub fn interruptible_sleep<'a>(cancelled: &'a dyn Fn() -> bool) -> impl FnMut(Duration) + 'a {
    move |total| {
        let step = Duration::from_millis(100);
        let mut left = total;
        while !left.is_zero() && !cancelled() {
            let now = left.min(step);
            std::thread::sleep(now);
            left -= now;
        }
    }
}

/// The connection a share is sent through.
struct Destination {
    id: String,
    connection: Connection,
}

/// Everything worked out before anything is sent.
struct Prepared {
    /// Where the share goes. `None`: there is no active connection, so it is saved locally.
    destination: Option<Destination>,
    service: String,
    trigger_name: String,
    payload: Payload,
    /// The ticked files, as offered by the capture.
    selected: Vec<String>,
    /// The folder the ticked files are read from; their paths are relative to it.
    files_root: Option<String>,
    /// The name typed in the layout's Zip name block, as a file name. `None`: the default name.
    zip_name: Option<String>,
    /// This capture's own copies of ticked files: the folder holding them, and for each file
    /// (by its path in the project) the path of its copy inside that folder.
    staged_root: Option<PathBuf>,
    staged: BTreeMap<String, String>,
    /// What this button sends beyond its text: the picture, and the files (attached or as text).
    send: SendRules,
    close: bool,
    notes: Vec<String>,
}

/// The connection and list a dropdown of the layout may be filled from, and nothing else: the
/// window cannot ask for a list the layout does not name. With `parent`, the second list of a
/// dropdown that goes deeper, asked for as `<list>:<parent>`.
pub fn options_asked(
    layout: &Layout,
    block_id: &str,
    parent: Option<&str>,
) -> Option<(String, String)> {
    let d = match &layout.block(block_id)?.kind {
        BlockKind::Dropdown(d) => d,
        // a Tags block may suggest from one list, never a list inside a choice
        BlockKind::Tags(t) => {
            let list = t.suggest.as_ref().filter(|_| parent.is_none())?;
            return Some((list.connection_id.clone(), list.options_id.clone()));
        }
        _ => return None,
    };
    let DropdownSource::WebApi {
        connection_id,
        options_id,
    } = &d.source
    else {
        return None;
    };
    match (parent, &d.deeper) {
        (None, _) => Some((connection_id.clone(), options_id.clone())),
        (Some(parent), Some(deeper)) if !parent.trim().is_empty() => Some((
            connection_id.clone(),
            format!("{}:{}", deeper.list, parent.trim()),
        )),
        _ => None,
    }
}

fn field_kind(k: InfoKind) -> FieldKind {
    match k {
        InfoKind::Text => FieldKind::Text,
        InfoKind::Choice => FieldKind::Choice,
        InfoKind::MultiChoice => FieldKind::MultiChoice,
        InfoKind::Number => FieldKind::Number,
        InfoKind::Flag => FieldKind::Flag,
    }
}

fn render(text: &str, view: &SessionView) -> String {
    template::render(text, |name| view.variables.get(name).cloned()).text
}

/// What a text block holds now (the window's value, else the capture's).
fn text_value(view: &SessionView, req: &Request, id: &str) -> String {
    req.texts
        .get(id)
        .cloned()
        .or_else(|| view.texts.get(id).map(|t| t.value.clone()))
        .unwrap_or_default()
}

/// The tags of a Tags block now (the window's, else the capture's), cleaned, at most 25.
fn tags_value(view: &SessionView, req: &Request, id: &str) -> Vec<String> {
    let given: Vec<String> = req
        .tags
        .get(id)
        .cloned()
        .or_else(|| view.tags.get(id).map(|t| t.tags.clone()))
        .unwrap_or_default();
    // a tag never holds a comma: it would split in two
    cargoframe_layout::split_tags(given.iter().map(|t| t.as_str())).0
}

/// One mapped value: the text, and whether it was picked from a list the service supplied.
fn source_value(
    layout: &Layout,
    view: &SessionView,
    req: &Request,
    source: &ValueSource,
) -> (String, bool, bool) {
    match source {
        ValueSource::Block(id) => match layout.block(id).map(|b| &b.kind) {
            Some(BlockKind::Text(_)) => (text_value(view, req, id), false, false),
            Some(BlockKind::Tags(_)) => (tags_value(view, req, id).join(", "), false, false),
            Some(BlockKind::Dropdown(d)) => {
                let value = req.dropdowns.get(id).cloned().flatten().unwrap_or_default();
                (
                    value,
                    matches!(d.source, DropdownSource::WebApi { .. }),
                    true,
                )
            }
            _ => (String::new(), false, false),
        },
        ValueSource::Rule(id) => (
            view.variables
                .get(&format!("rule.{id}"))
                .cloned()
                .unwrap_or_default(),
            false,
            false,
        ),
        ValueSource::Fixed(text) => (render(text, view), false, false),
    }
}

fn blocking_reasons(layout: &Layout, button_id: &str, reg: &AppRegistry) -> Vec<String> {
    let SharingState::Disabled(codes) = sharing_state(layout, button_id, reg) else {
        return Vec::new();
    };
    let messages: Vec<String> = validate(layout, reg)
        .into_iter()
        .filter(|i| i.block_id.as_deref() == Some(button_id) && codes.contains(&i.code))
        .map(|i| i.message)
        .collect();
    if messages.is_empty() {
        vec!["This sharing button cannot send right now".to_string()]
    } else {
        messages
    }
}

fn prepare(svc: &Services, session: &Session, req: &Request) -> Result<Prepared, Vec<String>> {
    let view = &session.view;
    if req.session_id != view.session_id {
        return Err(vec![
            "This capture is no longer open. Take a new one".to_string()
        ]);
    }
    let Some(layout) = view.layout.as_ref() else {
        return Err(vec!["This capture has no layout".to_string()]);
    };
    if !matches!(
        layout.block(&req.button_id).map(|b| &b.kind),
        Some(BlockKind::SharingButton(_))
    ) {
        return Err(vec!["That sharing button is not in this layout".to_string()]);
    }

    let registry = AppRegistry::from_store(svc.store, svc.connections);
    let reasons = blocking_reasons(layout, &req.button_id, &registry);
    if !reasons.is_empty() {
        return Err(reasons);
    }
    // The button as this click has it: the boxes the person ticked or cleared are applied to a
    // copy, so the layout itself never changes. Boxes are checked again here, not trusted.
    let selection = select(
        layout,
        &req.button_id,
        &registry,
        req.send_blocks.as_deref(),
    )?;
    let button: SharingButton = selection.button;
    // With no active connection the button saves a zip in the folder for saved zips.
    let local = req.save_here
        || sharing_mode(layout, &req.button_id, &registry) == SharingMode::SaveLocally;
    let destination = if local {
        None
    } else {
        let Some(id) = button.connection_id.clone() else {
            return Err(vec!["Choose where this button shares to".to_string()]);
        };
        let connection = svc.connections.load(svc.store, &id).map_err(|e| vec![e])?;
        Some(Destination { id, connection })
    };
    let connection_id: Option<&str> = destination.as_ref().map(|d| d.id.as_str());
    let kind = connection_id
        .and_then(|id| svc.connections.info(svc.store, id))
        .map(|i| i.kind);
    let is_ai = kind.is_some_and(|k| k.is_ai_destination());

    let mut reasons: Vec<String> = Vec::new();

    // The prompt.
    let prompt = button
        .prompt_block_id
        .as_deref()
        .filter(|id| matches!(layout.block(id).map(|b| &b.kind), Some(BlockKind::Text(_))))
        .map(|id| text_value(view, req, id).trim().to_string())
        .filter(|p| !p.is_empty());
    if is_ai && prompt.is_none() {
        reasons.push("Write the prompt before sharing".to_string());
    }

    // The mapped fields, in the order the layout lists them.
    let infos: Vec<InfoType> = svc
        .store
        .load_all::<InfoType>(Kind::InfoTypes)
        .map(|l| l.items.into_iter().map(|(_, t)| t).collect())
        .unwrap_or_default();
    let info_for = |field: &str| {
        infos
            .iter()
            .find(|t| Some(t.connection_id.as_str()) == connection_id && t.field == field)
    };
    let mut fields: Vec<Field> = Vec::new();
    // The mappings this click uses: the button's own, the chat of a Claude button, and none for a
    // block whose box was cleared.
    let mappings = selection.mappings;
    for mapping in &mappings {
        let (mut value, from_dropdown_list, is_dropdown) =
            source_value(layout, view, req, &mapping.source);
        // a field fill with steps of its own runs after every block is in (below)
        let info = info_for(&mapping.field).filter(|t| !t.fills());
        if value.trim().is_empty() {
            if let Some(t) = info.filter(|t| !t.default_template.trim().is_empty()) {
                value = render(&t.default_template, view);
            }
        }
        if value.trim().is_empty() && info.is_some_and(|t| t.required) {
            let name = info.map(|t| t.name.as_str()).unwrap_or(&mapping.field);
            reasons.push(format!("\u{201c}{name}\u{201d} needs a value"));
        }
        let is_tags = matches!(
            &mapping.source,
            ValueSource::Block(id) if matches!(layout.block(id).map(|b| &b.kind), Some(BlockKind::Tags(_)))
        );
        let (kind, from_list) = match info.or_else(|| info_for(&mapping.field)) {
            Some(t) => (
                field_kind(t.kind),
                from_dropdown_list || matches!(t.options, InfoOptions::Api { .. }),
            ),
            None if is_tags => (FieldKind::MultiChoice, false),
            None if is_dropdown => (FieldKind::Choice, from_dropdown_list),
            None => (FieldKind::Text, false),
        };
        fields.push(Field {
            key: mapping.field.clone(),
            value,
            kind,
            from_list,
        });
    }
    // The second step of a dropdown that goes deeper fills a field of its own, by itself.
    for mapping in &mappings {
        let ValueSource::Block(id) = &mapping.source else {
            continue;
        };
        let Some(BlockKind::Dropdown(DropdownBlock {
            deeper: Some(deeper),
            ..
        })) = layout.block(id).map(|b| &b.kind)
        else {
            continue;
        };
        let value = req.deeper.get(id).cloned().flatten().unwrap_or_default();
        if !value.trim().is_empty() && !fields.iter().any(|f| f.key == deeper.field) {
            fields.push(Field {
                key: deeper.field.clone(),
                value,
                kind: FieldKind::Choice,
                from_list: true,
            });
        }
    }
    // Field fills: their steps run on every share to the service, after the blocks are in.
    if connection_id.is_some() {
        let spec = connection_id
            .and_then(|id| svc.connections.describe(svc.store, id).ok())
            .unwrap_or_default();
        let fills = infos.iter().filter(|t| {
            Some(t.connection_id.as_str()) == connection_id && t.fills() && t.applies_in(&layout.id)
        });
        for t in fills {
            let at = fields.iter().position(|f| f.key == t.field);
            let mut value = at.map(|i| fields[i].value.clone()).unwrap_or_default();
            let many = t.kind == InfoKind::MultiChoice
                || spec
                    .fields
                    .iter()
                    .any(|f| f.key == t.field && f.hint.to_lowercase().contains("comma"));
            let lines = matches!(
                t.field.as_str(),
                "description" | "body" | "message" | "text" | "content" | "comment" | "environment"
            );
            for step in t.all_steps() {
                if let Some(w) = &step.when {
                    let subject = w
                        .subject
                        .trim()
                        .trim_start_matches("{{")
                        .trim_end_matches("}}");
                    let of = if subject.is_empty() {
                        value.clone()
                    } else {
                        view.variables
                            .get(subject.trim())
                            .cloned()
                            .unwrap_or_default()
                    };
                    if !holds(w, &of) {
                        continue;
                    }
                }
                for a in &step.then {
                    value = act(&value, a.act, &render(&a.text, view), many, lines);
                }
            }
            if value.trim().is_empty() && t.required {
                reasons.push(format!("\u{201c}{}\u{201d} needs a value", t.name));
            }
            match at {
                Some(i) => fields[i].value = value,
                None if !value.trim().is_empty() => fields.push(Field {
                    key: t.field.clone(),
                    value,
                    kind: field_kind(t.kind),
                    from_list: matches!(t.options, InfoOptions::Api { .. }),
                }),
                None => {}
            }
        }
    }
    // A required field that no mapping fills can never be satisfied: say so.
    for t in infos
        .iter()
        .filter(|t| Some(t.connection_id.as_str()) == connection_id && t.required && !t.fills())
    {
        if !mappings.iter().any(|m| m.field == t.field) {
            reasons.push(format!(
                "\u{201c}{}\u{201d} is required: map it to a block in the layout",
                t.name
            ));
        }
    }

    // The ticked files must be ones this capture offered.
    let mut selected: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for block in &layout.blocks {
        if !matches!(block.kind, BlockKind::Files(_))
            || button.send.files == FilesHow::None
            || !selection.files_blocks.contains(&block.id)
        {
            continue;
        }
        let Some(wanted) = req.files.get(&block.id) else {
            continue;
        };
        for path in wanted {
            // found in the project, or added by hand (then already copied into the capture's folder)
            if !session.offers(&block.id, path) {
                reasons.push(format!(
                    "\u{201c}{path}\u{201d} is not one of the files offered for this capture"
                ));
            } else if seen.insert(path.clone()) {
                selected.push(path.clone());
            }
        }
    }
    let added_by_hand = |p: &String| p.starts_with(&format!("{}/", crate::session::ADDED));
    if selected.iter().any(|p| !added_by_hand(p)) && session.files_root.is_none() {
        reasons.push("There is no folder to read the ticked files from".to_string());
    }
    if !reasons.is_empty() {
        return Err(reasons);
    }

    // A video capture sends its recording where a screenshot capture sends the picture. With a
    // video block and a screenshot block in one layout, each goes by its own box, and the
    // screenshot is the picture the person took from the recording.
    let in_layout = |wanted: fn(&BlockKind) -> bool| layout.blocks.iter().find(|b| wanted(&b.kind));
    let video_block = in_layout(|k| matches!(k, BlockKind::Video(_)));
    let shot_block = in_layout(|k| matches!(k, BlockKind::Screenshot));
    let goes = |b: Option<&cargoframe_layout::Block>| {
        b.is_some_and(|b| !selection.left_out.contains(&b.id))
    };
    let (image, still) = match &session.clip {
        Some(clip) => {
            let both = video_block.is_some() && shot_block.is_some();
            let recording_goes = if both {
                goes(video_block)
            } else {
                button.send.screenshot
            };
            let recording = if recording_goes {
                let edited = match edited_recording(svc, session, clip, req.clip_edit) {
                    Ok(c) => c,
                    Err(why) => return Err(vec![why]),
                };
                match video_attachment(edited.as_deref().unwrap_or(&clip.path), kind) {
                    Ok(a) => Some(a),
                    Err(why) => return Err(vec![why]),
                }
            } else {
                None
            };
            let picture = (both && req.still && goes(shot_block)).then(|| Attachment {
                name: "screenshot.png".into(),
                mime: "image/png".into(),
                data: session.image_to_send(),
            });
            match (recording, picture) {
                // without the recording the picture is the one thing sent, where it would be
                (None, picture) => (picture, None),
                (recording, picture) => (recording, picture),
            }
        }
        None if !button.send.screenshot => (None, None),
        None => (
            Some(Attachment {
                name: "screenshot.png".into(),
                mime: "image/png".into(),
                data: if req.image_edited {
                    session.image_to_send()
                } else {
                    session.png.clone()
                },
            }),
            None,
        ),
    };
    let title = view
        .app
        .as_ref()
        .map(|a| a.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Screen".to_string());
    let payload = Payload {
        title,
        image,
        still,
        files_zip: None,
        local: None,
        inline_files: Vec::new(),
        prompt,
        fields,
        vars: view.variables.clone(),
    };
    // the screenshot and the recording can be put in the text only where the service shows
    // them there; elsewhere their places are taken out, and the files go the usual way
    let payload = match kind {
        Some(ConnectionKind::Jira | ConnectionKind::Github | ConnectionKind::Teams) => payload,
        _ => payload.without_media_places(),
    };
    let service = match &destination {
        None => "local".to_string(),
        Some(d) if d.connection.name.trim().is_empty() => d.id.clone(),
        Some(d) => d.connection.name.trim().to_string(),
    };
    // Only the first block with the Zip name role counts.
    let zip_name = layout
        .blocks
        .iter()
        .find(|b| matches!(&b.kind, BlockKind::Text(t) if t.role == TextRole::ZipName))
        .filter(|b| !selection.left_out.contains(&b.id))
        .and_then(|b| custom_zip_file_name(&text_value(view, req, &b.id)));
    Ok(Prepared {
        service,
        destination,
        trigger_name: view.trigger_name.clone(),
        payload,
        selected,
        files_root: session.files_root.clone(),
        zip_name,
        staged_root: None,
        staged: BTreeMap::new(),
        send: button.send,
        close: button.on_success == OnSuccess::CloseWindow,
        notes: Vec::new(),
    })
}

/// The largest recording CargoFrame reads into memory to send (the services have limits of
/// their own, which they report when a file is too big).
pub const MAX_VIDEO_BYTES: u64 = 100 * 1024 * 1024;

/// The file to send for a video capture whose recording the person edited in the window: a new
/// file in the capture's own folder, or `None` when nothing was changed (the whole recording is
/// sent as it is).
fn edited_recording(
    svc: &Services,
    session: &Session,
    clip: &crate::session::Clip,
    edit: Option<crate::video::ClipEdit>,
) -> Result<Option<PathBuf>, String> {
    let Some(edit) = edit.filter(|e| !e.changes_nothing(clip.duration_ms)) else {
        return Ok(None);
    };
    let output = session
        .with_dir(svc.workspace, |d| d.path().join("recording-edited.mp4"))
        .map_err(|e| format!("CargoFrame could not make room for the edited recording: {e}"))?;
    let settings = svc
        .store
        .load_or_default::<cargoframe_config::Video>()
        .map(crate::video::normalized)
        .unwrap_or_default();
    let mark = if edit.mark {
        let png = session
            .clip_mark()
            .ok_or("The mark was not drawn: try again")?;
        let path = session
            .with_dir(svc.workspace, |d| d.path().join("clip-mark.png"))
            .map_err(|e| format!("CargoFrame could not keep the mark: {e}"))?;
        fs::write(&path, png).map_err(|e| format!("CargoFrame could not keep the mark: {e}"))?;
        Some(path)
    } else {
        None
    };
    let keys = match edit.keys {
        Some(_) => Some(keys_list(svc, session, clip, &edit)?),
        None => None,
    };
    let follow = session
        .view
        .video
        .as_ref()
        .map(|v| v.follow.as_slice())
        .unwrap_or_default();
    let layers = crate::video::Layers {
        mark: mark.as_deref(),
        follow,
        keys: keys.as_deref(),
    };
    crate::video::edit_clip(&settings, &clip.path, &output, edit, layers)?;
    Ok(Some(output))
}

/// Write the pictures of the keys the window drew into the capture's folder, and the list
/// ffmpeg plays them from. The window sends a see-through blank picture first, then one for
/// each line of keys the capture holds, in order.
fn keys_list(
    svc: &Services,
    session: &Session,
    clip: &crate::session::Clip,
    edit: &crate::video::ClipEdit,
) -> Result<PathBuf, String> {
    let lines = session
        .view
        .video
        .as_ref()
        .map(|v| v.keys.as_slice())
        .unwrap_or_default();
    let pictures = session
        .clip_keys()
        .filter(|p| p.len() == lines.len() + 1)
        .ok_or("The keys pressed were not drawn: try again")?;
    let keep = |e: std::io::Error| format!("CargoFrame could not keep the keys pressed: {e}");
    let dir = session
        .with_dir(svc.workspace, |d| d.path().join("keys"))
        .map_err(keep)?;
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(keep)?;
    let blank = dir.join("blank.png");
    fs::write(&blank, &pictures[0]).map_err(keep)?;
    let mut shown = Vec::with_capacity(lines.len());
    for (i, (line, png)) in lines.iter().zip(&pictures[1..]).enumerate() {
        let path = dir.join(format!("k{i:04}.png"));
        fs::write(&path, png).map_err(keep)?;
        shown.push((path, line.from_ms, line.to_ms));
    }
    let end = edit.end_ms.unwrap_or(clip.duration_ms);
    let list = dir.join("keys.txt");
    fs::write(
        &list,
        cargoframe_video::keys_list(&blank, &shown, edit.start_ms, end),
    )
    .map_err(keep)?;
    Ok(list)
}

/// The recording of a video capture as the attachment a connector sends, or why it cannot go to
/// this destination.
fn video_attachment(
    path: &Path,
    kind: Option<cargoframe_layout::ConnectionKind>,
) -> Result<Attachment, String> {
    if let Some(kind) = kind {
        let caps = cargoframe_layout::Capabilities::for_kind(kind, cfg!(windows));
        if !caps.video.works() {
            return Err(format!(
                "This destination cannot take a video: {}",
                caps.video.note
            ));
        }
    }
    let size = fs::metadata(path)
        .map(|m| m.len())
        .map_err(|_| "The recording is no longer there: record it again".to_string())?;
    if size > MAX_VIDEO_BYTES {
        return Err(format!(
            "The recording is {} MB and CargoFrame sends up to {} MB: record a shorter one, or lower the picture size in Settings, Video",
            size / (1024 * 1024),
            MAX_VIDEO_BYTES / (1024 * 1024),
        ));
    }
    let data = fs::read(path).map_err(|e| format!("The recording could not be read: {e}"))?;
    Ok(Attachment {
        name: crate::session::CLIP_NAME.into(),
        mime: "video/mp4".into(),
        data,
    })
}

fn manifest_info(svc: &Services, p: &Prepared, notes: Vec<String>) -> ManifestInfo {
    ManifestInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        created: created(&svc.now),
        trigger: p.trigger_name.clone(),
        service: p.service.clone(),
        notes,
    }
}

/// Find (or make) this capture's own copy of each ticked file. A zip then holds the files as
/// they were at the moment of the capture, not as they are when the button is pressed. Waits
/// for the copying that started with the capture (see `Session::stage_preselected`). A file
/// that cannot be copied is read from the project as before, so it is skipped or reported
/// the way it always was.
fn adopt_staged(svc: &Services, session: &Session, p: &mut Prepared) {
    if p.selected.is_empty() {
        return;
    }
    let found = session.with_dir(svc.workspace, |dir| {
        // with no project folder only files added by hand are ticked: they are copies already
        let root = p
            .files_root
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.path().to_path_buf());
        let files_dir = dir.files_dir();
        let mut staged = BTreeMap::new();
        let mut total = 0u64;
        for rel in &p.selected {
            let Ok(copy) = dir.stage(&root, rel, MAX_STAGED_FILE_BYTES) else {
                continue;
            };
            total += fs::metadata(&copy).map(|m| m.len()).unwrap_or(0);
            if total > MAX_STAGED_TOTAL_BYTES {
                break;
            }
            if let Ok(inside) = copy.strip_prefix(&files_dir) {
                staged.insert(rel.clone(), inside.to_string_lossy().replace('\\', "/"));
            }
        }
        (files_dir, staged)
    });
    match found {
        Ok((files_dir, staged)) => {
            p.staged_root = Some(files_dir);
            p.staged = staged;
        }
        Err(e) => tracing::warn!("the capture's folder could not be used for the files: {e}"),
    }
}

/// Where a ticked file is read from when zipped: this capture's copy of it, else the project.
fn file_source(p: &Prepared, rel: &str) -> Source {
    if let (Some(root), Some(inside)) = (&p.staged_root, p.staged.get(rel)) {
        return Source::ProjectFile {
            root: root.clone(),
            relative: inside.clone(),
        };
    }
    Source::ProjectFile {
        root: PathBuf::from(p.files_root.clone().unwrap_or_default()),
        relative: rel.to_string(),
    }
}

/// The ticked files as one zip, named by their path in the project.
fn zip_selected(svc: &Services, p: &Prepared) -> Result<(Vec<u8>, Vec<String>), BundleError> {
    let items = p
        .selected
        .iter()
        .map(|rel| Item {
            name: rel.clone(),
            source: file_source(p, rel),
            kind: "file",
        })
        .collect();
    let mut out = Cursor::new(Vec::new());
    let manifest = build_zip(
        &mut out,
        items,
        manifest_info(svc, p, Vec::new()),
        Limits::default(),
    )?;
    Ok((out.into_inner(), manifest.notes))
}

/// A chat takes a limited number of files in one message, and the screenshot is one of them.
fn check_file_count(ticked: usize, with_screenshot: bool) -> Result<(), String> {
    let total = ticked + usize::from(with_screenshot);
    if total > MAX_FILES {
        return Err(format!(
            "Claude takes at most {MAX_FILES} files in one message{}: untick {} of the files",
            if with_screenshot {
                ", including the screenshot"
            } else {
                ""
            },
            total - MAX_FILES
        ));
    }
    Ok(())
}

/// A file's bytes as text, if they are text: valid UTF-8 with no NUL byte. A byte-order mark at
/// the start is dropped.
fn as_text(bytes: &[u8]) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    Some(text.trim_start_matches('\u{feff}').to_string())
}

/// Take the text files that fit into `limit` bytes (all together) out of the files to attach,
/// in the order they were ticked, to be written into the message. The rest stay as files.
fn move_text_inline(local: &mut LocalCopies, limit: usize) -> Vec<InlineFile> {
    let mut inline = Vec::new();
    let mut used = 0usize;
    let mut stay = Vec::new();
    for (name, path) in std::mem::take(&mut local.files) {
        let room = limit.saturating_sub(used);
        let text = fs::metadata(&path)
            .ok()
            .filter(|m| m.len() as usize <= room)
            .and_then(|_| fs::read(&path).ok())
            .and_then(|bytes| as_text(&bytes))
            .filter(|t| t.len() <= room);
        match text {
            Some(text) => {
                used += text.len();
                inline.push(InlineFile { name, text });
            }
            None => stay.push((name, path)),
        }
    }
    local.files = stay;
    inline
}

/// The screenshot as shown and the ticked files, as copies in this capture's folder, for a
/// destination that takes real files. The folder goes when the capture window closes.
fn stage_local(svc: &Services, session: &Session, p: &Prepared) -> Result<LocalCopies, String> {
    // Files written into the message are not files in the chat: they are counted once the text
    // ones have moved out.
    if p.send.files != FilesHow::Inline {
        check_file_count(p.selected.len(), p.payload.image.is_some())?;
    }
    let screenshot_bytes = p.payload.image.as_ref().map(|a| a.data.as_slice());
    let root = p.files_root.as_deref().map(PathBuf::from);
    let total_limit = Limits::default().max_total_bytes;
    session
        .with_dir(svc.workspace, |dir| {
            let screenshot = screenshot_bytes
                .map(|bytes| {
                    dir.write_screenshot(bytes)
                        .map_err(|e| format!("The screenshot could not be saved for sending: {e}"))
                })
                .transpose()?;
            let mut files = Vec::new();
            let mut total = 0u64;
            for rel in &p.selected {
                // a file added by hand is in the capture's folder already, whatever the root
                let root = root
                    .as_deref()
                    .or_else(|| dir.staged(rel).map(|_| dir.path()))
                    .ok_or("There is no folder to read the ticked files from")?;
                let copy = dir
                    .stage(root, rel, MAX_FILE_BYTES)
                    .map_err(|e| format!("\u{201c}{rel}\u{201d} cannot be sent: {e}"))?;
                total += std::fs::metadata(&copy).map(|m| m.len()).unwrap_or(0);
                if total > total_limit {
                    return Err(format!(
                        "The ticked files are larger than {} MB together: untick some",
                        total_limit / (1024 * 1024)
                    ));
                }
                files.push((rel.clone(), copy));
            }
            Ok(LocalCopies { screenshot, files })
        })
        .map_err(|e| format!("The temporary folder for this capture could not be made: {e}"))?
}

/// Walk the safe-settings ladder with a zip of everything the share held.
struct ZipIo<'a> {
    source: PathBuf,
    local: PathBuf,
    picker: &'a mut dyn FnMut(u32) -> Option<PickedFolder>,
}

impl FallbackIo for ZipIo<'_> {
    fn save_zip(&mut self, dir: &str, file_name: &str) -> Result<String, WriteFailure> {
        // Our own folder may not exist yet. Any other folder must: a mistyped path is reported,
        // never silently created.
        if Path::new(dir) == self.local {
            let _ = fs::create_dir_all(dir);
        }
        copy_atomic(&self.source, Path::new(dir), file_name).map(|p| p.display().to_string())
    }

    fn pick_folder(&mut self, attempt: u32) -> Option<PickedFolder> {
        (self.picker)(attempt)
    }

    fn local_data_dir(&self) -> String {
        self.local.display().to_string()
    }
}

/// Why a zip could not be written, in words.
fn describe_write(f: &WriteFailure) -> String {
    match f {
        WriteFailure::NotWritable => "the folder is missing or cannot be written to".to_string(),
        WriteFailure::DiskFull => "the disk is full".to_string(),
        WriteFailure::Other(e) => e.clone(),
    }
}

/// Everything the share held, as a zip in CargoFrame's temporary folder: the picture, what was
/// typed and picked, the ticked files, and why it was kept (`failure`, when a send failed).
/// Returns where the zip is. `None` only if the zip itself could not be built.
fn build_keep_zip(
    svc: &Services,
    p: &Prepared,
    view: &SessionView,
    failure: Option<(&SendError, u32)>,
) -> Option<PathBuf> {
    let tmp_dir = svc.store.dirs().data().join("tmp");
    fs::create_dir_all(&tmp_dir).ok()?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let source = tmp_dir.join(format!("keep-{}-{nonce}.zip", std::process::id()));

    let share_json = serde_json::json!({
        "destination": p.service,
        "trigger": p.trigger_name,
        "title": p.payload.title,
        "prompt": p.payload.prompt,
        "fields": p.payload.fields.iter()
            .map(|f| serde_json::json!({"key": f.key, "value": f.value}))
            .collect::<Vec<_>>(),
        "variables": view.variables,
    });
    let mut items = Vec::new();
    // The picture is left out when the button's rules leave it out.
    for image in p.payload.image.iter().chain(p.payload.still.iter()) {
        items.push(Item {
            name: image.name.clone(),
            source: Source::Bytes(image.data.clone()),
            kind: if image.mime.starts_with("video/") {
                "video"
            } else {
                "screenshot"
            },
        });
    }
    items.push(Item {
        name: "share.json".into(),
        source: Source::Bytes(serde_json::to_vec_pretty(&share_json).unwrap_or_default()),
        kind: "text",
    });
    if let Some((failure, attempts)) = failure {
        let error_json = serde_json::json!({
            "failure": failure.kind.label(),
            "message": failure.message,
            "attempts": attempts,
            "when": created(&svc.now).iso(),
        });
        items.push(Item {
            name: "error.json".into(),
            source: Source::Bytes(serde_json::to_vec_pretty(&error_json).unwrap_or_default()),
            kind: "text",
        });
    }
    if !p.selected.is_empty() {
        for rel in &p.selected {
            items.push(Item {
                name: format!("files/{rel}"),
                source: file_source(p, rel),
                kind: "file",
            });
        }
    }
    let built = fs::File::create(&source)
        .map_err(|e| e.to_string())
        .and_then(|f| {
            build_zip(
                f,
                items,
                manifest_info(svc, p, p.notes.clone()),
                Limits::default(),
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        });
    if let Err(e) = built {
        tracing::error!("could not build the zip to keep: {e}");
        let _ = fs::remove_file(&source);
        return None;
    }
    Some(source)
}

/// The zip's file name: what the person typed in the Zip name block, else one made from the
/// moment the share happened.
fn zip_name(svc: &Services, p: &Prepared) -> String {
    if let Some(name) = &p.zip_name {
        return name.clone();
    }
    let m = &svc.now;
    file_name_for(
        (m.year.clamp(0, 9999) as u16, m.month, m.day),
        (m.hour, m.minute, m.second),
        &p.service,
    )
}

/// Build the fallback zip and put it somewhere safe. `None` only if the zip itself could not be
/// built (then the failure message is all there is).
fn keep_a_copy(
    svc: &Services,
    picker: &mut dyn FnMut(u32) -> Option<PickedFolder>,
    p: &Prepared,
    view: &SessionView,
    safe: &Safe,
    failure: &SendError,
    attempts: u32,
) -> Option<SavedCopy> {
    let source = build_keep_zip(svc, p, view, Some((failure, attempts)))?;
    let file_name = zip_name(svc, p);
    let cfg = LadderConfig {
        safe_folder: safe.folder.clone(),
        ..LadderConfig::default()
    };
    let mut io = ZipIo {
        source: source.clone(),
        local: svc.store.dirs().kept_zips(),
        picker,
    };
    let report = run_ladder(&cfg, &mut io, &file_name);
    let saved = match report.saved {
        Saved::SafeFolder { path } => SavedCopy {
            place: "safe_folder",
            path,
            note: None,
        },
        Saved::Picked { path, remember_dir } => {
            if let Some(dir) = remember_dir {
                let mut s = svc.store.load_or_default::<Safe>().unwrap_or_default();
                s.folder = Some(dir);
                if let Err(e) = svc.store.save(&s) {
                    tracing::warn!("could not remember the folder: {e}");
                }
            }
            SavedCopy {
                place: "picked_folder",
                path,
                note: None,
            }
        }
        Saved::LocalData { path, reason } => SavedCopy {
            place: "kept_locally",
            path,
            note: Some(reason),
        },
        Saved::Unsaved { reason } => {
            // Keep the temporary zip: it is the only copy left.
            return Some(SavedCopy {
                place: "not_saved",
                path: source.display().to_string(),
                note: Some(reason),
            });
        }
    };
    let _ = fs::remove_file(&source);
    Some(saved)
}

/// No active connection: put a zip of the share in the folder for saved zips. That folder must
/// be set. There is no asking, no fallback to another folder: the person chose "Save locally"
/// by having no connection, and the setting says where. If it cannot be written the share is
/// refused with the reason, and everything is still in the window.
fn save_locally(svc: &Services, p: &Prepared, view: &SessionView) -> ShareResult {
    let folder = svc
        .store
        .load_or_default::<Safe>()
        .ok()
        .and_then(|s| s.folder)
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty());
    let Some(folder) = folder else {
        return blocked(NEEDS_SAVE_FOLDER);
    };
    if !Path::new(&folder).is_dir() {
        return blocked(format!(
            "The folder for saved zips cannot be used: {folder} is missing. Choose it again in Settings"
        ));
    }
    let Some(source) = build_keep_zip(svc, p, view, None) else {
        return blocked("The zip could not be built");
    };
    let saved = copy_atomic(&source, Path::new(&folder), &zip_name(svc, p));
    let _ = fs::remove_file(&source);
    match saved {
        Ok(path) => {
            tracing::info!("share saved locally");
            ShareResult::SavedLocally {
                path: path.display().to_string(),
                notes: p.notes.clone(),
                close: p.close,
            }
        }
        Err(e) => blocked(format!(
            "Could not save in {folder}: {}",
            describe_write(&e)
        )),
    }
}

/// Do the share. Always returns a result the window can show; nothing is lost on failure.
pub fn run(
    svc: &Services,
    picker: &mut dyn FnMut(u32) -> Option<PickedFolder>,
    sleep: &mut dyn FnMut(Duration),
    session: &Session,
    req: &Request,
) -> ShareResult {
    let mut prepared = match prepare(svc, session, req) {
        Ok(p) => p,
        Err(reasons) => return ShareResult::Blocked { reasons },
    };
    // The ticked files go into the saved zip one by one; nothing is sent, so nothing is zipped
    // twice.
    let Some(Destination {
        id: connection_id,
        connection,
    }) = prepared.destination.take()
    else {
        adopt_staged(svc, session, &mut prepared);
        return save_locally(svc, &prepared, &session.view);
    };

    // Claude takes real files pasted into the chat, not a zip: copy them into the capture's own
    // folder, as they are now.
    let takes_files = matches!(connection.settings, Settings::Claude(_));
    if takes_files {
        match stage_local(svc, session, &prepared) {
            Ok(mut local) => {
                if prepared.send.files == FilesHow::Inline {
                    let limit = prepared.send.inline_limit_bytes();
                    prepared.payload.inline_files = move_text_inline(&mut local, limit);
                    let stayed = local.files.len();
                    if stayed > 0 {
                        prepared.notes.push(format!(
                            "{stayed} file{} not plain text or past the {} KB limit, so {} attached instead of written into the message",
                            if stayed == 1 { " was" } else { "s were" },
                            limit / 1024,
                            if stayed == 1 { "it was" } else { "they were" },
                        ));
                    }
                    if let Err(reason) =
                        check_file_count(local.files.len(), prepared.payload.image.is_some())
                    {
                        return blocked(reason);
                    }
                }
                prepared.payload.local = Some(local);
                // So a copy kept after a failed send holds the same files.
                adopt_staged(svc, session, &mut prepared);
            }
            Err(reason) => return blocked(reason),
        }
    } else if !prepared.selected.is_empty() {
        adopt_staged(svc, session, &mut prepared);
        match zip_selected(svc, &prepared) {
            Ok((bytes, notes)) => {
                prepared.notes.extend(notes);
                prepared.payload.files_zip = Some(Attachment {
                    name: "files.zip".into(),
                    mime: "application/zip".into(),
                    data: bytes,
                });
            }
            Err(e) => return blocked(format!("The ticked files cannot be zipped: {e}")),
        }
    }

    let safe = svc.store.load_or_default::<Safe>().unwrap_or_default();
    let policy = RetryPolicy {
        max_retries: safe.max_retries,
        ..RetryPolicy::default()
    };
    let timeout = Duration::from_secs(u64::from(safe.timeout_secs));
    let progress = Progress::new();

    let RetryResult { result, attempts } = {
        let mut op = |_attempt: u32| {
            svc.connections.with_ctx(
                &connection_id,
                &connection,
                svc.browser,
                svc.deliver,
                &progress,
                timeout,
                svc.cancelled,
                |connector, ctx| connector.send(ctx, &prepared.payload),
            )
        };
        run_with_retry(&policy, sleep, svc.cancelled, &mut op)
    };

    match result {
        Ok(outcome) => {
            tracing::info!(
                connection = %connection_id,
                attempts,
                "share sent"
            );
            // A share to an existing Claude chat makes it the most recent one.
            if let Some(chat) = prepared.payload.value("chat").filter(|c| *c != "new") {
                crate::chats::touch(svc.store, chat, crate::connections::unix_now());
            }
            let mut notes = prepared.notes.clone();
            notes.extend(outcome.notes);
            ShareResult::Sent {
                summary: outcome.summary,
                url: outcome.url,
                notes,
                close: prepared.close,
                place: outcome.place,
            }
        }
        Err(err) => {
            tracing::warn!(
                connection = %connection_id,
                attempts,
                failure = err.kind.label(),
                "share failed"
            );
            let reconnect = err.kind.needs_reconnect();
            if reconnect {
                svc.connections.mark_expired(&connection_id);
            }
            let saved = keep_a_copy(svc, picker, &prepared, &session.view, &safe, &err, attempts);
            ShareResult::Failed {
                label: err.kind.label().to_string(),
                message: err.message,
                reconnect,
                attempts,
                saved,
            }
        }
    }
}

#[cfg(test)]
#[path = "share_tests.rs"]
mod tests;
