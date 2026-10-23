// No console window for the release build on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cargoframe_app::run();
}
