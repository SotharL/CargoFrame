//! While CargoFrame records, it shows it: a red outline around what is recorded and a red dot
//! that blinks once a second in a corner. They are drawn by the operating system directly (no
//! web page), cannot be clicked, never take the focus, and are kept out of the recording:
//!
//! - Windows hides them from every screen capture, the recording included.
//! - On Linux (X11) they sit just outside a recorded window, so only a whole-screen recording
//!   can show them; the settings say so.
//! - Wayland desktops do not let a program place windows like this; they show their own sign
//!   that the screen is shared.
//!
//! [`Indicator::show`] puts them up; dropping the [`Indicator`] takes them down.

mod geometry;
#[cfg(windows)]
#[allow(unsafe_code)]
mod win;
#[cfg(target_os = "linux")]
mod x11;

pub use geometry::{dot_at, frame_bands, layout, round_spans, Corner, Layout, Rect, DOT, EDGE};

/// What to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub outline: bool,
    pub dot: bool,
    pub corner: Corner,
}

/// The outline and dot on screen, until dropped.
pub struct Indicator {
    #[cfg(target_os = "linux")]
    inner: Option<x11::Shown>,
    #[cfg(windows)]
    inner: Option<win::Shown>,
}

impl Indicator {
    /// Show `look` around `area` (desktop pixels), on `screen` (the desktop or monitor it is
    /// on). Nothing to show, or a desktop that cannot show it, gives an indicator that shows
    /// nothing: recording goes on either way.
    pub fn show(area: Rect, screen: Rect, look: Look) -> Indicator {
        let lay = layout(area, screen, look.outline, look.dot, look.corner);
        if lay.bands.is_empty() && lay.dot.is_none() {
            return Indicator::none();
        }
        #[cfg(target_os = "linux")]
        {
            let inner = if std::env::var_os("WAYLAND_DISPLAY").is_some()
                && std::env::var_os("DISPLAY").is_none()
            {
                None
            } else {
                match x11::Shown::start(&lay) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        tracing::warn!("the recording sign could not be shown: {e}");
                        None
                    }
                }
            };
            Indicator { inner }
        }
        #[cfg(windows)]
        {
            let inner = match win::Shown::start(&lay) {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!("the recording sign could not be shown: {e}");
                    None
                }
            };
            Indicator { inner }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = lay;
            Indicator {}
        }
    }

    /// An indicator that shows nothing.
    pub fn none() -> Indicator {
        #[cfg(any(windows, target_os = "linux"))]
        {
            Indicator { inner: None }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Indicator {}
        }
    }

    /// Whether anything is on screen.
    pub fn showing(&self) -> bool {
        #[cfg(any(windows, target_os = "linux"))]
        {
            self.inner.is_some()
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            false
        }
    }
}
