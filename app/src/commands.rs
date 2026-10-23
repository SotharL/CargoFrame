//! Commands the web view can call. Keep this list in step with `build.rs` and
//! `capabilities/`.

use crate::chats::{self, ChatView};
use crate::connections::{unix_now, ConnectionsView};
use crate::desktop::{openable, TauriBrowser, TauriClipboard};
use crate::items::{self, ItemKind, ItemList};
use crate::registry::AppRegistry;
use crate::session::{self, check_png};
use crate::share::{self, ShareResult};
use crate::state::AppState;
use crate::tools::{self, RuleTest, VariableInfo};
use crate::windowing;
use cargoframe_config::{General, Safe};
use cargoframe_connectors::{Describe, OptionItem};
use cargoframe_layout::{validate, BlockKind, DropdownSource, Layout};
use cargoframe_rules::Rule;
use cargoframe_safe::ladder::PickedFolder;
use cargoframe_window::view::{issue_views, IssueView, SessionView};
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_clipboard_manager::ClipboardExt as _;
use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt as _;

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    os: &'static str,
    config_dir: String,
    data_dir: String,
    tray_available: bool,
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        os: os_name(),
        config_dir: state.store.dirs().config().display().to_string(),
        data_dir: state.store.dirs().data().display().to_string(),
        tray_available: state.tray_available,
    }
}

