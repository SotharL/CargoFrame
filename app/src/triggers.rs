//! Triggers: global shortcuts that start a capture, plus the tray entries that do the same
//! (needed on Wayland, where programs cannot register global shortcuts).

use crate::state::AppState;
use crate::{notify, session, tray, windowing};
use cargoframe_config::{shortcut, Kind, Trigger};
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Register the shortcut of every enabled trigger, replacing what was registered before.
/// Call at start-up and whenever triggers change.
pub fn reload(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();

    let mut errors = BTreeMap::new();
    let loaded = match state.store.load_all::<Trigger>(Kind::Triggers) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("cannot read triggers: {e}");
            return;
        }
    };
    for e in &loaded.errors {
        tracing::warn!("skipping a damaged trigger file: {e}");
    }
    let mut claimed: BTreeMap<String, String> = BTreeMap::new();
    for (id, trigger) in &loaded.items {
        if !trigger.enabled || trigger.shortcut.trim().is_empty() {
            continue;
        }
        let canonical = match shortcut::normalize(&trigger.shortcut) {
            Ok(c) => c,
            Err(e) => {
                errors.insert(id.clone(), e.to_string());
                continue;
            }
        };
        if let Some(other) = claimed.get(&canonical) {
            errors.insert(
                id.clone(),
                format!("{canonical} is already used by \"{other}\""),
            );
            continue;
        }
        let parsed: Shortcut = match canonical.parse() {
            Ok(s) => s,
            Err(e) => {
                errors.insert(
                    id.clone(),
                    format!("{canonical} is not supported here: {e}"),
                );
                continue;
            }
        };
        let trigger_id = id.clone();
        let registered = shortcuts.on_shortcut(parsed, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                tracing::debug!(trigger = %trigger_id, "shortcut pressed");
                run(app, trigger_id.clone());
            }
        });
        match registered {
            Ok(()) => {
                tracing::info!(trigger = %id, shortcut = %canonical, "shortcut registered");
                claimed.insert(canonical, id.clone());
            }
            Err(e) => {
                tracing::warn!(trigger = %id, "shortcut not registered: {e}");
                errors.insert(id.clone(), e.to_string());
            }
        }
    }
    state.set_shortcut_errors(errors);
    tray::refresh_menu(app);
}

/// Copy the files ticked by default into the capture's temporary folder, off the thread that
/// opens the window. A share that starts while this runs waits for it (see `Session::with_dir`).
fn stage_files_in_background(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(session) = state.sessions.current() else {
        return;
    };
    if session.preselected().is_empty() {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("stage-files".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            let open = || {
                state
                    .sessions
                    .current()
                    .is_some_and(|c| std::sync::Arc::ptr_eq(&c, &session))
            };
            let staged = session.stage_preselected(
                &state.workspace,
                crate::share::MAX_STAGED_FILE_BYTES,
                crate::share::MAX_STAGED_TOTAL_BYTES,
                &open,
            );
            tracing::debug!(staged, "files copied for this capture");
        });
    if let Err(e) = spawned {
        tracing::warn!("could not start copying the files: {e}");
    }
}

/// Take a capture for `trigger_id` and open the CargoFrame window. Safe to call from any
/// thread; does nothing while paused or while another capture is running.
pub fn run(app: &AppHandle, trigger_id: String) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.paused() {
        tracing::debug!("triggers are paused");
        return;
    }
    if !state.try_begin_capture() {
        tracing::debug!("a capture is already running");
        return;
    }
    state.timings.begin("capture");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let worker = app.clone();
        let id = trigger_id.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let state = worker.state::<AppState>();
            let session_id = state.sessions.next_id();
            session::capture_and_prepare(&state.store, &state.connections, &id, session_id)
        })
        .await;
        let state = app.state::<AppState>();
        match result {
            Ok(Ok(s)) => {
                state.set_last_program(s.view.app.as_ref().and_then(|a| a.exe_path.clone()));
                state.sessions.set(s);
                stage_files_in_background(&app);
                if let Some(ms) = state.timings.mark("capture", "ready") {
                    tracing::info!(ready_ms = ms, "timing: capture ready");
                }
                windowing::open_capture(&app);
            }
            Ok(Err(e)) => {
                tracing::error!(trigger = %trigger_id, "capture failed: {e}");
                notify::problem(&app, "CargoFrame could not capture", &e.to_string());
            }
            Err(e) => tracing::error!("capture task failed: {e}"),
        }
        state.end_capture();
    });
}
