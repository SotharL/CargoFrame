//! Creating and focusing windows. Windows are created on demand, so a tray-only CargoFrame
//! holds no web view and uses little memory. What a closed window does next (destroyed at once,
//! kept hidden for five minutes, or kept for good) follows Settings, General, Memory.

use crate::state::AppState;
use crate::warm::{self, OnClose};
use cargoframe_config::General;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const SETTINGS: &str = "settings";
/// The first CargoFrame window; more are `capture-2` and so on (see `session::window_label`).
pub const CAPTURE: &str = "capture";

pub use crate::session::is_capture_label;

/// The window `label` is closing: its capture is no longer open and its share, if it is sending,
/// stops retrying. The other windows are not touched.
pub fn forget_capture(state: &AppState, label: &str) {
    if let Some(session) = state.sessions.clear(label) {
        state.cancel_share(session.view.session_id);
    }
}

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

/// Start timing a window opening, noting whether the window is already there.
fn timing_begin(app: &AppHandle, flow: &str, label: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.timings.begin(flow);
        let warm = app.get_webview_window(label).is_some();
        state.timings.note(flow, if warm { "warm" } else { "cold" });
    }
}

fn timing_mark(app: &AppHandle, flow: &str, metric: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(ms) = state.timings.mark(flow, metric) {
            tracing::info!(ms, metric, "timing: window shown");
        }
    }
}

fn open_settings_now(app: &AppHandle) -> tauri::Result<()> {
    shown(app, SETTINGS);
    timing_begin(app, "settings", SETTINGS);
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

/// Show the CargoFrame window `label` for the capture it was given. If the window is already
/// there (kept warm) it is reused: the new capture replaces the old one without rebuilding the
/// web view.
pub fn open_capture(app: &AppHandle, label: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open_capture_now(&app, &label) {
            tracing::error!("could not open the CargoFrame window: {e}");
        }
    });
}

/// The window's usual width, for a layout of 12 columns.
const CAPTURE_WIDTH: f64 = 1120.0;

/// How wide the window for `label` is opened: wider for a layout with more columns, never wider
/// than the screen.
fn capture_width(app: &AppHandle, label: &str) -> f64 {
    let columns = app
        .try_state::<AppState>()
        .and_then(|s| s.sessions.for_label(label))
        .and_then(|s| s.view.layout.as_ref().map(|l| l.grid_columns()))
        .unwrap_or(cargoframe_layout::GRID_COLUMNS);
    let screen = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.size().to_logical::<f64>(m.scale_factor()).width - 40.0)
        .unwrap_or(f64::MAX);
    wanted_width(columns, screen)
}

/// `columns` as wide as the usual 12, within `screen`.
pub(crate) fn wanted_width(columns: u32, screen: f64) -> f64 {
    let w = CAPTURE_WIDTH * f64::from(columns) / f64::from(cargoframe_layout::GRID_COLUMNS);
    w.min(screen).max(CAPTURE_WIDTH.min(screen))
}

fn open_capture_now(app: &AppHandle, label: &str) -> tauri::Result<()> {
    shown(app, label);
    // The capture flow began at the shortcut; only say whether the window was ready.
    if let Some(state) = app.try_state::<AppState>() {
        let warm = app.get_webview_window(label).is_some();
        state
            .timings
            .note("capture", if warm { "warm" } else { "cold" });
    }
    if let Some(win) = app.get_webview_window(label) {
        // only this window reloads its capture; the others keep theirs
        app.emit_to(label.to_string(), "session-changed", ())?;
        // a layout with more columns wants a wider window than the one kept
        let want = capture_width(app, label);
        if let (Ok(size), Ok(scale)) = (win.inner_size(), win.scale_factor()) {
            let now = size.to_logical::<f64>(scale);
            if now.width + 1.0 < want {
                let _ = win.set_size(tauri::LogicalSize::new(want, now.height));
            }
        }
        let _ = win.unminimize();
        win.show()?;
        win.set_focus()?;
        timing_mark(app, "capture", "window");
        return Ok(());
    }
    let first = label == CAPTURE;
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("capture.html".into()))
        .title(if first {
            "CargoFrame".to_string()
        } else {
            format!("CargoFrame {}", label.trim_start_matches("capture-"))
        })
        .inner_size(capture_width(app, label), 760.0)
        .min_inner_size(640.0, 420.0)
        .focused(true);
    if first {
        builder = builder.center();
    }
    builder.build()?;
    timing_mark(app, "capture", "window");
    Ok(())
}

/// Open Settings on a page, for the "Edit layout" button.
pub fn open_settings_at(app: &AppHandle, page: &str) {
    let app = app.clone();
    let page = page.to_string();
    tauri::async_runtime::spawn(async move {
        shown(&app, SETTINGS);
        timing_begin(&app, "settings", SETTINGS);
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
    if label != SETTINGS && !is_capture_label(&label) {
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
    if label == SETTINGS {
        // A recording started from the Video page belongs to that window.
        state.recording.cancel();
    }
    tracing::debug!(window = %label, ?mode, "window closing");
    let OnClose::Hide { release_after } = warm::on_close(mode) else {
        return;
    };
    api.prevent_close();
    if is_capture_label(&label) {
        // The screenshot is large and belongs to the capture that just ended.
        forget_capture(&state, &label);
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

#[cfg(test)]
mod tests {
    use super::wanted_width;

    #[test]
    fn a_layout_with_more_columns_opens_a_wider_window_within_the_screen() {
        assert_eq!(wanted_width(12, 3000.0), 1120.0);
        assert_eq!(wanted_width(18, 3000.0), 1680.0);
        assert_eq!(wanted_width(24, 1880.0), 1880.0);
        // a small screen: no wider than it
        assert_eq!(wanted_width(12, 1000.0), 1000.0);
    }
}
