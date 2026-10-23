//! The one notification shown when CargoFrame starts. Clicking it opens Settings.

use tauri::AppHandle;

const TITLE: &str = "CargoFrame is running";
const BODY: &str = "Click here to edit settings.";

#[cfg(windows)]
pub fn launched(app: &AppHandle) {
    use tauri_winrt_notification::{Duration, Toast};

    // An installed build shows the toast under its own identity. A debug build has no Start
    // Menu entry, so it borrows PowerShell's so that the toast appears at all.
    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    };
    let handle = app.clone();
    let shown = Toast::new(&app_id)
        .title(TITLE)
        .text1(BODY)
        .duration(Duration::Short)
        .on_activated(move |_| {
            crate::windowing::open_settings(&handle);
            Ok(())
        })
        .show();
    if let Err(e) = shown {
        tracing::warn!("launch notification failed: {e}");
    }
}

#[cfg(target_os = "linux")]
pub fn launched(app: &AppHandle) {
    use notify_rust::{Notification, Timeout};

    let handle = app.clone();
    // `show` and the wait for a click both block, so keep them off the main thread.
    std::thread::spawn(move || {
        let result = Notification::new()
            .appname("CargoFrame")
            .summary(TITLE)
            .body(BODY)
            .icon("cargoframe")
            .action("default", "Open settings")
            .timeout(Timeout::Milliseconds(10_000))
            .show();
        match result {
            Ok(note) => note.wait_for_action(|action| {
                if action == "default" {
                    crate::windowing::open_settings(&handle);
                }
            }),
            Err(e) => tracing::warn!("launch notification failed: {e}"),
        }
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn launched(_app: &AppHandle) {}
