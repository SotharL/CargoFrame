//! The real desktop.

use crate::{Deliver, DeliverError, Delivered, Delivery, Desk, Key, Window};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// Delivery against this computer's desktop.
pub struct System;

impl Deliver for System {
    fn deliver(
        &self,
        delivery: &Delivery,
        open: &dyn Fn(&str) -> Result<(), String>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Delivered, DeliverError> {
        crate::deliver(&SystemDesk::default(), open, delivery, cancelled)
    }
}

/// The real front window, clipboard and keyboard.
#[derive(Default)]
pub struct SystemDesk {
    // Kept for the whole delivery: on X11 the clipboard's content is served by this program, so
    // it has to stay alive until the paste has happened.
    clipboard: Mutex<Option<arboard::Clipboard>>,
}

impl SystemDesk {
    fn with_clipboard<R>(
        &self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<R, arboard::Error>,
    ) -> Result<R, String> {
        let mut guard = self.clipboard.lock().unwrap_or_else(|e| e.into_inner());
        let clipboard = match guard.as_mut() {
            Some(c) => c,
            None => guard.insert(arboard::Clipboard::new().map_err(|e| e.to_string())?),
        };
        f(clipboard).map_err(|e| e.to_string())
    }
}

/// `C:\Program Files\Claude\Claude.exe` becomes `claude`.
fn program_name(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .filter(|s| !s.is_empty())
}

impl Desk for SystemDesk {
    fn foreground(&self) -> Result<Option<Window>, String> {
        cargoframe_capture::focused_app()
            .map(|app| {
                app.map(|a| Window {
                    id: a.window,
                    title: a.title,
                    program: a.exe_path.as_deref().and_then(program_name),
                })
            })
            .map_err(|e| e.to_string())
    }

    fn can_paste_files(&self) -> bool {
        cfg!(any(windows, target_os = "macos"))
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn put_files(&self, files: &[PathBuf]) -> Result<(), String> {
        self.with_clipboard(|c| c.set().file_list(files))
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn put_files(&self, _files: &[PathBuf]) -> Result<(), String> {
        Err("this system cannot paste files from CargoFrame yet".into())
    }

    fn put_text(&self, text: &str) -> Result<(), String> {
        self.with_clipboard(|c| c.set_text(text.to_string()))
    }

    fn press(&self, key: Key) -> Result<(), String> {
        press(key)
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

#[cfg(windows)]
fn press(key: Key) -> Result<(), String> {
    crate::win::press(key)
}

/// X11 only; there is no way to press keys in another program under Wayland.
#[cfg(target_os = "linux")]
fn press(key: Key) -> Result<(), String> {
    let keys = match key {
        Key::Paste => "ctrl+v",
        Key::Enter => "Return",
    };
    let status = std::process::Command::new("xdotool")
        .args(["key", "--clearmodifiers", keys])
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "xdotool is not installed: install it so CargoFrame can press keys (X11 only)"
                    .to_string()
            } else {
                e.to_string()
            }
        })?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("xdotool failed ({status})"))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn press(_key: Key) -> Result<(), String> {
    Err("pressing keys is not supported on this system".into())
}

#[cfg(test)]
mod tests {
    use super::program_name;
    use std::path::Path;

    #[test]
    fn a_program_is_named_by_its_file_without_folder_or_extension() {
        assert_eq!(
            program_name(Path::new("/opt/Claude/Claude.exe")).as_deref(),
            Some("claude")
        );
        assert_eq!(
            program_name(Path::new("/opt/google/chrome/chrome")).as_deref(),
            Some("chrome")
        );
        assert_eq!(program_name(Path::new("")), None);
    }
}