#[tauri::command]
pub fn get_general(state: State<'_, AppState>) -> Result<General, String> {
    state
        .store
        .load_or_default::<General>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_general(state: State<'_, AppState>, general: General) -> Result<(), String> {
    state.store.save(&general).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable().map_err(|e| e.to_string())
    } else {
        launcher.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn open_config_folder(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.store.dirs().config();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

// ----- the CargoFrame window -----

/// What the window shows for the current capture.
#[tauri::command]
pub fn get_session(state: State<'_, AppState>) -> Result<SessionView, String> {
    state
        .sessions
        .current()
        .map(|s| s.view.clone())
        .ok_or_else(|| "There is no capture to show".to_string())
}

/// The screenshot as PNG bytes (sent raw, not as JSON numbers).
#[tauri::command]
pub fn get_session_image(state: State<'_, AppState>) -> Result<Response, String> {
    state
        .sessions
        .current()
        .map(|s| Response::new(s.png.clone()))
        .ok_or_else(|| "There is no capture to show".to_string())
}

#[tauri::command]
pub fn close_capture(app: AppHandle) {
    if let Some(win) = app.get_webview_window(windowing::CAPTURE) {
        let _ = win.close();
    }
}

/// "Edit layout": open Settings on the layouts page.
#[tauri::command]
pub fn edit_layout(app: AppHandle, layout_id: Option<String>) {
    let page = match layout_id {
        Some(id) if cargoframe_config::valid_id(&id) => format!("layouts/{id}"),
        _ => "layouts".to_string(),
    };
    windowing::open_settings_at(&app, &page);
}

/// Open Settings on one of its pages (for example `connections`, to reconnect).
#[tauri::command]
pub fn open_settings(app: AppHandle, page: String) {
    const PAGES: [&str; 7] = [
        "triggers",
        "connections",
        "rules",
        "infotypes",
        "layouts",
        "safe",
        "general",
    ];
    if PAGES.contains(&page.as_str()) {
        windowing::open_settings_at(&app, &page);
    } else {
        windowing::open_settings(&app);
    }
}

// ----- triggers -----

/// Shortcuts that could not be registered, by trigger id, with the reason.
#[tauri::command]
pub fn get_shortcut_errors(
    state: State<'_, AppState>,
) -> std::collections::BTreeMap<String, String> {
    state.shortcut_errors()
}

/// Call after triggers are saved or deleted: re-registers the shortcuts and the tray entries.
#[tauri::command]
pub fn reload_triggers(app: AppHandle) {
    crate::triggers::reload(&app);
}

// ----- items: triggers, layouts, data rules, info types -----

#[tauri::command]
pub fn list_items(state: State<'_, AppState>, kind: ItemKind) -> Result<ItemList, String> {
    items::list(&state.store, kind)
}

#[tauri::command]
pub fn save_item(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ItemKind,
    id: String,
    item: serde_json::Value,
) -> Result<(), String> {
    items::save(&state.store, kind, &id, item)?;
    match kind {
        ItemKind::Trigger => crate::triggers::reload(&app),
        // Settings (an address, a project) may have changed: forget what was cached.
        ItemKind::Connection => state.connections.invalidate(&id),
        _ => {}
    }
    Ok(())
}

#[tauri::command]
pub fn delete_item(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ItemKind,
    id: String,
) -> Result<(), String> {
    if kind == ItemKind::Connection {
        // Take its passwords and tokens out of the credential store along with it.
        if let Ok(c) = state.connections.load(&state.store, &id) {
            state.connections.forget_secrets(&id, c.settings.kind());
        }
    }
    items::delete(&state.store, kind, &id)?;
    if kind == ItemKind::Trigger {
        crate::triggers::reload(&app);
    }
    Ok(())
}

#[tauri::command]
pub fn new_item_id(
    state: State<'_, AppState>,
    kind: ItemKind,
    name: String,
) -> Result<String, String> {
    items::new_id(&state.store, kind, &name)
}

/// The builder's status chips for a layout that may not be saved yet.
#[tauri::command]
pub fn validate_layout(state: State<'_, AppState>, layout: Layout) -> Vec<IssueView> {
    let registry = AppRegistry::from_store(&state.store, &state.connections);
    issue_views(&validate(&layout, &registry))
}

/// Built-in variables and one per data rule, for the variable picker.
#[tauri::command]
pub fn list_variables(state: State<'_, AppState>) -> Vec<VariableInfo> {
    tools::variables(&state.store)
}

// ----- data rules -----

/// Try a rule. Without a folder, the project folder of the last capture is used.
#[tauri::command]
pub async fn test_rule(
    state: State<'_, AppState>,
    rule: Rule,
    folder: Option<String>,
) -> Result<RuleTest, String> {
    let folder = folder.filter(|f| !f.trim().is_empty()).or_else(|| {
        state
            .sessions
            .current()
            .and_then(|s| s.view.project.as_ref().map(|p| p.folder.clone()))
    });
    Ok(tools::test_rule(&rule, folder.as_deref()))
}

// ----- safe settings -----

#[tauri::command]
pub fn get_safe(state: State<'_, AppState>) -> Result<Safe, String> {
    state
        .store
        .load_or_default::<Safe>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_safe(state: State<'_, AppState>, mut safe: Safe) -> Result<(), String> {
    if !(5..=300).contains(&safe.timeout_secs) {
        return Err("The timeout must be between 5 and 300 seconds".into());
    }
    if safe.max_retries > 10 {
        return Err("At most 10 retries".into());
    }
    safe.folder = safe
        .folder
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty());
    state.store.save(&safe).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_safe_folder(path: String) -> Result<(), String> {
    tools::test_folder(&path)
}

/// The system folder picker. `None` when the person cancels.
#[tauri::command]
pub async fn pick_folder(app: AppHandle, title: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file();
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

// ----- connections (Settings) -----

/// Run something that may wait on the credential store or the network off the UI thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("The task stopped unexpectedly: {e}"))
}

/// Every connection with its state, what it can fill, and whether its secrets are stored.
#[tauri::command]
pub async fn list_connections(app: AppHandle) -> Result<ConnectionsView, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.connections.view(&state.store)
    })
    .await
}

/// Store (or, with an empty value, remove) a password or key the person typed. The value is
/// never read back to the window.
#[tauri::command]
pub async fn set_connection_secret(
    app: AppHandle,
    id: String,
    name: String,
    value: String,
) -> Result<(), String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state
            .connections
            .set_secret(&state.store, &id, &name, &value)
    })
    .await?
}

