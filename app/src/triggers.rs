//! Triggers: global shortcuts that start a capture, plus the tray entries that do the same.
//! On Wayland the desktop holds the shortcuts (see `desktop_keys`).

use crate::state::AppState;
use crate::video::{self, Step};
use crate::{notify, session, tray, windowing};
use cargoframe_config::{shortcut, Kind, Trigger, Video};
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Register the shortcut of every enabled trigger, replacing what was registered before.
/// Call at start-up and whenever triggers change.
pub fn reload(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let wayland = crate::desktop_keys::wayland();
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
    let mut for_desktop = Vec::new();
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
        if wayland {
            // The plugin only hears keys pressed in X11 programs: the desktop takes them.
            claimed.insert(canonical.clone(), id.clone());
            for_desktop.push(cargoframe_shortcuts::Binding {
                id: id.clone(),
                name: trigger.name.clone(),
                shortcut: canonical,
            });
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
            key(app, &trigger_id, event.state == ShortcutState::Pressed);
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
    if wayland {
        crate::desktop_keys::apply(app, for_desktop, errors);
    } else {
        state.set_shortcut_status(cargoframe_shortcuts::Status {
            route: Some(cargoframe_shortcuts::Route::System),
            errors,
            ..Default::default()
        });
    }
    tray::refresh_menu(app);
}

/// Continuous recording is on in the settings: a shortcut is then acted on when it comes up, so
/// holding it can mean something else.
fn holds_count(app: &AppHandle) -> bool {
    app.try_state::<AppState>().is_some_and(|s| {
        s.store
            .load_or_default::<Video>()
            .map(|v| v.continuous)
            .unwrap_or(false)
    })
}

/// A shortcut's key went down (`down`) or came up.
fn key(app: &AppHandle, trigger_id: &str, down: bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if !holds_count(app) && !state.recording.is_continuous() {
        // as always: a press is acted on at once
        if down {
            tracing::debug!(trigger = %trigger_id, "shortcut pressed");
            run(app, trigger_id.to_string());
        }
        return;
    }
    if down {
        let now = std::time::Instant::now();
        if !state.holds.down(trigger_id, now) {
            return;
        }
        let app = app.clone();
        let id = trigger_id.to_string();
        let wait = crate::hold::Holds::after(now, Video::HOLD_MS) - now;
        let _ = std::thread::Builder::new()
            .name("shortcut-hold".into())
            .spawn(move || {
                std::thread::sleep(wait);
                let state = app.state::<AppState>();
                if state.holds.held(&id, now) {
                    held(&app, &id);
                }
            });
        return;
    }
    if state.holds.up(trigger_id) == crate::hold::Act::Press {
        tracing::debug!(trigger = %trigger_id, "shortcut pressed");
        run(app, trigger_id.to_string());
    }
}

/// A shortcut was held: stop the continuous recording, or ask to start one.
pub fn held(app: &AppHandle, trigger_id: &str) {
    let state = app.state::<AppState>();
    if state.recording.is_continuous() {
        stop_continuous(app);
        return;
    }
    if !matches!(
        state.recording.view(),
        video::RecordingView::Idle | video::RecordingView::Failed { .. }
    ) {
        notify::problem(
            app,
            "CargoFrame is already recording",
            "Press the shortcut to stop that recording first, then hold it to record continuously.",
        );
        return;
    }
    let keep = state
        .store
        .load_or_default::<Video>()
        .map(video::normalized)
        .unwrap_or_default()
        .window_secs;
    let app2 = app.clone();
    let id = trigger_id.to_string();
    // the question waits for an answer: not on the thread that got the key
    std::thread::spawn(move || {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
        let yes = app2
            .dialog()
            .message(format!(
                "CargoFrame records your screen until you hold the shortcut again. Each press of a shortcut then opens its window with the last {keep} seconds, and the recording goes on."
            ))
            .title("Record continuously?")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Start recording".into(),
                "Not now".into(),
            ))
            .blocking_show();
        if yes {
            start_continuous(&app2, &id);
        }
    });
}

