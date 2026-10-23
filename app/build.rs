fn main() {
    // Only the commands listed here exist for the web view, and each window is granted just
    // the ones it needs (see capabilities/).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_info",
            "get_general",
            "set_general",
            "get_autostart",
            "set_autostart",
            "open_config_folder",
            "get_session",
            "get_session_image",
            "close_capture",
            "edit_layout",
            "get_shortcut_errors",
            "reload_triggers",
        ]),
    ))
    .expect("failed to run tauri-build");
}
