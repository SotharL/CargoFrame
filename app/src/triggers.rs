//! Triggers: global shortcuts that start a capture, plus the tray entries that do the same
//! (needed on Wayland, where programs cannot register global shortcuts).

use crate::state::AppState;
use crate::video::{self, Step};
use crate::{notify, session, tray, windowing};
use cargoframe_config::{shortcut, Kind, Media, Trigger, Video};
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
fn stage_files_in_background(app: &AppHandle, session: std::sync::Arc<session::Session>) {
    if session.preselected().is_empty() {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("stage-files".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            let open = || {
                // the window may have been closed while the files were being copied
                state
                    .sessions
                    .by_id(session.view.session_id)
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

/// Tell the person every CargoFrame window is in use. True when there is no room.
fn no_room(app: &AppHandle, state: &AppState) -> bool {
    if !state.sessions.is_full() {
        return false;
    }
    notify::problem(
        app,
        &format!("CargoFrame has {} windows open", session::MAX_WINDOWS),
        "Close one of them, then press the shortcut again.",
    );
    true
}

/// Give a finished capture a window of its own and open it. The windows already open keep
/// their captures: up to ten can be open together.
fn show(app: &AppHandle, state: &AppState, s: session::Session) {
    match state.sessions.place(s) {
        Ok((label, session)) => {
            stage_files_in_background(app, session);
            if let Some(ms) = state.timings.mark("capture", "ready") {
                tracing::info!(ready_ms = ms, "timing: capture ready");
            }
            windowing::open_capture(app, label);
        }
        Err(session::AllWindowsOpen) => {
            // a window was opened while this capture was being taken
            no_room(app, state);
        }
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
    // A video trigger starts a recording, and stops it the next time.
    let makes_video = state
        .store
        .load_item::<Trigger>(Kind::Triggers, &trigger_id)
        .ok()
        .flatten()
        .is_some_and(|t| t.media == Media::Video);
    if makes_video {
        press_video(app, trigger_id);
        return;
    }
    if no_room(app, &state) {
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
                show(&app, &state, s);
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

/// A press of a video trigger: the first starts recording, the next stops it and opens the
/// window with the clip. Runs off the thread that got the key, because stopping waits for ffmpeg.
fn press_video(app: &AppHandle, trigger_id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let worker = app.clone();
        let done =
            tauri::async_runtime::spawn_blocking(move || video_step(&worker, &trigger_id)).await;
        if let Err(e) = done {
            tracing::error!("video trigger task failed: {e}");
        }
    });
}

fn video_step(app: &AppHandle, trigger_id: &str) {
    let state = app.state::<AppState>();
    match video::step(
        &state.recording.view(),
        state.recording.started_by_trigger(),
    ) {
        Step::Refuse(why) => notify::problem(app, "CargoFrame", why),
        Step::Report(why) => {
            // hand the failure over (this also clears it), so the next press starts again
            let _ = state.recording.stop();
            tray::refresh_menu(app);
            notify::problem(app, "The recording stopped by itself", &why);
        }
        // a recording is not started, or finished, when there would be no window to show it in
        Step::Start | Step::Stop if no_room(app, &state) => {}
        Step::Start => start_recording(app, &state, trigger_id),
        Step::Stop => stop_recording(app, &state, trigger_id),
    }
}

/// The window in focus: where it is, as a part of the desktop to record, and which program it is.
fn focus() -> Option<video::Focus> {
    let app = cargoframe_capture::focused_app().ok().flatten()?;
    let b = app.bounds;
    Some(video::Focus {
        area: cargoframe_video::recorder::Area {
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
        },
        program: app.pid,
    })
}

fn start_recording(app: &AppHandle, state: &AppState, trigger_id: &str) {
    let settings = state
        .store
        .load_or_default::<Video>()
        .map(video::normalized)
        .unwrap_or_default();
    let work = crate::video::work_dir(&state.store);
    // the layout this trigger opens says what to record: the screen or the window in focus, and
    // which sounds go in
    let layout = state
        .store
        .load_item::<Trigger>(Kind::Triggers, trigger_id)
        .ok()
        .flatten()
        .and_then(|t| t.layout_id)
        .and_then(|id| {
            state
                .store
                .load_item::<cargoframe_layout::Layout>(Kind::Layouts, &id)
                .ok()
                .flatten()
        });
    let (wish, mut notes) = video::wish_for(layout.as_ref(), &focus);
    let window = wish.area.is_some();
    let started = state.recording.start_for_trigger(
        &settings,
        video::Os::current(),
        &video::Session::current(),
        &work,
        settings.window_secs,
        &wish,
    );
    match started {
        Ok(s) => {
            tracing::info!(window, sound = s.sound, "recording started by a trigger");
            notes.extend(s.notes);
            let what = format!(
                "your {}{}",
                if window { "window" } else { "screen" },
                if s.sound { ", with sound" } else { "" }
            );
            let mut body = format!(
                "Press the shortcut again to stop. Only the last {} seconds are kept.",
                settings.window_secs
            );
            for n in &notes {
                body.push(' ');
                body.push_str(n);
            }
            notify::info(app, &format!("CargoFrame is recording {what}"), &body);
            tray::refresh_menu(app);
        }
        Err(e) => {
            tracing::error!("recording could not start: {e}");
            notify::problem(app, "CargoFrame could not start recording", &e);
        }
    }
}

fn stop_recording(app: &AppHandle, state: &AppState, trigger_id: &str) {
    if !state.try_begin_capture() {
        tracing::debug!("a capture is already running");
        return;
    }
    let trial = state.recording.stop();
    tray::refresh_menu(app);
    let made = session_from_recording(state, trigger_id, trial);
    state.end_capture();
    match made {
        Ok(s) => {
            state.set_last_program(s.view.app.as_ref().and_then(|a| a.exe_path.clone()));
            show(app, state, s);
        }
        Err(e) => {
            tracing::error!(trigger = %trigger_id, "video capture failed: {e}");
            notify::problem(app, "CargoFrame could not finish the recording", &e);
        }
    }
}

/// The capture the window shows, from a recording that has just been stopped.
fn session_from_recording(
    state: &AppState,
    trigger_id: &str,
    trial: video::Trial,
) -> Result<session::Session, String> {
    if !trial.ok {
        return Err(trial.message);
    }
    let clip = trial
        .clip
        .map(std::path::PathBuf::from)
        .ok_or("The recording left no clip")?;
    let trigger = state
        .store
        .load_item::<Trigger>(Kind::Triggers, trigger_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("trigger \"{trigger_id}\" does not exist"))?;
    let captured = session::capture_for_video(&trigger).map_err(|e| e.to_string())?;
    session::prepare_video(
        &state.store,
        &state.connections,
        &state.workspace,
        trigger_id,
        &trigger,
        captured,
        session::now(),
        state.sessions.next_id(),
        &clip,
        session::ClipInfo {
            width: trial.width,
            height: trial.height,
            duration_ms: trial.clip_ms,
            bytes: trial.clip_bytes,
            has_audio: trial.clip_audio,
        },
    )
    .map_err(|e| e.to_string())
}
