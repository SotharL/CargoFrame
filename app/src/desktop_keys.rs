//! Shortcuts on a Wayland desktop: handed to the desktop, which runs `cargoframe open --trigger
//! <id>` when one is pressed (see `cargoframe_shortcuts` for each desktop's way).

use cargoframe_shortcuts::{Binding, Route, Status};
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Whether this session is a Wayland one, where the shortcut plugin cannot hear keys.
pub fn wayland() -> bool {
    #[cfg(target_os = "linux")]
    {
        cargoframe_capture::session::detect_env() == cargoframe_capture::session::Session::Wayland
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// What was handed to the desktop last time, so a change can take it back.
#[derive(Default)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct DesktopKeys {
    inner: Mutex<Inner>,
}

#[derive(Default)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct Inner {
    route: Option<Route>,
    /// sway and Hyprland: the keys bound last time.
    bound: Vec<String>,
    #[cfg(target_os = "linux")]
    portal: Option<Arc<cargoframe_shortcuts::portal::Portal>>,
    /// Bumped at each change, so a slow portal answer for an old list is dropped.
    generation: u64,
}

/// The program a desktop shortcut starts.
fn program() -> String {
    let exe = std::env::current_exe().unwrap_or_else(|_| "cargoframe".into());
    cargoframe_shortcuts::program(std::env::var("APPIMAGE").ok().as_deref(), &exe)
}

/// Hand `wanted` to the desktop. `errors` holds the triggers refused before (a shortcut that
/// cannot be read, one used twice); the desktop's own refusals are added to them.
pub fn apply(app: &AppHandle, wanted: Vec<Binding>, errors: BTreeMap<String, String>) {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return;
    };
    let program = program();
    let commands: BTreeMap<String, String> = wanted
        .iter()
        .map(|b| {
            (
                b.id.clone(),
                cargoframe_shortcuts::command_line(&program, &b.id),
            )
        })
        .collect();
    let with_commands: Vec<(Binding, String)> = wanted
        .iter()
        .map(|b| (b.clone(), commands[&b.id].clone()))
        .collect();

    #[cfg(target_os = "linux")]
    {
        use cargoframe_capture::wayland::Compositor;
        use cargoframe_shortcuts::{gnome, portal, wlroots, Abilities};
        let desktop = Compositor::detect_env();
        let keys = &state.desktop_keys;
        let mut inner = keys.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.generation += 1;
        let route = match inner.route {
            Some(r) => r,
            None => {
                let can = Abilities {
                    gnome_settings: matches!(desktop, Compositor::Gnome)
                        && gnome::Gsettings::available(),
                    portal: !desktop.is_wlroots() && portal::available(),
                };
                let r = cargoframe_shortcuts::choose(&desktop, &can);
                tracing::info!(
                    ?r,
                    desktop = desktop.name(),
                    "Wayland shortcuts go through the desktop"
                );
                inner.route = Some(r);
                r
            }
        };
        let mut status = Status {
            route: Some(route),
            note: route.note().map(str::to_string),
            errors,
            commands,
        };
        match (route, &desktop) {
            (Route::Sway, Compositor::Sway { socket }) => {
                let (bound, refused) = wlroots::sway_apply(socket, &inner.bound, &with_commands);
                inner.bound = bound;
                status.errors.extend(refused);
            }
            (Route::Hyprland, Compositor::Hyprland { socket }) => {
                let (bound, refused) = wlroots::hypr_apply(socket, &inner.bound, &with_commands);
                inner.bound = bound;
                status.errors.extend(refused);
            }
            (Route::Gnome, _) => {
                status
                    .errors
                    .extend(gnome::sync(&gnome::Gsettings, &with_commands));
            }
            (Route::Portal, _) => {
                // The desktop may show a dialog and wait for the person: never on this thread.
                if inner.portal.is_none() {
                    let handle = app.clone();
                    match portal::Portal::start(move |id| {
                        tracing::debug!(trigger = %id, "portal shortcut pressed");
                        crate::triggers::run(&handle, id);
                    }) {
                        Ok(p) => inner.portal = Some(Arc::new(p)),
                        Err(e) => tracing::warn!("the shortcut portal could not be reached: {e}"),
                    }
                }
                match inner.portal.clone() {
                    Some(p) => {
                        let generation = inner.generation;
                        let handle = app.clone();
                        let asked = wanted.clone();
                        let spawned = std::thread::Builder::new()
                            .name("bind-shortcuts".into())
                            .spawn(move || {
                                let outcome = p.bind(&asked);
                                let state = handle.state::<crate::state::AppState>();
                                let current = state
                                    .desktop_keys
                                    .inner
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .generation;
                                if current != generation {
                                    return;
                                }
                                state.update_shortcut_status(|s| match outcome {
                                    Ok(bound) => {
                                        for b in &asked {
                                            if !bound.contains_key(&b.id) {
                                                s.errors.insert(
                                                    b.id.clone(),
                                                    "The desktop did not take this shortcut".into(),
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        for b in &asked {
                                            s.errors.insert(
                                                b.id.clone(),
                                                format!("The desktop's shortcut service said: {e}"),
                                            );
                                        }
                                    }
                                });
                            });
                        if let Err(e) = spawned {
                            tracing::warn!("could not start binding the shortcuts: {e}");
                        }
                    }
                    None => {
                        for b in &wanted {
                            status.errors.insert(
                                b.id.clone(),
                                "The desktop's shortcut service could not be reached: add the command to your desktop's shortcuts".into(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        drop(inner);
        state.set_shortcut_status(status);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = with_commands;
        state.set_shortcut_status(Status {
            route: None,
            note: None,
            errors,
            commands,
        });
    }
}