/// Start a continuous recording, as the layout of `trigger_id` asks (the sounds); it records
/// the whole screen, so the window in focus can change while it runs.
pub fn start_continuous(app: &AppHandle, trigger_id: &str) {
    let state = app.state::<AppState>();
    let settings = state
        .store
        .load_or_default::<Video>()
        .map(video::normalized)
        .unwrap_or_default();
    let layout = layout_of(&state, trigger_id);
    let (mut wish, mut notes) = video::wish_for(layout.as_ref(), &focus);
    // the window in focus changes while it runs: the whole screen is recorded, and the window
    // the clip is about is chosen when it is handed out
    wish.area = None;
    let started = state.recording.start_continuous(
        &settings,
        video::Os::current(),
        &video::Session::current(),
        &video::work_dir(&state.store),
        &wish,
    );
    match started {
        Ok(s) => {
            tracing::info!(sound = s.sound, "continuous recording started");
            notes.extend(s.notes);
            let mut body = format!(
                "Press a shortcut to open its window with the last {} seconds; the recording goes on. Hold it two seconds to stop.",
                settings.window_secs
            );
            for n in &notes {
                body.push(' ');
                body.push_str(n);
            }
            notify::info(app, "CargoFrame is recording continuously", &body);
            state.set_watch(crate::watch::Watch::start_following(
                app,
                &settings.sign,
                video::Session::current().wayland,
                u64::from(settings.window_secs) * 1000,
            ));
            tray::refresh_menu(app);
        }
        Err(e) => {
            tracing::error!("continuous recording could not start: {e}");
            notify::problem(app, "CargoFrame could not start recording", &e);
        }
    }
}

/// Stop the continuous recording and throw it away (what was handed out stays).
pub fn stop_continuous(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.recording.cancel();
    drop(state.take_watch());
    tray::refresh_menu(app);
    notify::info(
        app,
        "Continuous recording stopped",
        "Nothing more is recorded.",
    );
}

/// The layout a trigger opens.
fn layout_of(state: &AppState, trigger_id: &str) -> Option<cargoframe_layout::Layout> {
    state
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
        })
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
    run_with(app, trigger_id, Vec::new());
}

