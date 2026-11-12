//! CargoFrame: the tray app.
//!
//! The always-running part is small on purpose: a tray icon, a shortcut listener and the
//! state. Windows (and their web views) exist only while open.

mod audio;
mod chats;
mod cleanup;
mod cli;
mod commands;
mod connections;
mod desktop;
mod desktop_keys;
mod hold;
mod items;
mod logging;
mod notify;
mod registry;
mod secrets;
mod session;
mod setup;
mod share;
mod state;
mod timing;
mod tools;
mod transfer;
mod tray;
mod tray_host;
mod triggers;
mod updates;
mod video;
mod warm;
mod watch;
mod windowing;

use cargoframe_config::{Dirs, Store};
use cli::{Cli, Startup};
use connections::Connections;
use state::AppState;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;

/// `cargoframe open ...` from another program: run the trigger with the values it sent, or
/// say what was wrong with the request.
fn open_from_another_program(app: &tauri::AppHandle, open: Result<cli::OpenRequest, String>) {
    match open {
        Ok(req) => {
            tracing::info!(trigger = %req.trigger, values = req.values.len(), "asked to open by another program");
            triggers::run_with(app, req.trigger, req.values);
        }
        Err(why) => notify::problem(app, "CargoFrame could not open that", &why),
    }
}

