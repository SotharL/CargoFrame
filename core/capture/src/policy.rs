//! What gets captured, and what happens when the first choice is not possible.
//!
//! The rule from the brief: capture the program in focus; if there is none (or it cannot be
//! read), capture the display instead. The result says what was really captured so the window
//! can show a note.

use crate::error::CaptureError;
use crate::frame::{Frame, PixelRect};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The window in focus; the display when there is none.
    ActiveWindow,
    /// The display under the pointer.
    Display,
    /// Every display as one picture.
    AllDisplays,
}

/// What the picture shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Window,
    Display,
    AllDisplays,
}

/// The program that had the focus when the shortcut was pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedApp {
    /// Operating-system window handle (HWND or X11 window id), for diagnostics.
    pub window: u64,
    /// Full path of the program's executable, when the system will tell.
    pub exe_path: Option<PathBuf>,
    pub title: String,
    pub pid: Option<u32>,
    /// Where the window is, in desktop coordinates.
    pub bounds: PixelRect,
}

#[derive(Debug)]
pub struct Captured {
    pub frame: Frame,
    pub app: Option<FocusedApp>,
    pub subject: Subject,
    /// Plain-language remarks about fallbacks, shown as notes in the window.
    pub notes: Vec<String>,
}

/// One operating system's way of finding the focus and reading pixels.
pub trait Backend {
    /// The window in focus. `None` when nothing has the focus or the system cannot tell
    /// (Wayland does not let programs ask).
    fn focused(&self) -> Result<Option<FocusedApp>, CaptureError>;
    /// The display the pointer is on, if known.
    fn pointer_display(&self) -> Result<Option<PixelRect>, CaptureError>;
    /// All displays together, in desktop coordinates.
    fn desktop(&self) -> Result<PixelRect, CaptureError>;
    /// The pixels of `area` (desktop coordinates, already inside `desktop()`).
    fn grab(&self, area: PixelRect) -> Result<Frame, CaptureError>;
}

/// Smallest window worth a screenshot; anything tinier is a tray stub or a hidden helper.
const MIN_SIDE: u32 = 16;

