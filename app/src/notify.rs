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

/// A short notice that something went wrong (a capture failed). No click action.
#[cfg(windows)]
pub fn problem(app: &AppHandle, title: &str, body: &str) {
    use tauri_winrt_notification::{Duration, Toast};
    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    };
    if let Err(e) = Toast::new(&app_id)
        .title(title)
        .text1(body)
        .duration(Duration::Short)
        .show()
    {
        tracing::warn!("notification failed: {e}");
    }
}

#[cfg(target_os = "linux")]
pub fn problem(_app: &AppHandle, title: &str, body: &str) {
    use notify_rust::{Notification, Timeout};
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        if let Err(e) = Notification::new()
            .appname("CargoFrame")
            .summary(&title)
            .body(&body)
            .icon("cargoframe")
            .timeout(Timeout::Milliseconds(8_000))
            .show()
        {
            tracing::warn!("notification failed: {e}");
        }
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn problem(_app: &AppHandle, _title: &str, _body: &str) {}

/// A short notice that is not a problem (a recording has started). Shown like `problem`.
pub fn info(app: &AppHandle, title: &str, body: &str) {
    problem(app, title, body);
}

/// "A newer version is available". Clicking it opens Settings, General, where Updates is.
#[cfg(windows)]
pub fn update_available(app: &AppHandle, version: &str) {
    use tauri_winrt_notification::{Duration, Toast};

    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    };
    let handle = app.clone();
    let title = format!("CargoFrame {version} is available");
    let shown = Toast::new(&app_id)
        .title(&title)
        .text1("Click here to update.")
        .duration(Duration::Long)
        .on_activated(move |_| {
            crate::windowing::open_settings_at(&handle, "general");
            Ok(())
        })
        .show();
    if let Err(e) = shown {
        tracing::warn!("update notification failed: {e}");
    }
}

#[cfg(target_os = "linux")]
pub fn update_available(app: &AppHandle, version: &str) {
    use notify_rust::{Notification, Timeout};

    let handle = app.clone();
    let title = format!("CargoFrame {version} is available");
    std::thread::spawn(move || {
        let result = Notification::new()
            .appname("CargoFrame")
            .summary(&title)
            .body("Click here to update.")
            .icon("cargoframe")
            .action("default", "Open settings")
            .timeout(Timeout::Milliseconds(15_000))
            .show();
        match result {
            Ok(note) => note.wait_for_action(|action| {
                if action == "default" {
                    crate::windowing::open_settings_at(&handle, "general");
                }
            }),
            Err(e) => tracing::warn!("update notification failed: {e}"),
        }
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn update_available(_app: &AppHandle, _version: &str) {}
