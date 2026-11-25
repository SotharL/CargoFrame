//! Starting CargoFrame when the person signs in.
//!
//! On Windows this is the person's own `Run` key only (`HKEY_CURRENT_USER`), the same value the
//! installer writes. The autostart plugin used before 0.7.2 put it in the machine's `Run` key
//! whenever the program ran as an administrator, and an installer cannot change that key: a copy
//! built by hand and run that way (`target\release\cargoframe.exe`) then kept starting at
//! sign-in after every update, and the newer copy, started second, only handed over to it.
//!
//! So, when an installed copy starts, [`repair`] points the person's entry at itself and removes
//! any `CargoFrame` entry of the machine's that starts another program; one it is not allowed to
//! remove is listed in Settings ([`others`]), whose button asks Windows for the right once
//! ([`remove_others`]). On Linux it is the XDG autostart entry `install.sh` writes.

// The rules about the entries found are Windows' only; they are tested everywhere.
#![cfg_attr(not(windows), allow(dead_code))]

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::path::{Path, PathBuf};

/// The entry's name: the `Run` value on Windows, `CargoFrame.desktop` on Linux.
pub const NAME: &str = "CargoFrame";
const ARG: &str = "--autostart";

/// The program to start: the AppImage when running from one, else this program.
fn program() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(p) = std::env::var_os("APPIMAGE") {
        return Some(PathBuf::from(p));
    }
    std::env::current_exe().ok()
}

fn launcher() -> Result<AutoLaunch, String> {
    let exe = program().ok_or("CargoFrame could not find its own program")?;
    let mut b = AutoLaunchBuilder::new();
    b.set_app_name(NAME).set_args(&[ARG]);
    // quoted, as the installer writes it: a path with a space starts as one program
    #[cfg(windows)]
    b.set_app_path(&format!("\"{}\"", exe.display()))
        .set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
    #[cfg(not(windows))]
    b.set_app_path(&exe.display().to_string());
    b.build().map_err(|e| e.to_string())
}

pub fn is_enabled() -> Result<bool, String> {
    launcher()?.is_enabled().map_err(|e| e.to_string())
}

pub fn set(enabled: bool) -> Result<(), String> {
    let l = launcher()?;
    if enabled { l.enable() } else { l.disable() }.map_err(|e| e.to_string())
}

/// The program a `Run` command starts: the quoted path, or what comes before ` --`.
pub fn program_of(command: &str) -> &str {
    let c = command.trim();
    if let Some(rest) = c.strip_prefix('"') {
        return rest.split('"').next().unwrap_or(rest);
    }
    match c.to_ascii_lowercase().find(".exe") {
        Some(i) => &c[..i + 4],
        None => c.split(" --").next().unwrap_or(c).trim(),
    }
}

/// Whether a command starts this program (Windows paths: case aside, either slash).
pub fn starts(command: &str, me: &Path) -> bool {
    let norm = |s: &str| s.replace('/', "\\").to_lowercase();
    norm(program_of(command)) == norm(&me.display().to_string())
}

/// What to do with the entries found, decided without touching anything (so it is tested).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Write the person's entry for this program.
    pub point_here: bool,
    /// Machine entries (their key) to remove: they start another program.
    pub remove: Vec<String>,
}

/// `installed`: this program sits beside its uninstaller. A copy that was not installed (a
/// build, a copy carried around) never takes the entry over.
pub fn plan(me: &Path, installed: bool, user: Option<&str>, machine: &[(String, String)]) -> Plan {
    let mut p = Plan::default();
    if !installed {
        return p;
    }
    let wanted = user.is_some() || !machine.is_empty();
    p.point_here = wanted && !user.is_some_and(|c| starts(c, me));
    p.remove = machine
        .iter()
        .filter(|(_, c)| !starts(c, me))
        .map(|(k, _)| k.clone())
        .collect();
    p
}

#[cfg(windows)]
mod win {
    use windows_registry::{CURRENT_USER, LOCAL_MACHINE};

    pub const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    pub const RUN32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";

    pub fn user() -> Option<String> {
        CURRENT_USER.open(RUN).ok()?.get_string(super::NAME).ok()
    }

