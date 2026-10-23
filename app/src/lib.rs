//! CargoFrame: the tray app.
//!
//! The always-running part is small on purpose: a tray icon, a shortcut listener and the
//! state. Windows (and their web views) exist only while open.

mod cli;
mod commands;
mod logging;
mod notify;
mod state;
mod tray;
mod tray_host;
mod windowing;

use cargoframe_config::{Dirs, Store};
use cli::{Cli, Startup};
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
        .manage(AppState::new(Store::new(dirs), tray_available))
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_general,
            commands::set_general,
            commands::get_autostart,
            commands::set_autostart,
            commands::open_config_folder,
        ])
        .setup(move |app| {
            if startup == Startup::Exit {
                app.handle().exit(0);
                return Ok(());
            }
            tray::build(app.handle())?;
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
