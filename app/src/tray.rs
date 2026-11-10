//! The tray icon and its menu.

use crate::{state::AppState, triggers, windowing};
use cargoframe_config::{General, Kind, Trigger};
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

/// The tray icon is drawn in two layers: the frames as they are, and the arrow, which takes the
/// accent colour chosen in Settings, General, Appearance (see `scripts/dev/render-icons.mjs`).
const FRAMES_PNG: &[u8] = include_bytes!("../icons/tray-frames.png");
const ARROW_PNG: &[u8] = include_bytes!("../icons/tray-arrow.png");
/// What the icon was before it had layers, kept for the case where a layer cannot be read.
const PLAIN_PNG: &[u8] = include_bytes!("../icons/32x32.png");

/// Lay the arrow, painted in `accent`, over the frames. Both are RGBA of the same size; the
/// arrow layer contributes only its transparency, its own colour is ignored.
fn with_accent(frames: &[u8], arrow: &[u8], accent: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frames.len());
    for (f, a) in frames.chunks_exact(4).zip(arrow.chunks_exact(4)) {
        let fa = f[3] as f32 / 255.0;
        let aa = a[3] as f32 / 255.0;
        let alpha = aa + fa * (1.0 - aa);
        for c in 0..3 {
            let v = if alpha <= 0.0 {
                0.0
            } else {
                (accent[c] as f32 * aa + f[c] as f32 * fa * (1.0 - aa)) / alpha
            };
            out.push(v.round().clamp(0.0, 255.0) as u8);
        }
        out.push((alpha * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    out
}

/// The tray icon with its arrow in `accent`.
fn icon_with_accent(accent: [u8; 3]) -> Image<'static> {
    let layers = Image::from_bytes(FRAMES_PNG)
        .and_then(|frames| Image::from_bytes(ARROW_PNG).map(|arrow| (frames, arrow)));
    if let Ok((frames, arrow)) = layers {
        if frames.width() == arrow.width() && frames.height() == arrow.height() {
            let rgba = with_accent(frames.rgba(), arrow.rgba(), accent);
            return Image::new_owned(rgba, frames.width(), frames.height());
        }
    }
    Image::from_bytes(PLAIN_PNG)
        .expect("the bundled tray icon is a PNG")
        .to_owned()
}

fn chosen_accent(app: &AppHandle) -> [u8; 3] {
    app.try_state::<AppState>()
        .and_then(|s| s.store.load_or_default::<General>().ok())
        .unwrap_or_default()
        .accent_rgb()
}

/// Redraw the tray icon after the accent colour changed.
pub fn refresh_icon(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(e) = tray.set_icon(Some(icon_with_accent(chosen_accent(app)))) {
            tracing::warn!("could not redraw the tray icon: {e}");
        }
    }
}

/// The menu: one "Capture" entry per enabled trigger, then the fixed entries.
/// A video trigger is recording the screen right now.
fn recording_by_trigger(app: &AppHandle) -> bool {
    app.try_state::<AppState>().is_some_and(|s| {
        s.recording.started_by_trigger()
            && matches!(
                s.recording.view(),
                crate::video::RecordingView::Recording { .. }
            )
    })
}

fn tooltip(recording: bool) -> &'static str {
    if recording {
        "CargoFrame is recording your screen. Press the video shortcut again to stop."
    } else {
        "CargoFrame"
    }
}

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
    let recording_now = recording_by_trigger(app);
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

/// Rebuild the menu after triggers change, and say in the tooltip when the screen is being recorded.
pub fn refresh_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip(recording_by_trigger(app))));
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
        .icon(icon_with_accent(chosen_accent(app)))
        .tooltip(tooltip(false))
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
                    drop(state.take_watch());
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

#[cfg(test)]
mod tests {
    use super::{icon_with_accent, tooltip, with_accent};

    #[test]
    fn the_tray_says_when_the_screen_is_being_recorded() {
        assert_eq!(tooltip(false), "CargoFrame");
        let t = tooltip(true);
        assert!(t.contains("recording your screen"), "{t}");
        assert!(t.contains("again to stop"), "{t}");
    }

    #[test]
    fn the_arrow_takes_the_accent_and_the_frames_stay_as_drawn() {
        // two pixels: one with only the frame (grey), one with the arrow over the frame
        let frames = [100, 100, 100, 255, 100, 100, 100, 255];
        let arrow = [0, 0, 0, 0, 0, 0, 0, 255];
        let out = with_accent(&frames, &arrow, [10, 200, 30]);
        assert_eq!(&out[..4], &[100, 100, 100, 255]);
        assert_eq!(&out[4..], &[10, 200, 30, 255]);
        // half transparent arrow over nothing keeps its colour and its transparency
        let out = with_accent(&[0, 0, 0, 0], &[0, 0, 0, 128], [10, 200, 30]);
        assert_eq!(&out[..3], &[10, 200, 30]);
        assert_eq!(out[3], 128);
    }

    #[test]
    fn the_bundled_layers_make_an_icon_whose_arrow_follows_the_accent() {
        let red = icon_with_accent([255, 0, 0]);
        let green = icon_with_accent([0, 255, 0]);
        assert_eq!((red.width(), red.height()), (32, 32));
        // pixels that differ between the two are the arrow, and they carry the accent
        let mut arrow_pixels = 0;
        for (r, g) in red.rgba().chunks_exact(4).zip(green.rgba().chunks_exact(4)) {
            if r != g {
                arrow_pixels += 1;
                assert!(r[0] > r[1] && g[1] > g[0], "{r:?} {g:?}");
            }
        }
        assert!(arrow_pixels >= 6, "the arrow has pixels: {arrow_pixels}");
        // the frames are not accent coloured: there are neutral pixels in both
        assert!(red
            .rgba()
            .chunks_exact(4)
            .any(|p| p[3] > 0 && p[0].abs_diff(p[1]) < 8));
    }
}