/// The same, with values another program sent for the window's blocks (`cargoframe open`).
pub fn run_with(app: &AppHandle, trigger_id: String, values: Vec<(String, String)>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.paused() {
        tracing::debug!("triggers are paused");
        return;
    }
    // A trigger whose layout has a Video block starts a recording, and stops it the next time.
    let makes_video = state
        .store
        .load_item::<Trigger>(Kind::Triggers, &trigger_id)
        .ok()
        .flatten()
        .is_some_and(|t| session::records(&state.store, &t));
    if makes_video {
        if !values.is_empty() {
            notify::problem(
                app,
                "CargoFrame",
                "A layout that records cannot take values from another program yet; it records as usual.",
            );
        }
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
            Ok(Ok(mut s)) => {
                session::apply_incoming(&mut s.view, &values);
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
        state.recording.is_continuous(),
    ) {
        Step::Refuse(why) => notify::problem(app, "CargoFrame", why),
        Step::Report(why) => {
            // hand the failure over (this also clears it), so the next press starts again
            let _ = state.recording.stop();
            drop(state.take_watch());
            tray::refresh_menu(app);
            notify::problem(app, "The recording stopped by itself", &why);
        }
        // a recording is not started, or finished, when there would be no window to show it in
        Step::Start | Step::Stop | Step::Snapshot if no_room(app, &state) => {}
        Step::Start => start_recording(app, &state, trigger_id),
        Step::Stop => stop_recording(app, &state, trigger_id),
        Step::Snapshot => snapshot_recording(app, &state, trigger_id),
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
    let layout = layout_of(state, trigger_id);
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
            let area = wish.area.map(|a| cargoframe_indicator::Rect {
                x: a.x,
                y: a.y,
                w: a.w,
                h: a.h,
            });
            state.set_watch(crate::watch::Watch::start(
                app,
                &settings.sign,
                area,
                video::Session::current().wayland,
            ));
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
    let watched = state.take_watch().map(|w| w.finish());
    tray::refresh_menu(app);
    let keep_ms = u64::from(
        state
            .store
            .load_or_default::<Video>()
            .map(video::normalized)
            .unwrap_or_default()
            .window_secs,
    ) * 1000;
    let made = session_from_recording(state, trigger_id, trial, watched.as_ref(), keep_ms);
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

/// A press during a continuous recording: the window opens with its last seconds, and the
/// recording goes on. The window in focus the longest in that part is what the capture is about.
fn snapshot_recording(app: &AppHandle, state: &AppState, trigger_id: &str) {
    if !state.try_begin_capture() {
        tracing::debug!("a capture is already running");
        return;
    }
    let trial = state.recording.snapshot();
    let watched = state.peek_watch();
    let keep_ms = u64::from(
        state
            .store
            .load_or_default::<Video>()
            .map(video::normalized)
            .unwrap_or_default()
            .window_secs,
    ) * 1000;
    let made =
        session_from_recording(state, trigger_id, trial, watched.as_ref(), keep_ms).map(|s| {
            crop_to_focus(
                app,
                s,
                watched.as_ref(),
                keep_ms,
                &layout_of(state, trigger_id),
            )
        });
    state.end_capture();
    match made {
        Ok(s) => {
            state.set_last_program(s.view.app.as_ref().and_then(|a| a.exe_path.clone()));
            show(app, state, s);
        }
        Err(e) => {
            tracing::error!(trigger = %trigger_id, "the last seconds could not be handed out: {e}");
            notify::problem(app, "CargoFrame could not open the recording", &e);
        }
    }
}

/// A continuous recording holds the whole screen. When the layout records the window in focus,
/// the window starts cropped to the one in focus the longest in the clip, which the person can
/// change or take away.
fn crop_to_focus(
    app: &AppHandle,
    mut s: session::Session,
    watched: Option<&crate::watch::Watched>,
    keep_ms: u64,
    layout: &Option<cargoframe_layout::Layout>,
) -> session::Session {
    let wants_window = layout.as_ref().is_some_and(|l| {
        l.blocks.iter().any(|b| {
            matches!(&b.kind, cargoframe_layout::BlockKind::Video(v) if v.subject == cargoframe_layout::VideoSubject::FocusedWindow)
        })
    });
    let Some(video) = s.view.video.as_mut() else {
        return s;
    };
    if !wants_window {
        return s;
    }
    let Some((focused, _)) = watched.and_then(|w| crate::watch::longest(w, keep_ms)) else {
        return s;
    };
    let b = focused.bounds;
    let desktop = crate::watch::desktop_of(app);
    video.crop = crate::watch::crop_in(
        cargoframe_indicator::Rect {
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
        },
        desktop.as_ref(),
        (video.width, video.height),
    );
    s
}

/// The capture the window shows, from a recording that has just been stopped.
fn session_from_recording(
    state: &AppState,
    trigger_id: &str,
    trial: video::Trial,
    watched: Option<&crate::watch::Watched>,
    keep_ms: u64,
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
    let mut captured = session::capture_for_video(&trigger).map_err(|e| e.to_string())?;
    // the data comes from the program in focus the longest in the part kept, not from the one
    // the shortcut was pressed in
    if let Some((app, others)) = watched.and_then(|w| crate::watch::longest(w, keep_ms)) {
        let same = captured.app.as_ref().map(|a| (&a.exe_path, &a.title))
            == Some((&app.exe_path, &app.title));
        if !same || !others.is_empty() {
            let name = app
                .exe_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| app.title.clone());
            captured.notes.push(if others.is_empty() {
                format!("The data is from {name}, in focus during the recording.")
            } else {
                format!(
                    "The recording also shows {}; the data is from {name}, in focus the longest.",
                    others.join(", ")
                )
            });
            captured.app = Some(app);
        }
    }
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
