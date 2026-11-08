//! Find the program in focus and take its screenshot.
//!
//! The platform code sits behind the [`Backend`] trait; [`capture_with`] holds the rules (focused
//! window, else the display) and is tested with a fake backend. [`capture`] picks the real
//! backend for this system:
//!
//! * Windows: the foreground window and a desktop copy.
//! * Linux on X11: the window manager's active window and the root window's pixels.
//! * Linux on Wayland: `grim` or the desktop portal for the picture, and the compositor (sway,
//!   Hyprland, KDE) or XWayland for the window in focus. See [`wayland`].

pub mod error;
pub mod frame;
pub mod policy;
pub mod session;

#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub mod screencast;
#[cfg(target_os = "linux")]
pub mod wayland;
#[cfg(windows)]
mod win;
#[cfg(target_os = "linux")]
pub mod x11;

pub use error::CaptureError;
pub use frame::{Frame, PixelRect};
pub use policy::{capture_with, Backend, Captured, FocusedApp, Mode, Subject};

/// Capture with this system's backend.
pub fn capture(mode: Mode) -> Result<Captured, CaptureError> {
    #[cfg(windows)]
    {
        policy::capture_with(&win::Windows, mode)
    }
    #[cfg(target_os = "linux")]
    {
        match session::detect_env() {
            session::Session::Wayland => policy::capture_with(
                &wayland::Wayland::new(wayland::Compositor::detect_env()),
                mode,
            ),
            session::Session::X11 => policy::capture_with(&x11::X11::connect()?, mode),
            session::Session::Other => Err(CaptureError::NoDisplay(
                "this is not a desktop session (no DISPLAY or WAYLAND_DISPLAY)".into(),
            )),
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = mode;
        Err(CaptureError::Unsupported(
            "this system is not supported yet".into(),
        ))
    }
}

/// The window in focus right now, without taking a picture. `None` when nothing has the focus.
/// On a Wayland desktop that does not say (GNOME), the error is [`CaptureError::FocusUnknown`].
pub fn focused_app() -> Result<Option<FocusedApp>, CaptureError> {
    #[cfg(windows)]
    {
        policy::Backend::focused(&win::Windows)
    }
    #[cfg(target_os = "linux")]
    {
        match session::detect_env() {
            session::Session::X11 => policy::Backend::focused(&x11::X11::connect()?),
            session::Session::Wayland => wayland::focused_app(&wayland::Compositor::detect_env()),
            session::Session::Other => Ok(None),
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(None)
    }
}
