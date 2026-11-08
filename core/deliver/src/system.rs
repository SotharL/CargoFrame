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

/// On Linux the clipboard's content is served by the program that put it there. After a hand-over
/// the person pastes later, so the last clipboard is kept until the next one replaces it.
#[cfg(target_os = "linux")]
static KEPT: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

#[cfg(target_os = "linux")]
impl Drop for SystemDesk {
    fn drop(&mut self) {
        let mine = self
            .clipboard
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(c) = mine {
            *KEPT.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
        }
    }
}

/// The Wayland desktop this runs on, or `None` on X11 and elsewhere.
#[cfg(target_os = "linux")]
fn wayland() -> Option<cargoframe_capture::wayland::Compositor> {
    use cargoframe_capture::session::{detect_env, Session};
    (detect_env() == Session::Wayland).then(cargoframe_capture::wayland::Compositor::detect_env)
}

/// What presses keys on this Wayland desktop: `wtype` on wlroots desktops (sway, Hyprland).
/// GNOME and KDE let no program press keys in another.
#[cfg(target_os = "linux")]
fn key_presser(desktop: &cargoframe_capture::wayland::Compositor) -> Option<PathBuf> {
    desktop
        .is_wlroots()
        .then(|| cargoframe_capture::wayland::find_program("wtype"))
        .flatten()
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

    #[cfg(target_os = "linux")]
    fn can_drive(&self) -> bool {
        match wayland() {
            None => true,
            Some(desktop) => desktop.knows_focus() && key_presser(&desktop).is_some(),
        }
    }
}

#[cfg(windows)]
fn press(key: Key) -> Result<(), String> {
    crate::win::press(key)
}

/// X11 with xdotool; Wayland with wtype where the desktop allows it (sway, Hyprland).
#[cfg(target_os = "linux")]
fn press(key: Key) -> Result<(), String> {
    if let Some(desktop) = wayland() {
        let program = key_presser(&desktop).ok_or_else(|| {
            format!(
                "{} does not let CargoFrame press keys: paste with Ctrl+V yourself",
                desktop.name()
            )
        })?;
        let args: &[&str] = match key {
            Key::Paste => &["-M", "ctrl", "v", "-m", "ctrl"],
            Key::Enter => &["-k", "Return"],
        };
        let status = std::process::Command::new(&program)
            .args(args)
            .status()
            .map_err(|e| format!("wtype could not start: {e}"))?;
        return status
            .success()
            .then_some(())
            .ok_or_else(|| format!("wtype failed ({status})"));
    }
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