/// Sign in. Opens the browser for services that use it and waits until the person is done.
#[tauri::command]
pub async fn connect_connection(app: AppHandle, id: String) -> Result<String, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.connections.connect(
            &state.store,
            &id,
            &TauriBrowser(app.clone()),
            &TauriClipboard(app.clone()),
        )
    })
    .await?
}

#[tauri::command]
pub fn cancel_sign_in(state: State<'_, AppState>) {
    state.connections.cancel_sign_in();
}

#[tauri::command]
pub async fn disconnect_connection(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.connections.disconnect(&state.store, &id)
    })
    .await?
}

/// Ask the service who we are.
#[tauri::command]
pub async fn test_connection(app: AppHandle, id: String) -> Result<String, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.connections.test(&state.store, &id)
    })
    .await?
}

#[tauri::command]
pub fn describe_connection(state: State<'_, AppState>, id: String) -> Result<Describe, String> {
    state.connections.describe(&state.store, &id)
}

#[derive(Serialize)]
pub struct OptionsError {
    message: String,
    /// The sign-in is no longer accepted.
    reconnect: bool,
}

/// A list from a service, for the Settings pages (the info type editor).
#[tauri::command]
pub async fn preview_options(
    app: AppHandle,
    id: String,
    list: String,
) -> Result<Vec<OptionItem>, OptionsError> {
    fetch(app, id, list).await
}

async fn fetch(app: AppHandle, id: String, list: String) -> Result<Vec<OptionItem>, OptionsError> {
    let result = blocking(move || {
        let state = app.state::<AppState>();
        state.connections.options(&state.store, &id, &list)
    })
    .await
    .map_err(|message| OptionsError {
        message,
        reconnect: false,
    })?;
    result.map_err(|e| OptionsError {
        reconnect: e.kind.needs_reconnect(),
        message: e.message,
    })
}

// ----- Claude chats (Settings) -----

#[tauri::command]
pub fn list_chats(state: State<'_, AppState>) -> Result<Vec<ChatView>, String> {
    chats::list(&state.store)
}

#[tauri::command]
pub fn pin_chat(state: State<'_, AppState>, link: String, title: String) -> Result<(), String> {
    chats::pin_link(&state.store, &link, &title, unix_now())
}

#[tauri::command]
pub fn set_chat_pinned(state: State<'_, AppState>, id: String, pinned: bool) -> Result<(), String> {
    chats::set_pinned(&state.store, &id, pinned)
}

#[tauri::command]
pub fn remove_chat(state: State<'_, AppState>, id: String) -> Result<(), String> {
    chats::remove(&state.store, &id)
}

// ----- sharing (the CargoFrame window) -----

/// A dropdown list for a block of the open capture. Only the lists the layout itself asks for
/// can be fetched from here.
#[tauri::command]
pub async fn fetch_options(
    app: AppHandle,
    session_id: u64,
    block_id: String,
) -> Result<Vec<OptionItem>, OptionsError> {
    let asked = {
        let state = app.state::<AppState>();
        state
            .sessions
            .current()
            .filter(|s| s.view.session_id == session_id)
            .and_then(|s| {
                let layout = s.view.layout.as_ref()?;
                match &layout.block(&block_id)?.kind {
                    BlockKind::Dropdown(d) => match &d.source {
                        DropdownSource::WebApi {
                            connection_id,
                            options_id,
                        } => Some((connection_id.clone(), options_id.clone())),
                        DropdownSource::ClaudeChats => None,
                    },
                    _ => None,
                }
            })
    };
    let Some((connection_id, options_id)) = asked else {
        return Err(OptionsError {
            message: "That dropdown has no list to load".into(),
            reconnect: false,
        });
    };
    fetch(app, connection_id, options_id).await
}

/// The picture as edited (crop, drawings), sent just before a share. An empty body clears it.
/// The capture is named in the `x-session-id` header so a late call cannot reach a newer one.
#[tauri::command]
pub fn set_edited_image(state: State<'_, AppState>, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected a picture".into());
    };
    let id: u64 = request
        .headers()
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or("The capture is not named")?;
    let session = state
        .sessions
        .current()
        .filter(|s| s.view.session_id == id)
        .ok_or("This capture is no longer open")?;
    if bytes.is_empty() {
        session.set_edited(None);
        return Ok(());
    }
    check_png(bytes)?;
    session.set_edited(Some(bytes.clone()));
    Ok(())
}

