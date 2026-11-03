//! The tray icon and its menu.

use crate::{state::AppState, triggers, windowing};
use cargoframe_config::{Kind, Trigger};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

const ID_OPEN: &str = "open_settings";
const ID_PAUSE: &str = "pause";
const ID_QUIT: &str = "quit";
const ID_STOP_RECORDING: &str = "stop_recording";
const ID_CAPTURE_PREFIX: &str = "capture:";
const TRAY_ID: &str = "main";

/// The menu: one "Capture" entry per enabled trigger, then the fixed entries.
fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, ID_OPEN, "Open settings", true, None::<&str>)?;
    let paused = app
        .try_state::<AppState>()
        .map(|s| s.paused())
        .unwrap_or(false);
    let pause =
        CheckMenuItem::with_id(app, ID_PAUSE, "Pause triggers", true, paused, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit CargoFrame", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    // while a video trigger is recording, a way to throw the recording away
    let recording_now = app.try_state::<AppState>().is_some_and(|s| {
        s.recording.started_by_trigger()
            && matches!(
                s.recording.view(),
                crate::video::RecordingView::Recording { .. }
            )
    });
    let stop_recording = MenuItem::with_id(
        app,
        ID_STOP_RECORDING,
        "Recording: stop and discard it",
        true,
        None::<&str>,
    )?;
    let sep_recording = PredefinedMenuItem::separator(app)?;

    let mut captures = Vec::new();
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(loaded) = state.store.load_all::<Trigger>(Kind::Triggers) {
            for (id, t) in loaded.items.into_iter().filter(|(_, t)| t.enabled) {
                let name = if t.name.trim().is_empty() {
                    id.clone()
                } else {
                    t.name.clone()
                };
                let label = if t.shortcut.is_empty() {
                    format!("Capture: {name}")
                } else {
                    format!("Capture: {name}  ({})", t.shortcut)
                };
                captures.push(MenuItem::with_id(
                    app,
                    format!("{ID_CAPTURE_PREFIX}{id}"),
                    label,
                    true,
                    None::<&str>,
                )?);
            }
        }
    }
    let mut items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    if recording_now {
        items.push(&stop_recording);
        items.push(&sep_recording);
    }
    for c in &captures {
        items.push(c);
    }
    if !captures.is_empty() {
        items.push(&separator);
    }
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    items.push(&open);
    items.push(&sep2);
    items.push(&pause);
    items.push(&sep3);
    items.push(&quit);
    Menu::with_items(app, &items)
}

/// Rebuild the menu after triggers change.
pub fn refresh_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match menu(app) {
            Ok(m) => {
                let _ = tray.set_menu(Some(m));
            }
            Err(e) => tracing::warn!("could not rebuild the tray menu: {e}"),
        }
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let menu = menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
        .tooltip("CargoFrame")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            ID_OPEN => windowing::open_settings(app),
            ID_PAUSE => {
                if let Some(state) = app.try_state::<AppState>() {
                    let paused = !state.paused();
                    state.set_paused(paused);
                    tracing::info!(paused, "triggers paused toggled");
                }
            }
            ID_QUIT => app.exit(0),
            ID_STOP_RECORDING => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.recording.cancel();
                    tracing::info!("recording discarded from the tray");
                }
                refresh_menu(app);
            }
            other => {
                if let Some(trigger_id) = other.strip_prefix(ID_CAPTURE_PREFIX) {
                    triggers::run(app, trigger_id.to_string());
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            // A left click opens Settings; the menu stays on the right click.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                windowing::open_settings(tray.app_handle());
            }
        })
        .build(app)
}