    /// The machine's entries named CargoFrame, with their key.
    pub fn machine() -> Vec<(String, String)> {
        [RUN, RUN32]
            .iter()
            .filter_map(|k| {
                let v = LOCAL_MACHINE.open(k).ok()?.get_string(super::NAME).ok()?;
                Some((k.to_string(), v))
            })
            .collect()
    }

    pub fn remove_machine(key: &str) -> bool {
        LOCAL_MACHINE
            .options()
            .write()
            .open(key)
            .and_then(|k| k.remove_value(super::NAME))
            .is_ok()
    }
}

/// Run once when CargoFrame starts (Windows; elsewhere nothing to do).
pub fn repair() {
    #[cfg(windows)]
    {
        let Some(me) = program() else { return };
        let installed = me
            .parent()
            .is_some_and(|d| d.join("uninstall.exe").is_file());
        let p = plan(&me, installed, win::user().as_deref(), &win::machine());
        for key in &p.remove {
            if win::remove_machine(key) {
                tracing::info!("removed an older CargoFrame from the machine's sign-in list");
            }
        }
        if p.point_here {
            if let Err(e) = set(true) {
                tracing::warn!("could not set CargoFrame to start at sign-in: {e}");
            }
        }
    }
}

/// Other programs the machine starts as CargoFrame at sign-in (Windows), for Settings.
pub fn others() -> Vec<String> {
    #[cfg(windows)]
    if let Some(me) = program() {
        return win::machine()
            .into_iter()
            .filter(|(_, c)| !starts(c, &me))
            .map(|(_, c)| program_of(&c).to_string())
            .collect();
    }
    Vec::new()
}

/// Remove those entries with the administrator's right: Windows asks the person once. The
/// command is fixed text (no path or name from anywhere), and the person's own entry is kept.
pub fn remove_others() -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let del = |k: &str| format!(r"reg delete HKLM\{k} /v {NAME} /f");
        let script = format!(
            "Start-Process -FilePath cmd.exe -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '/c {} & {}'",
            del(win::RUN),
            del(win::RUN32)
        );
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Windows did not give the right to change this".into());
        }
    }
    Ok(others())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = r"C:\Users\Ana\AppData\Local\CargoFrame\cargoframe.exe";
    const BUILD: &str = r"C:\Users\Ana\Documents\GitHub\CargoFrame\target\release\cargoframe.exe";

    #[test]
    fn the_program_of_a_run_command() {
        assert_eq!(program_of(&format!("\"{ME}\" --autostart")), ME);
        assert_eq!(program_of(&format!("{BUILD} --autostart")), BUILD);
        assert_eq!(
            program_of(r"C:\My Apps\cargoframe.exe --autostart"),
            r"C:\My Apps\cargoframe.exe"
        );
        assert_eq!(
            program_of("/opt/cargoframe/cargoframe --autostart"),
            "/opt/cargoframe/cargoframe"
        );
        assert!(starts(
            &format!("\"{}\" --autostart", ME.to_uppercase()),
            Path::new(ME)
        ));
        assert!(!starts(&format!("{BUILD} --autostart"), Path::new(ME)));
    }

    #[test]
    fn an_older_build_in_the_machines_list_is_removed_and_the_choice_kept() {
        let machine = [(win_key(), format!("{BUILD} --autostart"))];
        let p = plan(Path::new(ME), true, None, &machine);
        assert_eq!(
            p,
            Plan {
                point_here: true,
                remove: vec![win_key()]
            }
        );
    }

    #[test]
    fn the_persons_entry_is_pointed_here_only_when_it_starts_another() {
        let here = format!("\"{ME}\" --autostart");
        assert_eq!(plan(Path::new(ME), true, Some(&here), &[]), Plan::default());
        let p = plan(
            Path::new(ME),
            true,
            Some(&format!("{BUILD} --autostart")),
            &[],
        );
        assert!(p.point_here && p.remove.is_empty());
        // none: the person turned it off
        assert_eq!(plan(Path::new(ME), true, None, &[]), Plan::default());
    }

    #[test]
    fn a_copy_that_was_not_installed_changes_nothing() {
        let machine = [(win_key(), format!("\"{ME}\" --autostart"))];
        assert_eq!(
            plan(Path::new(BUILD), false, None, &machine),
            Plan::default()
        );
    }

    fn win_key() -> String {
        r"Software\Microsoft\Windows\CurrentVersion\Run".into()
    }
}
