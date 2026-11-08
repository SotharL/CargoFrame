//! Global shortcuts on Wayland.
//!
//! On Windows and X11 the global-shortcut plugin listens to the keys itself. A Wayland desktop
//! does not let a program do that, so each shortcut is handed to the desktop, which runs
//! `cargoframe open --trigger <id>` when it is pressed (the running copy takes it over):
//!
//! | Desktop | How | Lasts |
//! |---------|-----|-------|
//! | GNOME | a custom shortcut in GNOME's keyboard settings (gsettings) | until removed by CargoFrame |
//! | sway | `bindsym … exec …` over sway's IPC | while sway runs; set again at each start |
//! | Hyprland | `keyword bind …, exec, …` over Hyprland's IPC | while Hyprland runs; set again at each start |
//! | KDE Plasma and others | the desktop portal's GlobalShortcuts (the desktop may ask once) | the portal keeps it |
//! | none of these | the command, shown in Settings, to add to the desktop's own shortcuts | |
//!
//! CargoFrame only ever changes the shortcuts it made itself.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[cfg(target_os = "linux")]
pub mod gnome;
#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub mod wlroots;

/// One trigger's shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The trigger's id.
    pub id: String,
    /// The trigger's name, shown in the desktop's settings.
    pub name: String,
    /// The canonical shortcut (`Ctrl+Shift+F9`).
    pub shortcut: String,
}

/// Where the shortcuts of this session live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Windows and X11: CargoFrame listens itself.
    System,
    Gnome,
    Sway,
    Hyprland,
    Portal,
    /// The person adds the command to the desktop's settings.
    Manual,
}

impl Route {
    /// One sentence for Settings, or `None` when there is nothing to say.
    pub fn note(self) -> Option<&'static str> {
        Some(match self {
            Route::System => return None,
            Route::Gnome => "This is a GNOME Wayland desktop: CargoFrame adds its shortcuts to GNOME's keyboard settings (Settings, Keyboard, Custom Shortcuts) and keeps them up to date.",
            Route::Sway => "This is sway: CargoFrame adds its shortcuts to sway each time it starts. They are not written into your sway config.",
            Route::Hyprland => "This is Hyprland: CargoFrame adds its shortcuts to Hyprland each time it starts. They are not written into your Hyprland config.",
            Route::Portal => "On this Wayland desktop the shortcuts go through the desktop's shortcut service. The desktop may ask you once to confirm them.",
            Route::Manual => "This Wayland desktop does not let programs set shortcuts. Add each command below to your desktop's keyboard shortcuts.",
        })
    }
}

/// What Settings shows about the shortcuts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Status {
    pub route: Option<Route>,
    pub note: Option<String>,
    /// Per trigger: why its shortcut does not work.
    pub errors: BTreeMap<String, String>,
    /// Per trigger, on Wayland: the command a desktop shortcut runs.
    pub commands: BTreeMap<String, String>,
}

/// What the desktop can do for shortcuts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Abilities {
    /// GNOME's custom shortcuts can be written (gsettings and the schema are there).
    pub gnome_settings: bool,
    /// The portal has a GlobalShortcuts service.
    pub portal: bool,
}

/// The route for a Wayland session on `desktop`.
#[cfg(target_os = "linux")]
pub fn choose(desktop: &cargoframe_capture::wayland::Compositor, can: &Abilities) -> Route {
    use cargoframe_capture::wayland::Compositor;
    match desktop {
        Compositor::Sway { .. } => Route::Sway,
        Compositor::Hyprland { .. } => Route::Hyprland,
        // GNOME's own settings work on every version; its portal only from GNOME 48.
        Compositor::Gnome if can.gnome_settings => Route::Gnome,
        _ if can.portal => Route::Portal,
        _ => Route::Manual,
    }
}

/// The program a desktop shortcut starts: the AppImage itself when CargoFrame runs from one
/// (its inside is mounted somewhere new at every start), else this program.
pub fn program(appimage: Option<&str>, current_exe: &Path) -> String {
    match appimage.filter(|a| !a.trim().is_empty()) {
        Some(a) => a.to_string(),
        None => current_exe.to_string_lossy().into_owned(),
    }
}

/// A word quoted for a POSIX shell, when it needs it.
pub fn shell_quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+=:,@%".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// The command line a desktop shortcut runs for a trigger.
pub fn command_line(program: &str, trigger: &str) -> String {
    format!(
        "{} open --trigger {}",
        shell_quote(program),
        shell_quote(trigger)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_quoted_only_when_needed() {
        assert_eq!(
            command_line("/home/me/Apps/CargoFrame.AppImage", "bug"),
            "/home/me/Apps/CargoFrame.AppImage open --trigger bug"
        );
        assert_eq!(
            command_line("/home/me/My Apps/cargoframe", "it's"),
            r"'/home/me/My Apps/cargoframe' open --trigger 'it'\''s'"
        );
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a;rm -rf"), "'a;rm -rf'");
    }

    #[test]
    fn the_appimage_is_the_program_when_there_is_one() {
        let exe = Path::new("/tmp/.mount_Cargo123/usr/bin/cargoframe");
        assert_eq!(
            program(Some("/home/me/CargoFrame.AppImage"), exe),
            "/home/me/CargoFrame.AppImage"
        );
        assert_eq!(
            program(None, exe),
            "/tmp/.mount_Cargo123/usr/bin/cargoframe"
        );
        assert_eq!(
            program(Some(" "), exe),
            "/tmp/.mount_Cargo123/usr/bin/cargoframe"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn each_desktop_gets_the_route_it_supports() {
        use cargoframe_capture::wayland::Compositor;
        let none = Abilities::default();
        let portal = Abilities {
            portal: true,
            ..Abilities::default()
        };
        let gnome = Abilities {
            gnome_settings: true,
            portal: true,
        };
        assert_eq!(
            choose(
                &Compositor::Sway {
                    socket: "/s".into()
                },
                &none
            ),
            Route::Sway
        );
        assert_eq!(
            choose(
                &Compositor::Hyprland {
                    socket: "/h".into()
                },
                &portal
            ),
            Route::Hyprland
        );
        assert_eq!(choose(&Compositor::Gnome, &gnome), Route::Gnome);
        assert_eq!(choose(&Compositor::Gnome, &portal), Route::Portal);
        assert_eq!(choose(&Compositor::Kde, &portal), Route::Portal);
        assert_eq!(choose(&Compositor::Kde, &none), Route::Manual);
        assert_eq!(choose(&Compositor::Other, &none), Route::Manual);
        assert!(Route::Manual.note().unwrap().contains("command"));
        assert_eq!(Route::System.note(), None);
    }
}
