//! Creating and focusing windows. Windows are created on demand, so a tray-only CargoFrame
//! holds no web view and uses little memory. What a closed window does next (destroyed at once,
//! kept hidden for five minutes, or kept for good) follows Settings, General, Memory.

use crate::state::AppState;
use crate::warm::{self, OnClose};
use cargoframe_config::General;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const SETTINGS: &str = "settings";
pub const CAPTURE: &str = "capture";

/// Open the Settings window, or bring it to the front if it is already open.
///
/// Safe to call from any thread, including tray and notification callbacks: the window is
/// built on a worker task, which avoids a known deadlock when building windows from the
/// event loop on Windows.
pub fn open_settings(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open_settings_now(&app) {
            tracing::error!("could not open settings: {e}");
        }
    });
}

fn shown(app: &AppHandle, label: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.warm.shown(label);
    }
}

fn open_settings_now(app: &AppHandle) -> tauri::Result<()> {
    shown(app, SETTINGS);
    if let Some(win) = app.get_webview_window(SETTINGS) {
        let _ = win.unminimize();
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("CargoFrame settings")
        .inner_size(1000.0, 720.0)
        .min_inner_size(760.0, 520.0)
        .center()
        .build()?;
    Ok(())
}

/// Show the CargoFrame window for the current capture. If it is already open it is reused:
/// the new capture replaces the old one without rebuilding the web view.
pub fn open_capture(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open_capture_now(&app) {
            tracing::error!("could not open the CargoFrame window: {e}");
        }
    });
}

fn open_capture_now(app: &AppHandle) -> tauri::Result<()> {
    shown(app, CAPTURE);
    if let Some(win) = app.get_webview_window(CAPTURE) {
        win.emit("session-changed", ())?;
        let _ = win.unminimize();
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::App("capture.html".into()))
        .title("CargoFrame")
        .inner_size(1120.0, 760.0)
        .min_inner_size(640.0, 420.0)
        .center()
        .focused(true)
        .build()?;
    Ok(())
}

/// Open Settings on a page, for the "Edit layout" button.
pub fn open_settings_at(app: &AppHandle, page: &str) {
    let app = app.clone();
    let page = page.to_string();
    tauri::async_runtime::spawn(async move {
        shown(&app, SETTINGS);
        if let Some(win) = app.get_webview_window(SETTINGS) {
            let _ = win.emit("navigate", page);
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
            return;
        }
        let url = format!("settings.html?page={page}");
        if let Err(e) = WebviewWindowBuilder::new(&app, SETTINGS, WebviewUrl::App(url.into()))
            .title("CargoFrame settings")
            .inner_size(1000.0, 720.0)
            .min_inner_size(760.0, 520.0)
            .center()
            .build()
        {
            tracing::error!("could not open settings: {e}");
        }
    });
}

/// A Settings or CargoFrame window is being closed. In Low memory it closes (and is destroyed);
/// otherwise it is hidden, and in Balanced a timer destroys it if nobody opens it again.
pub fn close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let label = window.label().to_string();
    if label != SETTINGS && label != CAPTURE {
        return;
    }
    let app = window.app_handle().clone();
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let mode = state
        .store
        .load_or_default::<General>()
        .map(|g| g.memory_mode)
        .unwrap_or_default();
    tracing::debug!(window = %label, ?mode, "window closing");
    let OnClose::Hide { release_after } = warm::on_close(mode) else {
        return;
    };
    api.prevent_close();
    if label == CAPTURE {
        // The screenshot is large and belongs to the capture that just ended.
        state.cancel_share();
        state.sessions.clear();
    }
    let _ = window.hide();
    let ticket = state.warm.hidden(&label);
    if let Some(after) = release_after {
        let spawned = std::thread::Builder::new()
            .name(format!("release-{label}"))
            .spawn(move || {
                std::thread::sleep(after);
                let Some(state) = app.try_state::<AppState>() else {
                    return;
                };
                if !state.warm.still_hidden(&label, ticket) {
                    return;
                }
                if let Some(win) = app.get_webview_window(&label) {
                    if !win.is_visible().unwrap_or(true) {
                        tracing::debug!(window = %label, "releasing a window that stayed hidden");
                        let _ = win.destroy();
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("could not schedule releasing a hidden window: {e}");
        }
    }
}
