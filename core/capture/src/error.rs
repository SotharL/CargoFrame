use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    /// No screen to capture from: no X server, no desktop session.
    #[error("no display is available: {0}")]
    NoDisplay(String),
    /// The system refused or failed to hand over the pixels.
    #[error("the screen could not be read: {0}")]
    Grab(String),
    /// The screen uses a pixel format this build does not understand.
    #[error("unsupported screen format: {0}")]
    Unsupported(String),
    /// A picture was malformed.
    #[error("invalid picture: {0}")]
    Image(String),
    /// The desktop does not tell other programs which window is in focus. The text says so in
    /// the person's words and becomes the window's note.
    #[error("{0}")]
    FocusUnknown(String),
    /// The person (or the desktop's privacy settings) said no.
    #[error("the screenshot was cancelled or not allowed")]
    Refused,
}

pub(crate) fn grab<E: Display>(e: E) -> CaptureError {
    CaptureError::Grab(e.to_string())
}
