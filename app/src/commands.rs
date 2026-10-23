//! Commands the web view can call. Keep this list in step with `build.rs` and
//! `capabilities/`.

use crate::state::AppState;
use crate::windowing;
use cargoframe_config::General;
use cargoframe_window::view::SessionView;
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
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
