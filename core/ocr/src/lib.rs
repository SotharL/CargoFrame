//! Read the text in part of a screenshot, on this computer: nothing is sent anywhere.
//!
//! Windows has a text reader of its own (`Windows.Media.Ocr`, in every Windows 10 and 11); on
//! Linux the `tesseract` program is used when it is installed. Before reading, the picture is made
//! easier to read ([`prepare`]): grey, larger when the text is small, light text on dark turned
//! dark on light. Each try ("Try again") prepares it a little differently and reads it another
//! way, as one of them often does better on a given screen.

mod prepare;
#[cfg(target_os = "linux")]
mod tesseract;
#[cfg(windows)]
mod win;

pub use prepare::{prepare, Gray, Prepared};

/// The tries there are; a try past the last starts again from the first.
pub const TRIES: u32 = 3;

/// The longest side of a picture read (the system reader stops at about 2600 pixels).
pub const MAX_SIDE: u32 = 2600;

/// A line of text found, and where it is in the picture that was given (in its pixels).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Line {
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// What was read: the text (its lines, one per line) and each line's place.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Reading {
    pub text: String,
    pub lines: Vec<Line>,
    /// What read it, to say so ("Windows", "tesseract").
    pub reader: String,
    /// The try this was (0 for the first).
    pub attempt: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("the picture could not be read: {0}")]
    Picture(String),
    #[error("{0}")]
    NoReader(String),
    #[error("the text could not be read: {0}")]
    Failed(String),
}

/// Read the text in `png` (the part of the screenshot that was selected), the `attempt`-th way.
pub fn read(png: &[u8], attempt: u32) -> Result<Reading, OcrError> {
    let attempt = attempt % TRIES;
    let image = Gray::from_png(png).map_err(OcrError::Picture)?;
    let prepared = prepare(&image, attempt);
    let lines = read_prepared(&prepared, attempt)?;
    // back to the pixels of the picture that was given
    let f = prepared.scale;
    let lines: Vec<Line> = lines
        .into_iter()
        .filter(|l| !l.text.trim().is_empty())
        .map(|l| Line {
            text: l.text.trim().to_string(),
            x: (l.x as f32 / f) as u32,
            y: (l.y as f32 / f) as u32,
            w: ((l.w as f32 / f).ceil() as u32).max(1),
            h: ((l.h as f32 / f).ceil() as u32).max(1),
        })
        .collect();
    Ok(Reading {
        text: lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        lines,
        reader: READER.to_string(),
        attempt,
    })
}

#[cfg(windows)]
const READER: &str = "Windows";
#[cfg(target_os = "linux")]
const READER: &str = "tesseract";
#[cfg(not(any(windows, target_os = "linux")))]
const READER: &str = "none";

#[cfg(windows)]
fn read_prepared(p: &Prepared, _attempt: u32) -> Result<Vec<Line>, OcrError> {
    win::read(&p.image)
}

#[cfg(target_os = "linux")]
fn read_prepared(p: &Prepared, attempt: u32) -> Result<Vec<Line>, OcrError> {
    tesseract::read(&p.image, attempt)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn read_prepared(_: &Prepared, _: u32) -> Result<Vec<Line>, OcrError> {
    Err(OcrError::NoReader(
        "Reading text is not available on this system".into(),
    ))
}