pub fn run() {
    let cli = Cli::parse(std::env::args());
    let dirs = Dirs::from_env().expect("cannot find a settings folder for this user");
    logging::init(&dirs);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), ?cli, "starting");

    // WebKit's GPU path shows blank windows with NVIDIA's driver on Wayland: draw without it there.
    #[cfg(target_os = "linux")]
    if desktop_keys::wayland()
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
        && std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        tracing::info!("NVIDIA on Wayland: WebKit draws without DMA-BUF");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    let tray_available = tray_host::available();
    let startup = cli::startup(&cli, tray_available);

    let app = tauri::Builder::default()
        // Must be first: a second launch tells this copy what to do, then exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let second = Cli::parse(argv);
            if second.quit {
                tracing::info!("asked to quit by another launch");
                app.exit(0);
            } else if let Some(open) = second.open {
                open_from_another_program(app, open);
            } else {
                windowing::open_settings(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(setup::Installer::default())
        .manage(AppState::new(
            Store::new(dirs),
            tray_available,
            Connections::new(
                std::sync::Arc::new(cargoframe_net::UreqHttp::new()),
                std::sync::Arc::new(secrets::KeyringStore),
            ),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_general,
            commands::set_general,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_data_report,
            commands::clean_data,
            commands::reset_everything,
            commands::open_config_folder,
            commands::get_session,
            commands::get_session_image,
            commands::get_session_clip,
            commands::close_capture,
            commands::edit_layout,
            commands::open_settings,
            commands::get_shortcut_status,
            commands::list_items,
            commands::save_item,
            commands::delete_item,
            commands::new_item_id,
            commands::validate_layout,
            commands::list_layout_templates,
            commands::layout_from_template,
            commands::export_settings,
            commands::open_import,
            commands::apply_import,
            commands::discard_import,
            commands::list_variables,
            commands::test_rule,
            commands::preview_files,
            commands::button_reach,
            commands::get_safe,
            commands::set_safe,
            commands::test_safe_folder,
            commands::pick_folder,
            commands::list_connections,
            commands::set_connection_secret,
            commands::connect_connection,
            commands::cancel_sign_in,
            commands::disconnect_connection,
            commands::test_connection,
            commands::describe_connection,
            commands::preview_options,
            commands::list_chats,
            commands::pin_chat,
            commands::set_chat_pinned,
            commands::remove_chat,
            commands::fetch_options,
            commands::set_edited_image,
            commands::read_text,
            commands::add_files,
            commands::read_offered_file,
            commands::set_clip_mark,
            commands::set_clip_layer,
            commands::share,
            commands::open_link,
            commands::report_painted,
            commands::get_timings,
            commands::get_memory_status,
            commands::get_update_view,
            commands::check_update,
            commands::install_update,
            commands::set_update_token,
            commands::get_video,
            commands::set_video,
            commands::detect_video_tools,
            commands::list_audio_devices,
            commands::get_setup,
            commands::install_missing,
            commands::open_sound_settings,
            commands::try_video_recording,
            commands::get_video_test_clip,
            commands::open_video_test,
        ])
        .on_window_event(|window, event| match event {
            // Settings, General, Memory decides whether a closed window is destroyed or kept.
            tauri::WindowEvent::CloseRequested { api, .. } => {
                windowing::close_requested(window, api)
            }
            // Closing the CargoFrame window frees its capture (the screenshot is large).
            tauri::WindowEvent::Destroyed if windowing::is_capture_label(window.label()) => {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    windowing::forget_capture(&state, window.label());
                }
            }
            _ => {}
        })
        .setup(move |app| {
            if startup == Startup::Exit {
                app.handle().exit(0);
                return Ok(());
            }
            // Nothing is open yet, so every capture folder found now was left by a crash.
            if let Some(state) = app.handle().try_state::<AppState>() {
                let swept = state.sweep_captures(std::time::Duration::ZERO);
                if swept.removed > 0 {
                    tracing::info!(removed = swept.removed, "removed old capture folders");
                }
            }
            sweep_every_hour(app.handle().clone());
            check_for_updates_daily(app.handle().clone());
            tray::build(app.handle())?;
            triggers::reload(app.handle());
            notify::launched(app.handle());
            // started by another program with `open`: do what it asked once the shortcuts are in
            if let Some(open) = cli.open.clone() {
                open_from_another_program(app.handle(), open);
            }
            if startup == Startup::OpenSettings {
                // The first run lands on Setup, which says whether anything is missing; after
                // that, Settings opens where it was left.
                let first = app
                    .handle()
                    .try_state::<AppState>()
                    .is_some_and(|s| setup::first_run(s.store.dirs().config()));
                if first {
                    windowing::open_settings_at(app.handle(), "setup");
                } else {
                    windowing::open_settings(app.handle());
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building CargoFrame");

    app.run(|app, event| {
        match event {
            RunEvent::ExitRequested { api, code, .. } => {
                // Closing the last window must not quit: the tray keeps the app alive. Only an
                // explicit exit (a code is set) ends it.
                if code.is_none() {
                    api.prevent_exit();
                } else {
                    tracing::info!("exiting");
                }
            }
            // however the program ends, ffmpeg must not be left recording the screen
            RunEvent::Exit => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.recording.cancel();
                    drop(state.take_watch());
                }
            }
            _ => {}
        }
    });
}

/// With "Check for updates" on in General, look for a newer version a minute after start and
/// then once a day, and say so once per version. Nothing is downloaded or installed here: that
/// is a button in Settings. Downloads left by an earlier run are removed first.
fn check_for_updates_daily(app: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            if let Some(state) = app.try_state::<AppState>() {
                updates::clear_downloads(&state.store.dirs().data().join("updates"));
            }
            std::thread::sleep(std::time::Duration::from_secs(60));
            loop {
                if let Some(state) = app.try_state::<AppState>() {
                    let on = state
                        .store
                        .load_or_default::<cargoframe_config::General>()
                        .map(|g| g.check_updates)
                        .unwrap_or(false);
                    if on {
                        state.updates.check();
                        let _ = tauri::Emitter::emit(&app, "update-changed", ());
                        if let Some(version) = state.updates.news() {
                            notify::update_available(&app, &version);
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(24 * 3600));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("could not start the update check: {e}");
    }
}

/// Capture folders belong to open windows; this removes any that got left behind.
fn sweep_every_hour(app: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("capture-sweep".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
            if let Some(state) = app.try_state::<AppState>() {
                let swept = state.sweep_captures(std::time::Duration::from_secs(120));
                if swept.removed > 0 || swept.failed > 0 {
                    tracing::info!(
                        removed = swept.removed,
                        failed = swept.failed,
                        "swept capture folders"
                    );
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("could not start the capture folder sweep: {e}");
    }
}

/// Guards on the shipped security configuration: what the web views may load and which backend
/// commands each window may call. A loosened setting fails here, in review, not in a release.
#[cfg(test)]
mod shipped_config_tests {
    use serde_json::Value;

    fn json(text: &str) -> Value {
        serde_json::from_str(text).expect("valid json")
    }

    fn tauri_conf() -> Value {
        json(include_str!("../tauri.conf.json"))
    }

    fn capabilities() -> Vec<(&'static str, Value)> {
        vec![
            (
                "capture",
                json(include_str!("../capabilities/capture.json")),
            ),
            (
                "settings",
                json(include_str!("../capabilities/settings.json")),
            ),
        ]
    }

    fn directive<'a>(csp: &'a str, name: &str) -> Option<&'a str> {
        csp.split(';')
            .map(str::trim)
            .find_map(|d| d.strip_prefix(name).map(str::trim))
    }

    #[test]
    fn the_content_security_policy_stays_strict() {
        let conf = tauri_conf();
        let csp = conf["app"]["security"]["csp"].as_str().expect("a CSP");
        assert_eq!(directive(csp, "default-src"), Some("'self'"));
        for forbidden in ["'unsafe-eval'", "http:", "https:", "*"] {
            assert!(
                !csp.split_whitespace().any(|t| t == forbidden),
                "the CSP must not allow {forbidden}: {csp}"
            );
        }
        // Scripts come only from the app itself.
        assert!(directive(csp, "script-src")
            .map(|s| !s.contains("'unsafe-inline'"))
            .unwrap_or(true));
        // A clip can be played from a blob the page made itself, and from nowhere else.
        assert_eq!(directive(csp, "media-src"), Some("'self' blob:"));
        // The page talks to the backend and nothing else.
        assert_eq!(
            directive(csp, "connect-src"),
            Some("ipc: http://ipc.localhost")
        );
        for locked in ["object-src", "base-uri", "frame-src", "form-action"] {
            assert_eq!(directive(csp, locked), Some("'none'"), "{locked}");
        }
        assert_eq!(conf["app"]["withGlobalTauri"], false);
    }

    #[test]
    fn windows_only_get_our_own_commands_and_event_listening() {
        for (name, cap) in capabilities() {
            let windows = cap["windows"].as_array().expect("windows");
            // the CargoFrame window can be opened several times (capture, capture-2, ...)
            let several = format!("{name}-*");
            assert!(
                windows[0] == name
                    && (windows.len() == 1
                        || (windows.len() == 2
                            && windows[1] == several.as_str()
                            && name == "capture")),
                "{name} capability is for its own window only: {windows:?}"
            );
            assert!(
                cap.get("remote").is_none(),
                "{name}: a capability must never be given to remote pages"
            );
            for p in cap["permissions"].as_array().expect("permissions") {
                let p = p.as_str().expect("a permission name");
                let ours = p.starts_with("allow-");
                let events = p == "core:event:allow-listen" || p == "core:event:allow-unlisten";
                assert!(
                    ours || events,
                    "{name} window is granted {p}: web pages get only the app's own commands"
                );
            }
        }
    }

    /// The names of the functions marked as commands in `commands.rs`.
    fn defined_commands() -> std::collections::BTreeSet<String> {
        include_str!("commands.rs")
            .split("#[tauri::command]")
            .skip(1)
            .filter_map(|after| {
                let at = after.find("fn ")?;
                let rest = &after[at + 3..];
                let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
                Some(rest[..end].to_string())
            })
            .collect()
    }

    /// A command that is written but not registered, not listed for the web view, or not given to
    /// a window fails only when someone uses it. This keeps the four places in step.
    #[test]
    fn every_command_is_registered_listed_and_given_to_a_window() {
        let defined = defined_commands();
        assert!(
            defined.len() > 50,
            "the commands were not found: {defined:?}"
        );
        // registered: the lines `commands::name,` in the handler
        let lib = include_str!("lib.rs");
        let handler = &lib[lib.find("generate_handler![").expect("a handler")..];
        let handler = &handler[..handler.find(']').expect("the end of the handler")];
        let registered: std::collections::BTreeSet<String> = handler
            .lines()
            .filter_map(|l| l.trim().strip_prefix("commands::"))
            .map(|l| l.trim_end_matches(',').to_string())
            .collect();
        // listed: the strings in build.rs
        let build = include_str!("../build.rs");
        let listed: std::collections::BTreeSet<String> = build
            .split('"')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.to_string())
            .filter(|s| defined.contains(s))
            .collect();
        // given: `allow-some-name` in either window's capability
        let given: std::collections::BTreeSet<String> = capabilities()
            .iter()
            .flat_map(|(_, cap)| cap["permissions"].as_array().unwrap().clone())
            .filter_map(|p| {
                p.as_str()
                    .and_then(|p| p.strip_prefix("allow-"))
                    .map(|p| p.replace('-', "_"))
            })
            .collect();
        for name in &defined {
            assert!(
                registered.contains(name),
                "{name} is not in the invoke handler"
            );
            assert!(listed.contains(name), "{name} is not listed in build.rs");
            assert!(given.contains(name), "{name} is not given to any window");
        }
        for name in &registered {
            assert!(
                defined.contains(name),
                "the handler names {name}, which is not a command"
            );
        }
        for name in &given {
            assert!(
                defined.contains(name),
                "a window is given {name}, which is not a command"
            );
        }
    }

    /// What belongs to one capture is for the CargoFrame window only: Settings has no part in it.
    #[test]
    fn the_settings_window_holds_nothing_that_belongs_to_a_capture() {
        let caps = capabilities();
        let (_, settings) = &caps[1];
        let granted: Vec<&str> = settings["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for forbidden in [
            "allow-get-session",
            "allow-get-session-image",
            "allow-get-session-clip",
            "allow-set-edited-image",
            "allow-set-clip-mark",
            "allow-set-clip-layer",
            "allow-share",
        ] {
            assert!(
                !granted.contains(&forbidden),
                "the Settings window must not hold {forbidden}"
            );
        }
    }

    /// The CargoFrame window shows a screen capture and text read from files, so it is the less
    /// trusted window: it holds exactly these commands, all about its own capture. A new command
    /// for it has to be added here on purpose, never by accident.
    #[test]
    fn the_capture_window_holds_only_what_its_own_capture_needs() {
        let caps = capabilities();
        let (_, capture) = &caps[0];
        let mut granted: Vec<&str> = capture["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        granted.sort_unstable();
        let mut expected = vec![
            "allow-app-info",
            "allow-close-capture",
            "allow-edit-layout",
            "allow-fetch-options",
            "allow-get-general",
            "allow-get-session",
            "allow-get-session-clip",
            "allow-get-session-image",
            "allow-open-link",
            "allow-open-settings",
            "allow-report-painted",
            "allow-set-clip-mark",
            "allow-set-clip-layer",
            "allow-set-edited-image",
            "allow-read-text",
            "allow-add-files",
            "allow-read-offered-file",
            "allow-share",
            "core:event:allow-listen",
            "core:event:allow-unlisten",
        ];
        expected.sort_unstable();
        assert_eq!(granted, expected);
    }
}
