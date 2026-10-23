//! Creating and focusing windows. Windows are created on demand and destroyed when closed,
//! so a tray-only CargoFrame holds no web view and uses little memory.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const SETTINGS: &str = "settings";

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

fn open_settings_now(app: &AppHandle) -> tauri::Result<()> {
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
