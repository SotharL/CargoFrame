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
use cargoframe_config::{InfoKind, InfoOptions, InfoType, Kind, Safe, Store};
use cargoframe_connectors::{
    claude::{MAX_FILES, MAX_FILE_BYTES},
    Attachment, Browser, Connection, Deliver, Field, FieldKind, LocalCopies, Payload, Progress,
    Settings,
};
use cargoframe_context::template;
use cargoframe_layout::{
    sharing_mode, sharing_state, validate, BlockKind, DropdownSource, Layout, OnSuccess,
    SharingButton, SharingMode, SharingState, TextRole, ValueSource,
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
    /// The ticked files of each files block, relative to the project folder.
    pub files: BTreeMap<String, Vec<String>>,
    /// The picture was edited: send the edited one the window uploaded.
    pub image_edited: bool,
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
    project_folder: Option<String>,
    /// The name typed in the layout's Zip name block, as a file name. `None`: the default name.
    zip_name: Option<String>,
    close: bool,
    notes: Vec<String>,
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
    let button: SharingButton = match layout.block(&req.button_id).map(|b| &b.kind) {
        Some(BlockKind::SharingButton(b)) => b.clone(),
        _ => return Err(vec!["That sharing button is not in this layout".to_string()]),
    };

    let registry = AppRegistry::from_store(svc.store, svc.connections);
    let reasons = blocking_reasons(layout, &req.button_id, &registry);
    if !reasons.is_empty() {
        return Err(reasons);
    }
    // With no active connection the button saves a zip in the folder for saved zips.
    let local = sharing_mode(layout, &req.button_id, &registry) == SharingMode::SaveLocally;
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
    let is_ai = connection_id.is_some_and(|id| {
        svc.connections
            .info(svc.store, id)
            .is_some_and(|i| i.kind.is_ai_destination())
    });

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
    for mapping in &button.mappings {
        let (mut value, from_dropdown_list, is_dropdown) =
            source_value(layout, view, req, &mapping.source);
        let info = info_for(&mapping.field);
        if value.trim().is_empty() {
            if let Some(t) = info.filter(|t| !t.default_template.trim().is_empty()) {
                value = render(&t.default_template, view);
            }
        }
        if value.trim().is_empty() && info.is_some_and(|t| t.required) {
            let name = info.map(|t| t.name.as_str()).unwrap_or(&mapping.field);
            reasons.push(format!("\u{201c}{name}\u{201d} needs a value"));
        }
        let (kind, from_list) = match info {
            Some(t) => (
                field_kind(t.kind),
                from_dropdown_list || matches!(t.options, InfoOptions::Api { .. }),
            ),
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
    // A required field that no mapping fills can never be satisfied: say so.
    for t in infos
        .iter()
        .filter(|t| Some(t.connection_id.as_str()) == connection_id && t.required)
    {
        if !button.mappings.iter().any(|m| m.field == t.field) {
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
        if !matches!(block.kind, BlockKind::Files(_)) {
            continue;
        }
        let Some(wanted) = req.files.get(&block.id) else {
            continue;
        };
        let offered: BTreeSet<&str> = view
            .files
            .get(&block.id)
            .map(|f| f.entries.iter().map(|e| e.path.as_str()).collect())
            .unwrap_or_default();
        for path in wanted {
            if !offered.contains(path.as_str()) {
                reasons.push(format!(
                    "\u{201c}{path}\u{201d} is not one of the files offered for this capture"
                ));
            } else if seen.insert(path.clone()) {
                selected.push(path.clone());
            }
        }
    }
    if !selected.is_empty() && session.project_folder.is_none() {
        reasons.push("There is no project folder to read the ticked files from".to_string());
    }
    if !reasons.is_empty() {
        return Err(reasons);
    }

    let png = if req.image_edited {
        session.image_to_send()
    } else {
        session.png.clone()
    };
    let title = view
        .app
        .as_ref()
        .map(|a| a.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Screen".to_string());
    let payload = Payload {
        title,
        image: Some(Attachment {
            name: "screenshot.png".into(),
            mime: "image/png".into(),
            data: png,
        }),
        files_zip: None,
        local: None,
        prompt,
        fields,
        vars: view.variables.clone(),
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
        .and_then(|b| custom_zip_file_name(&text_value(view, req, &b.id)));
    Ok(Prepared {
        service,
        destination,
        trigger_name: view.trigger_name.clone(),
        payload,
        selected,
        project_folder: session.project_folder.clone(),
        zip_name,
        close: button.on_success == OnSuccess::CloseWindow,
        notes: Vec::new(),
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

/// The ticked files as one zip, named by their path in the project.
fn zip_selected(svc: &Services, p: &Prepared) -> Result<(Vec<u8>, Vec<String>), BundleError> {
    let root = PathBuf::from(p.project_folder.clone().unwrap_or_default());
    let items = p
        .selected
        .iter()
        .map(|rel| Item {
            name: rel.clone(),
            source: Source::ProjectFile {
                root: root.clone(),
                relative: rel.clone(),
            },
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
fn check_file_count(ticked: usize) -> Result<(), String> {
    if 1 + ticked > MAX_FILES {
        return Err(format!(
            "Claude takes at most {MAX_FILES} files in one message, including the screenshot: untick {} of the files",
            1 + ticked - MAX_FILES
        ));
    }
    Ok(())
}

/// The screenshot as shown and the ticked files, as copies in this capture's folder, for a
/// destination that takes real files. The folder goes when the capture window closes.
fn stage_local(svc: &Services, session: &Session, p: &Prepared) -> Result<LocalCopies, String> {
    check_file_count(p.selected.len())?;
    let screenshot_bytes = p.payload.image.as_ref().map(|a| a.data.as_slice());
    let root = p.project_folder.as_deref().map(PathBuf::from);
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
                let root = root
                    .as_deref()
                    .ok_or("There is no project folder to read the ticked files from")?;
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
    let mut items = vec![
        Item {
            name: "screenshot.png".into(),
            source: Source::Bytes(
                p.payload
                    .image
                    .as_ref()
                    .map(|a| a.data.clone())
                    .unwrap_or_default(),
            ),
            kind: "screenshot",
        },
        Item {
            name: "share.json".into(),
            source: Source::Bytes(serde_json::to_vec_pretty(&share_json).unwrap_or_default()),
            kind: "text",
        },
    ];
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
    if let Some(root) = &p.project_folder {
        for rel in &p.selected {
            items.push(Item {
                name: format!("files/{rel}"),
                source: Source::ProjectFile {
                    root: PathBuf::from(root),
                    relative: rel.clone(),
                },
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
        local: svc.store.dirs().data().join("kept"),
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
        return save_locally(svc, &prepared, &session.view);
    };

    // Claude takes real files pasted into the chat, not a zip: copy them into the capture's own
    // folder, as they are now.
    let takes_files = matches!(connection.settings, Settings::Claude(_));
    if takes_files {
        match stage_local(svc, session, &prepared) {
            Ok(local) => prepared.payload.local = Some(local),
            Err(reason) => return blocked(reason),
        }
    } else if !prepared.selected.is_empty() {
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
mod tests {
    use super::*;
    use crate::connections::tests::{fixture, save, Fixture};
    use crate::session::tests::{fake_capture, NOW};
    use crate::session::{prepare_with, Session};
    use cargoframe_config::{Kind, Trigger};
    use cargoframe_net::mock::{MockResponse, MockServer};
    use serde_json::json;
    use std::cell::RefCell;
    use std::io::Read;
    use std::sync::Mutex;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 1, 2, 3];
    const SECRET: &str = "TOP-SECRET-TOKEN-123";

    /// Opens links without doing anything.
    struct NoBrowser;
    impl Browser for NoBrowser {
        fn open(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
    }
    /// Reads what it is asked to paste while the files still exist, and says it was sent.
    #[derive(Default)]
    struct RecordingDeliver {
        seen: Mutex<Vec<Seen>>,
    }

    struct Seen {
        url: String,
        /// File name and content, in the order they would be pasted.
        files: Vec<(String, Vec<u8>)>,
        text: String,
        submit: bool,
    }

    impl Deliver for RecordingDeliver {
        fn deliver(
            &self,
            d: &cargoframe_connectors::Delivery,
            open: &dyn Fn(&str) -> Result<(), String>,
            _: &dyn Fn() -> bool,
        ) -> Result<cargoframe_connectors::Delivered, cargoframe_connectors::DeliverError> {
            open(&d.open_url).map_err(cargoframe_connectors::DeliverError::Open)?;
            let files = d
                .files
                .iter()
                .map(|p| {
                    (
                        p.file_name().unwrap().to_string_lossy().into_owned(),
                        fs::read(p).unwrap(),
                    )
                })
                .collect();
            self.seen.lock().unwrap().push(Seen {
                url: d.open_url.clone(),
                files,
                text: d.text.clone(),
                submit: d.submit,
            });
            Ok(cargoframe_connectors::Delivered {
                window: "Claude".into(),
                files: d.files.len(),
                skipped_files: 0,
                text: true,
                submitted: d.submit,
            })
        }
    }
    struct World {
        f: Fixture,
        server: MockServer,
        session: Session,
        out: tempfile::TempDir,
        /// Where captures keep their temporary copies.
        ws: Workspace,
        deliver: RecordingDeliver,
    }

    /// A project with a log file, a layout with a note, a files block and a sharing button that
    /// posts JSON to the mock server, and a capture of it.
    fn world(
        handler: impl Fn(&cargoframe_net::mock::Recorded) -> MockResponse + Send + Sync + 'static,
    ) -> World {
        let f = fixture();
        let server = MockServer::start(handler);
        save(
            &f.store,
            "api",
            json!({
                "name": "Hook", "kind": "custom", "base_url": server.base(),
                "auth": {"type": "bearer"},
                "send": {"method": "POST", "path": "/hook", "body": {"type": "json",
                    "template": "{\"summary\":\"{{summary}}\",\"build\":\"{{build}}\",\"kind\":\"{{kind}}\",\"prompt\":\"{{prompt}}\"}"}},
                "result_url_path": "url"
            }),
        );
        f.connections
            .set_secret(&f.store, "api", "secret", SECRET)
            .unwrap();

        let project = f._dir.path().join("Foo");
        fs::create_dir_all(project.join("bin")).unwrap();
        fs::write(project.join("run.log"), "LOGLINE").unwrap();
        fs::write(project.join("other.log"), "OTHER").unwrap();
        let exe = project.join("bin").join("Foo");

        let layout: Layout = serde_json::from_value(json!({
            "id": "", "name": "L", "blocks": [
                {"id": "note", "rect": {"x":0,"y":0,"w":6,"h":2}, "type": "text",
                 "title": "Note", "default_text": "hello"},
                {"id": "prompt", "rect": {"x":6,"y":0,"w":6,"h":2}, "type": "text",
                 "title": "Prompt", "role": "prompt", "default_text": ""},
                {"id": "logs", "rect": {"x":0,"y":2,"w":6,"h":2}, "type": "files",
                 "title": "Logs", "include": ["*.log"], "preselect": ["run.log"]},
                {"id": "send", "rect": {"x":6,"y":2,"w":6,"h":2}, "type": "sharing_button",
                 "label": "Send", "connection_id": "api", "prompt_block_id": "prompt",
                 "mappings": [
                    {"field": "summary", "source": {"kind": "block", "value": "note"}},
                    {"field": "build", "source": {"kind": "rule", "value": "version"}},
                    {"field": "kind", "source": {"kind": "fixed", "value": "bug in {{app.title}}"}}
                 ]}
            ]
        }))
        .unwrap();
        f.store.save_item(Kind::Layouts, "bug", &layout).unwrap();
        let trigger = Trigger {
            name: "Bug".into(),
            layout_id: Some("bug".into()),
            ..Trigger::default()
        };
        let captured = fake_capture(Some(&exe.display().to_string()));
        let mut session = prepare_with(
            &f.store,
            &f.connections,
            "bugtrig",
            &trigger,
            captured,
            NOW,
            7,
        )
        .unwrap();
        session
            .view
            .variables
            .insert("rule.version".into(), "1.2.3".into());
        session.png = PNG.to_vec();
        let out = tempfile::tempdir().unwrap();
        let ws = Workspace::new(out.path().join("captures"));
        World {
            f,
            server,
            session,
            out,
            ws,
            deliver: RecordingDeliver::default(),
        }
    }

    fn request() -> Request {
        Request {
            session_id: 7,
            button_id: "send".into(),
            texts: [
                ("note".to_string(), "it \"broke\"".to_string()),
                ("prompt".to_string(), "why?".to_string()),
            ]
            .into(),
            dropdowns: BTreeMap::new(),
            files: [("logs".to_string(), vec!["run.log".to_string()])].into(),
            image_edited: false,
        }
    }

    fn services<'a>(w: &'a World, cancelled: &'a dyn Fn() -> bool) -> Services<'a> {
        Services {
            store: &w.f.store,
            connections: &w.f.connections,
            browser: &NoBrowser,
            deliver: &w.deliver,
            workspace: &w.ws,
            cancelled,
            now: NOW,
        }
    }

    fn go(w: &World, req: &Request, picks: Vec<Option<PickedFolder>>) -> (ShareResult, u32) {
        let never = || false;
        let svc = services(w, &never);
        let queue = RefCell::new(picks.into_iter());
        let asked = RefCell::new(0u32);
        let mut picker = |_n: u32| {
            *asked.borrow_mut() += 1;
            queue.borrow_mut().next().flatten()
        };
        let mut sleep = |_d: Duration| {};
        let r = run(&svc, &mut picker, &mut sleep, &w.session, req);
        let n = *asked.borrow();
        (r, n)
    }

    fn set_safe(w: &World, folder: Option<&str>, retries: u32) {
        let s = Safe {
            folder: folder.map(String::from),
            max_retries: retries,
            ..Safe::default()
        };
        w.f.store.save(&s).unwrap();
    }

    fn zip_names(path: &str) -> Vec<String> {
        let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().unwrap().to_string())
            .collect()
    }

    fn zip_text(path: &str, name: &str) -> String {
        let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut s = String::new();
        z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    /// Point the test layout's button at a Claude connection.
    fn use_claude(w: &mut World, settings: serde_json::Value) {
        let mut conn = json!({"name": "Claude", "kind": "claude"});
        for (k, v) in settings.as_object().unwrap() {
            conn[k] = v.clone();
        }
        save(&w.f.store, "claude", conn);
        let layout = w.session.view.layout.as_mut().unwrap();
        for b in &mut layout.blocks {
            if let BlockKind::SharingButton(btn) = &mut b.kind {
                btn.connection_id = Some("claude".into());
                btn.mappings.clear();
            }
        }
    }

    #[test]
    fn a_share_to_claude_delivers_the_screenshot_the_files_and_the_message_in_one_go() {
        let mut w = world(|_| MockResponse::json(500, json!({})));
        use_claude(&mut w, json!({}));
        set_safe(&w, None, 0);
        let (r, _) = go(&w, &request(), vec![]);
        let ShareResult::Sent { summary, close, .. } = r else {
            panic!("{r:?}");
        };
        assert_eq!(summary, "Sent 2 files and the message to Claude");
        assert!(close);
        let seen = w.deliver.seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "one delivery, not a copy step per item");
        let s = &seen[0];
        assert_eq!(s.url, "https://claude.ai/new");
        assert!(s.submit);
        // The screenshot first, then the ticked file, each with the content it had.
        assert_eq!(
            s.files,
            vec![
                ("screenshot.png".to_string(), PNG.to_vec()),
                ("run.log".to_string(), b"LOGLINE".to_vec()),
            ]
        );
        assert!(s.text.starts_with("why?"), "{}", s.text);
        assert!(s.text.contains("Captured window: Editor"), "{}", s.text);
        assert!(s.text.contains("Attached files: run.log"), "{}", s.text);
        // Nothing went to the mock service: Claude is not a web request.
        assert!(w.server.requests_to("/hook").is_empty());
    }

    #[test]
    fn the_edited_picture_is_what_claude_gets() {
        let mut w = world(|_| MockResponse::json(500, json!({})));
        use_claude(&mut w, json!({}));
        set_safe(&w, None, 0);
        let edited = [0x89, b'P', b'N', b'G', 9, 9, 9, 9];
        w.session.set_edited(Some(edited.to_vec()));
        let mut req = request();
        req.image_edited = true;
        let (r, _) = go(&w, &req, vec![]);
        assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
        assert_eq!(
            w.deliver.seen.lock().unwrap()[0].files[0].1,
            edited.to_vec()
        );
    }

    #[test]
    fn the_capture_folder_goes_with_the_capture() {
        let mut w = world(|_| MockResponse::json(500, json!({})));
        use_claude(&mut w, json!({}));
        set_safe(&w, None, 0);
        assert!(
            w.session.dir_path().is_none(),
            "no folder until a destination needs files"
        );
        let (r, _) = go(&w, &request(), vec![]);
        assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
        let dir = w.session.dir_path().expect("the share made the folder");
        assert!(dir.join("screenshot.png").is_file());
        assert!(dir.join("files").join("run.log").is_file());
        // The window closes: the capture is dropped, and so is its folder.
        let World { session, ws, .. } = w;
        drop(session);
        assert!(!dir.exists(), "the folder outlived its capture");
        assert_eq!(ws.sweep(&[], Duration::ZERO).removed, 0);
    }

    #[test]
    fn claude_gets_the_files_as_they_were_when_it_was_shared_not_a_zip() {
        let mut w = world(|_| MockResponse::json(500, json!({})));
        use_claude(&mut w, json!({"send_automatically": false}));
        set_safe(&w, None, 0);
        let (r, _) = go(&w, &request(), vec![]);
        let ShareResult::Sent { summary, .. } = r else {
            panic!("{r:?}")
        };
        assert!(summary.starts_with("Pasted"), "{summary}");
        assert!(!w.deliver.seen.lock().unwrap()[0].submit);
    }

    #[test]
    fn claude_has_a_limit_on_files_per_message() {
        assert!(check_file_count(0).is_ok());
        assert!(check_file_count(MAX_FILES - 1).is_ok());
        let err = check_file_count(MAX_FILES + 2).unwrap_err();
        assert!(
            err.contains("at most 20") && err.contains("untick 3"),
            "{err}"
        );
    }

    #[test]
    fn a_share_sends_the_mapped_fields_the_prompt_and_the_secret_only_as_a_header() {
        let w = world(|_| MockResponse::json(201, json!({"url": "https://x.test/p/9"})));
        set_safe(&w, None, 0);
        let (r, asked) = go(&w, &request(), vec![]);
        let ShareResult::Sent {
            summary,
            url,
            close,
            ..
        } = r
        else {
            panic!("{r:?}");
        };
        assert!(summary.contains("201"), "{summary}");
        assert_eq!(url.as_deref(), Some("https://x.test/p/9"));
        assert!(close);
        assert_eq!(asked, 0);
        let sent = w.server.requests_to("/hook");
        assert_eq!(sent.len(), 1);
        let body = sent[0].json();
        assert_eq!(body["summary"], "it \"broke\"");
        assert_eq!(body["build"], "1.2.3");
        assert_eq!(body["kind"], "bug in Editor");
        assert_eq!(body["prompt"], "why?");
        assert_eq!(
            sent[0].header("authorization"),
            Some(format!("Bearer {SECRET}").as_str())
        );
        assert!(!String::from_utf8_lossy(&sent[0].body).contains(SECRET));
    }

    #[test]
    fn the_edited_picture_is_sent_only_when_the_window_says_it_was_edited() {
        let w = world(|_| MockResponse::json(200, json!({})));
        set_safe(&w, None, 0);
        // Switch the body to multipart so the picture is part of the request.
        save(
            &w.f.store,
            "api",
            json!({
                "name": "Hook", "kind": "custom", "base_url": w.server.base(),
                "auth": {"type": "bearer"},
                "send": {"method": "POST", "path": "/up", "body": {"type": "multipart",
                    "fields": [{"name": "title", "value": "{{title}}"}],
                    "image_field": "shot", "files_field": "zip"}}
            }),
        );
        w.f.connections.invalidate("api");
        let edited = [0x89, b'P', b'N', b'G', 9, 9, 9, 9];
        w.session.set_edited(Some(edited.to_vec()));

        let mut req = request();
        let (r, _) = go(&w, &req, vec![]);
        assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
        req.image_edited = true;
        let (r, _) = go(&w, &req, vec![]);
        assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");

        let sent = w.server.requests_to("/up");
        assert_eq!(sent.len(), 2);
        let has = |body: &[u8], needle: &[u8]| body.windows(needle.len()).any(|x| x == needle);
        assert!(has(&sent[0].body, PNG) && !has(&sent[0].body, &edited));
        assert!(has(&sent[1].body, &edited) && !has(&sent[1].body, PNG));
        // The ticked file travels as a zip that names it, with the title as a form field.
        assert!(has(&sent[0].body, b"filename=\"files.zip\""));
        assert!(has(&sent[0].body, b"run.log"));
        assert!(!has(&sent[0].body, b"other.log"));
        assert!(has(&sent[0].body, b"Editor"));
    }

    #[test]
    fn blocked_shares_say_why_and_send_nothing() {
        let w = world(|_| MockResponse::json(200, json!({})));
        set_safe(&w, None, 0);

        // a stale capture
        let mut req = request();
        req.session_id = 99;
        let (r, _) = go(&w, &req, vec![]);
        assert!(
            matches!(&r, ShareResult::Blocked { reasons } if reasons[0].contains("no longer open")),
            "{r:?}"
        );

        // a file the capture did not offer (other.log matches *.log but is only offered if
        // scanned; "../../etc/passwd" never is)
        let mut req = request();
        req.files
            .insert("logs".into(), vec!["../../etc/passwd".into()]);
        let (r, _) = go(&w, &req, vec![]);
        assert!(
            matches!(&r, ShareResult::Blocked { reasons } if reasons[0].contains("not one of the files")),
            "{r:?}"
        );

        // a button that is not in the layout
        let mut req = request();
        req.button_id = "nope".into();
        let (r, _) = go(&w, &req, vec![]);
        assert!(matches!(r, ShareResult::Blocked { .. }));

        // a required field with no value
        let t = cargoframe_config::InfoType {
            name: "Summary".into(),
            connection_id: "api".into(),
            field: "summary".into(),
            required: true,
            ..Default::default()
        };
        w.f.store.save_item(Kind::InfoTypes, "summary", &t).unwrap();
        let mut req = request();
        req.texts.insert("note".into(), "  ".into());
        let (r, _) = go(&w, &req, vec![]);
        assert!(
            matches!(&r, ShareResult::Blocked { reasons } if reasons[0].contains("Summary")),
            "{r:?}"
        );
        // and one that nothing maps
        let t = cargoframe_config::InfoType {
            name: "Owner".into(),
            connection_id: "api".into(),
            field: "owner".into(),
            required: true,
            ..Default::default()
        };
        w.f.store.save_item(Kind::InfoTypes, "owner", &t).unwrap();
        let (r, _) = go(&w, &request(), vec![]);
        assert!(
            matches!(&r, ShareResult::Blocked { reasons } if reasons.iter().any(|x| x.contains("Owner") && x.contains("map it"))),
            "{r:?}"
        );

        assert!(
            w.server.requests().is_empty(),
            "nothing may be sent when blocked"
        );
    }

    #[test]
    fn a_default_template_fills_an_empty_field() {
        let w = world(|_| MockResponse::json(200, json!({})));
        set_safe(&w, None, 0);
        let t = cargoframe_config::InfoType {
            name: "Summary".into(),
            connection_id: "api".into(),
            field: "summary".into(),
            default_template: "Problem in {{app.title}}".into(),
            ..Default::default()
        };
        w.f.store.save_item(Kind::InfoTypes, "summary", &t).unwrap();
        let mut req = request();
        req.texts.insert("note".into(), "".into());
        let (r, _) = go(&w, &req, vec![]);
        assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
        let body = w.server.requests_to("/hook")[0].json();
        assert_eq!(body["summary"], "Problem in Editor");
    }

    #[test]
    fn a_failed_share_retries_then_keeps_a_zip_in_the_safe_folder_without_secrets() {
        let w = world(|_| MockResponse::json(503, json!({"message": "down"})));
        let safe_dir = w.out.path().join("safe");
        fs::create_dir_all(&safe_dir).unwrap();
        set_safe(&w, Some(&safe_dir.display().to_string()), 2);
        let (r, asked) = go(&w, &request(), vec![]);
        let ShareResult::Failed {
            label,
            attempts,
            saved,
            reconnect,
            ..
        } = r
        else {
            panic!("{r:?}");
        };
        assert_eq!(label, "Service error");
        assert_eq!(attempts, 3, "one try and two retries");
        assert!(!reconnect);
        assert_eq!(asked, 0, "the safe folder worked, nobody is asked");
        assert_eq!(w.server.requests_to("/hook").len(), 3);

        let saved = saved.unwrap();
        assert_eq!(saved.place, "safe_folder");
        assert!(
            saved.path.starts_with(&safe_dir.display().to_string()),
            "{}",
            saved.path
        );
        assert!(
            saved.path.contains("CargoFrame_2026-10-23_080001_hook"),
            "{}",
            saved.path
        );
        let names = zip_names(&saved.path);
        for want in [
            "screenshot.png",
            "share.json",
            "error.json",
            "files/run.log",
            "manifest.json",
        ] {
            assert!(names.contains(&want.to_string()), "{names:?}");
        }
        assert!(!names.iter().any(|n| n.contains("other.log")));
        let share = zip_text(&saved.path, "share.json");
        assert!(
            share.contains("it \\\"broke\\\"") && share.contains("why?"),
            "{share}"
        );
        assert!(zip_text(&saved.path, "error.json").contains("Service error"));
        // The sign-in secret is nowhere in the kept copy, and no half-written file remains.
        let raw = fs::read(&saved.path).unwrap();
        assert!(!raw.windows(SECRET.len()).any(|x| x == SECRET.as_bytes()));
        let left: Vec<_> = fs::read_dir(&safe_dir).unwrap().flatten().collect();
        assert_eq!(left.len(), 1);
        // the temporary zip was cleaned up
        assert_eq!(
            fs::read_dir(w.f.store.dirs().data().join("tmp"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn a_rejected_sign_in_is_not_retried_and_offers_to_reconnect() {
        let w = world(|_| MockResponse::json(401, json!({"message": "bad token"})));
        let safe_dir = w.out.path().join("safe");
        fs::create_dir_all(&safe_dir).unwrap();
        set_safe(&w, Some(&safe_dir.display().to_string()), 3);
        let (r, _) = go(&w, &request(), vec![]);
        let ShareResult::Failed {
            reconnect,
            attempts,
            label,
            ..
        } = r
        else {
            panic!("{r:?}");
        };
        assert!(reconnect);
        assert_eq!(attempts, 1);
        assert_eq!(label, "Sign-in expired");
        let info = w.f.connections.info(&w.f.store, "api").unwrap();
        assert_eq!(info.state, cargoframe_layout::ConnectionState::Expired);
    }

    #[test]
    fn a_503_then_a_200_succeeds_after_one_retry() {
        let calls = std::sync::atomic::AtomicU32::new(0);
        let w = world(move |_| {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                MockResponse::new(503)
            } else {
                MockResponse::json(200, json!({}))
            }
        });
        set_safe(&w, None, 3);
        let (r, _) = go(&w, &request(), vec![]);
        assert!(matches!(r, ShareResult::Sent { .. }), "{r:?}");
        assert_eq!(w.server.requests_to("/hook").len(), 2);
    }

    #[test]
    fn with_no_safe_folder_the_person_is_asked_and_the_choice_can_be_remembered() {
        let w = world(|_| MockResponse::new(400));
        set_safe(&w, None, 0);
        let chosen = w.out.path().join("chosen");
        fs::create_dir_all(&chosen).unwrap();
        let pick = PickedFolder {
            dir: chosen.display().to_string(),
            remember: true,
        };
        let (r, asked) = go(&w, &request(), vec![Some(pick)]);
        let ShareResult::Failed { saved, label, .. } = r else {
            panic!("{r:?}");
        };
        assert_eq!(label, "Rejected by the service");
        assert_eq!(asked, 1);
        let saved = saved.unwrap();
        assert_eq!(saved.place, "picked_folder");
        assert!(Path::new(&saved.path).starts_with(&chosen));
        let safe = w.f.store.load_or_default::<Safe>().unwrap();
        assert_eq!(
            safe.folder.as_deref(),
            Some(chosen.display().to_string().as_str())
        );
    }

    #[test]
    fn when_nothing_else_works_the_zip_is_kept_in_the_data_folder() {
        let w = world(|_| MockResponse::new(400));
        let missing = w.out.path().join("not-there");
        set_safe(&w, Some(&missing.display().to_string()), 0);
        let (r, asked) = go(&w, &request(), vec![None, None, None]);
        let ShareResult::Failed { saved, .. } = r else {
            panic!("{r:?}");
        };
        assert!(asked >= 1);
        let saved = saved.unwrap();
        assert_eq!(saved.place, "kept_locally");
        assert!(Path::new(&saved.path).starts_with(w.f.store.dirs().data().join("kept")));
        assert!(saved.note.unwrap().contains("safe folder"));
        assert!(Path::new(&saved.path).is_file());
    }

    fn disconnect(w: &World) {
        w.f.connections
            .set_secret(&w.f.store, "api", "secret", "")
            .unwrap();
    }

    /// Every entry of the zip as text (the picture too, lossily): for "this appears nowhere".
    fn all_zip_text(path: &str) -> String {
        let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut out = String::new();
        for i in 0..z.len() {
            let mut bytes = Vec::new();
            z.by_index(i).unwrap().read_to_end(&mut bytes).unwrap();
            out.push_str(&String::from_utf8_lossy(&bytes));
            out.push('\n');
        }
        out
    }

    #[test]
    fn a_connection_that_is_not_signed_in_saves_a_zip_and_sends_nothing() {
        let w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        disconnect(&w);
        let (r, asked) = go(&w, &request(), vec![]);
        let ShareResult::SavedLocally { path, close, .. } = r else {
            panic!("{r:?}");
        };
        assert!(
            close,
            "the layout's close-after-sharing setting still applies"
        );
        assert_eq!(
            asked, 0,
            "the person is never asked where: the setting says"
        );
        assert!(Path::new(&path).starts_with(&dir), "{path}");
        assert!(path.ends_with("_local.zip"), "{path}");
        let mut names = zip_names(&path);
        names.sort();
        assert!(names.contains(&"screenshot.png".to_string()), "{names:?}");
        assert!(names.contains(&"share.json".to_string()));
        assert!(names.contains(&"files/run.log".to_string()));
        assert!(
            !names.contains(&"files/other.log".to_string()),
            "only ticked files"
        );
        assert!(!names.iter().any(|n| n == "error.json"), "nothing failed");
        let share: serde_json::Value =
            serde_json::from_str(&zip_text(&path, "share.json")).unwrap();
        assert_eq!(share["destination"], "local");
        assert_eq!(share["prompt"], "why?");
        assert_eq!(share["fields"][0]["value"], "it \"broke\"");
        assert!(
            w.server.requests().is_empty(),
            "nothing leaves the computer"
        );
        assert!(!all_zip_text(&path).contains(SECRET));
        // No leftovers next to the zip.
        let files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn a_button_with_no_connection_chosen_saves_a_zip_too() {
        let mut w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        if let Some(Some(BlockKind::SharingButton(b))) = w.session.view.layout.as_mut().map(|l| {
            l.blocks
                .iter_mut()
                .map(|b| &mut b.kind)
                .find(|k| matches!(k, BlockKind::SharingButton(_)))
        }) {
            b.connection_id = None;
        }
        let (r, _) = go(&w, &request(), vec![]);
        assert!(matches!(r, ShareResult::SavedLocally { .. }), "{r:?}");
        assert!(w.server.requests().is_empty());
    }

    #[test]
    fn saving_locally_needs_the_folder_for_saved_zips() {
        let w = world(|_| MockResponse::json(200, json!({})));
        disconnect(&w);
        for folder in [None, Some("   ")] {
            set_safe(&w, folder, 0);
            let (r, asked) = go(
                &w,
                &request(),
                vec![Some(PickedFolder {
                    dir: w.out.path().display().to_string(),
                    remember: true,
                })],
            );
            assert_eq!(
                r,
                ShareResult::Blocked {
                    reasons: vec![NEEDS_SAVE_FOLDER.to_string()]
                }
            );
            assert_eq!(asked, 0, "no folder picker for a local save");
        }
        assert!(!w.f.store.dirs().data().join("kept").exists());
        assert!(w.server.requests().is_empty());
    }

    #[test]
    fn a_folder_that_is_gone_is_reported_not_created_or_replaced() {
        let w = world(|_| MockResponse::json(200, json!({})));
        let missing = w.out.path().join("not-there");
        set_safe(&w, Some(&missing.display().to_string()), 0);
        disconnect(&w);
        let (r, _) = go(&w, &request(), vec![]);
        let ShareResult::Blocked { reasons } = r else {
            panic!("{r:?}");
        };
        assert!(
            reasons[0].contains("not-there") && reasons[0].contains("missing"),
            "{reasons:?}"
        );
        assert!(!missing.exists());
        assert!(!w.f.store.dirs().data().join("kept").exists());
    }

    /// Adds a Zip name block to the capture, holding `typed` (its value in the window).
    fn add_zip_name(w: &mut World, typed: &str) -> String {
        let layout = w.session.view.layout.as_mut().unwrap();
        layout.blocks.push(
            serde_json::from_value(json!({
                "id": "zipname", "rect": {"x":0,"y":4,"w":6,"h":1}, "type": "text",
                "title": "Zip name", "role": "zip_name", "default_text": ""
            }))
            .unwrap(),
        );
        typed.to_string()
    }

    fn request_with_zip_name(typed: &str) -> Request {
        let mut req = request();
        req.texts.insert("zipname".into(), typed.into());
        req
    }

    #[test]
    fn the_zip_name_block_names_a_zip_saved_locally() {
        let mut w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        disconnect(&w);
        let typed = add_zip_name(&mut w, "Bug in login");
        let (r, _) = go(&w, &request_with_zip_name(&typed), vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        assert_eq!(Path::new(&path), dir.join("Bug in login.zip"));
        // The same name twice keeps both.
        let (r, _) = go(&w, &request_with_zip_name(&typed), vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        assert_eq!(Path::new(&path), dir.join("Bug in login-2.zip"));
    }

    #[test]
    fn an_empty_or_unusable_zip_name_keeps_the_default_name() {
        let mut w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        disconnect(&w);
        add_zip_name(&mut w, "");
        for typed in ["", "   ", "..."] {
            let (r, _) = go(&w, &request_with_zip_name(typed), vec![]);
            let ShareResult::SavedLocally { path, .. } = r else {
                panic!("{r:?}");
            };
            assert!(
                path.contains("CargoFrame_2026-10-23_080001_local"),
                "{path}"
            );
        }
    }

    #[test]
    fn a_zip_name_cannot_write_outside_the_folder() {
        let mut w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        disconnect(&w);
        add_zip_name(&mut w, "");
        let (r, _) = go(&w, &request_with_zip_name("../../escape"), vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        assert_eq!(Path::new(&path).parent(), Some(dir.as_path()), "{path}");
        assert_eq!(Path::new(&path).file_name().unwrap(), "escape.zip");
    }

    #[test]
    fn only_the_first_zip_name_block_counts() {
        let mut w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        disconnect(&w);
        add_zip_name(&mut w, "");
        w.session.view.layout.as_mut().unwrap().blocks.push(
            serde_json::from_value(json!({
                "id": "zipname2", "rect": {"x":6,"y":4,"w":6,"h":1}, "type": "text",
                "title": "Other", "role": "zip_name", "default_text": ""
            }))
            .unwrap(),
        );
        let mut req = request_with_zip_name("first");
        req.texts.insert("zipname2".into(), "second".into());
        let (r, _) = go(&w, &req, vec![]);
        let ShareResult::SavedLocally { path, .. } = r else {
            panic!("{r:?}");
        };
        assert_eq!(Path::new(&path).file_name().unwrap(), "first.zip");
    }

    #[test]
    fn a_failed_share_keeps_its_copy_under_the_typed_zip_name() {
        let mut w = world(|_| MockResponse::json(503, json!({"message": "down"})));
        let safe_dir = w.out.path().join("safe");
        fs::create_dir_all(&safe_dir).unwrap();
        set_safe(&w, Some(&safe_dir.display().to_string()), 0);
        add_zip_name(&mut w, "");
        let (r, _) = go(&w, &request_with_zip_name("Crash on save"), vec![]);
        let ShareResult::Failed { saved, .. } = r else {
            panic!("{r:?}");
        };
        let saved = saved.unwrap();
        assert_eq!(saved.place, "safe_folder");
        assert_eq!(
            Path::new(&saved.path).file_name().unwrap(),
            "Crash on save.zip"
        );
    }

    #[test]
    fn two_local_saves_in_the_same_second_keep_both() {
        let w = world(|_| MockResponse::json(200, json!({})));
        let dir = w.out.path().join("zips");
        fs::create_dir_all(&dir).unwrap();
        set_safe(&w, Some(&dir.display().to_string()), 0);
        disconnect(&w);
        let (a, _) = go(&w, &request(), vec![]);
        let (b, _) = go(&w, &request(), vec![]);
        let (ShareResult::SavedLocally { path: a, .. }, ShareResult::SavedLocally { path: b, .. }) =
            (a, b)
        else {
            panic!("both saved");
        };
        assert_ne!(a, b);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
    }
}