pub fn capture_with(backend: &dyn Backend, mode: Mode) -> Result<Captured, CaptureError> {
    let desktop = backend.desktop()?;
    let mut notes = Vec::new();
    let mut app = None;

    let display_or_desktop = |notes: &mut Vec<String>| match backend.pointer_display() {
        Ok(Some(r)) => match r.intersect(&desktop) {
            Some(r) => (r, Subject::Display),
            None => (desktop, Subject::AllDisplays),
        },
        Ok(None) => (desktop, Subject::AllDisplays),
        Err(e) => {
            tracing::debug!("pointer display unknown: {e}");
            notes.push(
                "The display under the pointer is unknown, so every display was captured.".into(),
            );
            (desktop, Subject::AllDisplays)
        }
    };

    let (area, subject) = match mode {
        Mode::AllDisplays => (desktop, Subject::AllDisplays),
        Mode::Display => display_or_desktop(&mut notes),
        Mode::ActiveWindow => match backend.focused() {
            Ok(Some(found)) => {
                let visible = found
                    .bounds
                    .intersect(&desktop)
                    .filter(|r| r.w >= MIN_SIDE && r.h >= MIN_SIDE);
                match visible {
                    Some(r) => {
                        app = Some(found);
                        (r, Subject::Window)
                    }
                    None => {
                        notes.push(format!(
                            "'{}' is not visible on screen, so the display was captured.",
                            found.title
                        ));
                        app = Some(found);
                        display_or_desktop(&mut notes)
                    }
                }
            }
            Ok(None) => {
                notes.push("No window has the focus, so the display was captured.".into());
                display_or_desktop(&mut notes)
            }
            Err(e) => {
                tracing::debug!("focused window unknown: {e}");
                notes.push(
                    "The window in focus could not be read, so the display was captured.".into(),
                );
                display_or_desktop(&mut notes)
            }
        },
    };

    let frame = backend.grab(area)?;
    Ok(Captured {
        frame,
        app,
        subject,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake {
        focused: Result<Option<FocusedApp>, &'static str>,
        pointer: Result<Option<PixelRect>, &'static str>,
        desktop: PixelRect,
        grabbed: RefCell<Vec<PixelRect>>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                focused: Ok(None),
                pointer: Ok(None),
                desktop: PixelRect::new(0, 0, 1920, 1080),
                grabbed: RefCell::new(Vec::new()),
            }
        }
    }

    impl Backend for Fake {
        fn focused(&self) -> Result<Option<FocusedApp>, CaptureError> {
            self.focused
                .clone()
                .map_err(|e| CaptureError::Grab(e.into()))
        }
        fn pointer_display(&self) -> Result<Option<PixelRect>, CaptureError> {
            self.pointer.map_err(|e| CaptureError::Grab(e.into()))
        }
        fn desktop(&self) -> Result<PixelRect, CaptureError> {
            Ok(self.desktop)
        }
        fn grab(&self, area: PixelRect) -> Result<Frame, CaptureError> {
            self.grabbed.borrow_mut().push(area);
            Frame::new(
                area.w,
                area.h,
                vec![0; area.w as usize * area.h as usize * 4],
            )
        }
    }

    fn app(bounds: PixelRect) -> FocusedApp {
        FocusedApp {
            window: 7,
            exe_path: Some("/opt/app/app".into()),
            title: "Editor".into(),
            pid: Some(42),
            bounds,
        }
    }

    #[test]
    fn active_window_captures_the_window_and_keeps_the_app() {
        let mut f = Fake::new();
        f.focused = Ok(Some(app(PixelRect::new(100, 100, 800, 600))));
        let c = capture_with(&f, Mode::ActiveWindow).unwrap();
        assert_eq!(c.subject, Subject::Window);
        assert_eq!(c.app.unwrap().title, "Editor");
        assert_eq!(
            *f.grabbed.borrow(),
            vec![PixelRect::new(100, 100, 800, 600)]
        );
        assert!(c.notes.is_empty());
    }

    #[test]
    fn a_window_hanging_off_screen_is_clamped() {
        let mut f = Fake::new();
        f.focused = Ok(Some(app(PixelRect::new(1500, 900, 800, 600))));
        capture_with(&f, Mode::ActiveWindow).unwrap();
        assert_eq!(
            *f.grabbed.borrow(),
            vec![PixelRect::new(1500, 900, 420, 180)]
        );
    }

    #[test]
    fn no_focus_falls_back_to_the_pointer_display_with_a_note() {
        let mut f = Fake::new();
        f.pointer = Ok(Some(PixelRect::new(0, 0, 960, 1080)));
        let c = capture_with(&f, Mode::ActiveWindow).unwrap();
        assert_eq!(c.subject, Subject::Display);
        assert!(c.app.is_none());
        assert_eq!(c.notes.len(), 1);
        assert_eq!(*f.grabbed.borrow(), vec![PixelRect::new(0, 0, 960, 1080)]);
    }

    #[test]
    fn unknown_focus_and_pointer_fall_back_to_everything() {
        let mut f = Fake::new();
        f.focused = Err("boom");
        f.pointer = Err("boom");
        let c = capture_with(&f, Mode::ActiveWindow).unwrap();
        assert_eq!(c.subject, Subject::AllDisplays);
        assert_eq!(c.notes.len(), 2);
    }

    #[test]
    fn an_invisible_window_falls_back_but_the_app_is_kept_for_context() {
        let mut f = Fake::new();
        f.focused = Ok(Some(app(PixelRect::new(-32000, -32000, 160, 28))));
        let c = capture_with(&f, Mode::ActiveWindow).unwrap();
        assert_eq!(c.subject, Subject::AllDisplays);
        assert_eq!(c.app.unwrap().pid, Some(42));
        assert!(c.notes[0].contains("not visible"));
    }

    #[test]
    fn tiny_windows_are_not_worth_a_screenshot() {
        let mut f = Fake::new();
        f.focused = Ok(Some(app(PixelRect::new(10, 10, 8, 8))));
        let c = capture_with(&f, Mode::ActiveWindow).unwrap();
        assert_eq!(c.subject, Subject::AllDisplays);
    }

    #[test]
    fn display_and_all_displays_modes_ignore_the_focus() {
        let mut f = Fake::new();
        f.focused = Ok(Some(app(PixelRect::new(0, 0, 100, 100))));
        f.pointer = Ok(Some(PixelRect::new(1920, 0, 1920, 1080))); // outside the desktop
        let c = capture_with(&f, Mode::Display).unwrap();
        assert_eq!(c.subject, Subject::AllDisplays);
        assert!(c.app.is_none());
        let c = capture_with(&f, Mode::AllDisplays).unwrap();
        assert_eq!(c.subject, Subject::AllDisplays);
        assert!(c.notes.is_empty());
    }
}