/// Ask where to keep a copy when a share could not be delivered.
fn pick_fallback_folder(app: &AppHandle, attempt: u32) -> Option<PickedFolder> {
    let title = if attempt <= 1 {
        "CargoFrame could not send. Choose a folder to keep a copy in"
    } else {
        "That folder did not work. Choose another folder for the copy"
    };
    let dir = app
        .dialog()
        .file()
        .set_title(title)
        .blocking_pick_folder()?
        .into_path()
        .ok()?
        .display()
        .to_string();
    let remember = app
        .dialog()
        .message("Use this folder for future copies too?")
        .title("CargoFrame")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::YesNo)
        .blocking_show();
    Some(PickedFolder { dir, remember })
}

/// A window says it has painted its first usable content.
#[tauri::command]
pub fn report_painted(state: State<'_, AppState>, flow: String) {
    let metric = match flow.as_str() {
        "capture" => "paint",
        "settings" => "settings_paint",
        _ => return,
    };
    if let Some(ms) = state.timings.finish(&flow, metric) {
        tracing::info!(ms, metric = %metric, "timing: usable");
    }
}

/// The last timings, for Settings, General.
#[tauri::command]
pub fn get_timings(state: State<'_, AppState>) -> Vec<crate::timing::TimingView> {
    state.timings.snapshot()
}

/// Press a sharing button.
#[tauri::command]
pub async fn share(app: AppHandle, request: share::Request) -> Result<ShareResult, String> {
    {
        let state = app.state::<AppState>();
        if !state.try_begin_share() {
            return Ok(ShareResult::Blocked {
                reasons: vec!["A share is already being sent".into()],
            });
        }
    }
    let worker = app.clone();
    let started = std::time::Instant::now();
    let result = blocking(move || {
        let app = worker;
        let state = app.state::<AppState>();
        let Some(session) = state.sessions.current() else {
            return ShareResult::Blocked {
                reasons: vec!["This capture is no longer open".into()],
            };
        };
        let cancelled = || state.share_cancelled();
        let browser = TauriBrowser(app.clone());
        let clipboard = TauriClipboard(app.clone());
        let svc = share::Services {
            store: &state.store,
            connections: &state.connections,
            browser: &browser,
            clipboard: &clipboard,
            cancelled: &cancelled,
            now: session::now(),
        };
        let mut picker = |attempt: u32| pick_fallback_folder(&app, attempt);
        let mut sleep = share::interruptible_sleep(&cancelled);
        share::run(&svc, &mut picker, &mut sleep, &session, &request)
    })
    .await;
    let state = app.state::<AppState>();
    state.end_share();
    let result = result?;
    if matches!(
        result,
        ShareResult::Sent { .. } | ShareResult::Failed { .. }
    ) {
        state
            .timings
            .record("share", started.elapsed().as_millis() as u64);
    }
    // The window was closed while sending: nobody is watching, so say how it ended.
    if state.share_cancelled() {
        match &result {
            ShareResult::Sent { summary, .. } => crate::notify::problem(&app, "Shared", summary),
            ShareResult::Failed { label, saved, .. } => {
                let kept = saved
                    .as_ref()
                    .map(|s| format!(" A copy is in {}", s.path))
                    .unwrap_or_default();
                crate::notify::problem(
                    &app,
                    "CargoFrame could not share",
                    &format!("{label}.{kept}"),
                );
            }
            ShareResult::Blocked { .. } => {}
        }
    }
    Ok(result)
}

/// Copy text to the clipboard ("Copy prompt" after a hand-off).
#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    if text.len() > 1_000_000 {
        return Err("That text is too long to copy".into());
    }
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Open a link from a result (the created issue, the post) in the browser.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    if !openable(&url) {
        return Err("Only web links can be opened".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
