//! The tray icon and its menu.

use crate::{state::AppState, windowing};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

const ID_OPEN: &str = "open_settings";
const ID_PAUSE: &str = "pause";
const ID_QUIT: &str = "quit";

pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let open = MenuItem::with_id(app, ID_OPEN, "Open settings", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, ID_PAUSE, "Pause triggers", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit CargoFrame", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &pause,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("main")
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
            _ => {}
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
