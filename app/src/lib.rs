//! CargoFrame: the tray app.
//!
//! The always-running part is small on purpose: a tray icon, a shortcut listener and the
//! state. Windows (and their web views) exist only while open.

mod chats;
mod cli;
mod commands;
mod connections;
mod desktop;
mod items;
mod logging;
mod notify;
mod registry;
mod secrets;
mod session;
mod share;
mod state;
mod timing;
mod tools;
mod transfer;
mod tray;
mod tray_host;
mod triggers;
mod warm;
mod windowing;

use cargoframe_config::{Dirs, Store};
use cli::{Cli, Startup};
use connections::Connections;
use state::AppState;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    let cli = Cli::parse(std::env::args());
    let dirs = Dirs::from_env().expect("cannot find a settings folder for this user");
    logging::init(&dirs);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), ?cli, "starting");

    let tray_available = tray_host::available();
    let startup = cli::startup(&cli, tray_available);

    let app = tauri::Builder::default()
        // Must be first: a second launch tells this copy what to do, then exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let second = Cli::parse(argv);
            if second.quit {
                tracing::info!("asked to quit by another launch");
                app.exit(0);
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
        .plugin(tauri_plugin_clipboard_manager::init())
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
            commands::open_config_folder,
            commands::get_session,
            commands::get_session_image,
            commands::close_capture,
            commands::edit_layout,
            commands::open_settings,
            commands::get_shortcut_errors,
            commands::reload_triggers,
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
            commands::share,
            commands::copy_text,
            commands::open_link,
            commands::report_painted,
            commands::get_timings,
        ])
        .on_window_event(|window, event| match event {
            // Settings, General, Memory decides whether a closed window is destroyed or kept.
            tauri::WindowEvent::CloseRequested { api, .. } => {
                windowing::close_requested(window, api)
            }
            // Closing the CargoFrame window frees its capture (the screenshot is large).
            tauri::WindowEvent::Destroyed if window.label() == windowing::CAPTURE => {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    state.cancel_share();
                    state.sessions.clear();
                }
            }
            _ => {}
        })
        .setup(move |app| {
            if startup == Startup::Exit {
                app.handle().exit(0);
                return Ok(());
            }
            tray::build(app.handle())?;
            triggers::reload(app.handle());
            notify::launched(app.handle());
            if startup == Startup::OpenSettings {
                windowing::open_settings(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building CargoFrame");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event {
            // Closing the last window must not quit: the tray keeps the app alive. Only an
            // explicit exit (a code is set) ends it.
            if code.is_none() {
                api.prevent_exit();
            } else {
                tracing::info!("exiting");
                let _ = app.try_state::<AppState>();
            }
        }
    });
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
            assert_eq!(windows.len(), 1, "{name} capability is for one window only");
            assert_eq!(windows[0], name);
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

    #[test]
    fn the_capture_window_cannot_change_settings_or_manage_connections() {
        let caps = capabilities();
        let (_, capture) = &caps[0];
        let granted: Vec<&str> = capture["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for forbidden in [
            "allow-save-item",
            "allow-delete-item",
            "allow-set-connection-secret",
            "allow-connect-connection",
            "allow-disconnect-connection",
            "allow-set-general",
            "allow-set-safe",
            "allow-pick-folder",
            "allow-list-layout-templates",
            "allow-layout-from-template",
            "allow-export-settings",
            "allow-open-import",
            "allow-apply-import",
            "allow-discard-import",
        ] {
            assert!(
                !granted.contains(&forbidden),
                "the CargoFrame window must not hold {forbidden}"
            );
        }
    }
}
