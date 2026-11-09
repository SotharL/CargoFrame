//! Windows: the system's own text reader (`Windows.Media.Ocr`), in the languages of the person's
//! profile. It runs on a thread of its own, as it waits for the reading to finish.

use crate::prepare::Gray;
use crate::{Line, OcrError};
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

fn failed(e: windows::core::Error) -> OcrError {
    OcrError::Failed(e.message().to_string())
}

pub fn read(image: &Gray) -> Result<Vec<Line>, OcrError> {
    let image = image.clone();
    std::thread::Builder::new()
        .name("read-text".into())
        .spawn(move || read_here(&image))
        .map_err(|e| OcrError::Failed(e.to_string()))?
        .join()
        .map_err(|_| OcrError::Failed("the text reader stopped".into()))?
}

fn read_here(image: &Gray) -> Result<Vec<Line>, OcrError> {
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| {
        OcrError::NoReader(
            "Windows has no text reader for your languages: add one in Settings › Time & language › Language".into(),
        )
    })?;
    // the reader takes colour pixels (blue, green, red, alpha)
    let mut bgra = Vec::with_capacity(image.px.len() * 4);
    for &p in &image.px {
        bgra.extend_from_slice(&[p, p, p, 255]);
    }
    let writer = DataWriter::new().map_err(failed)?;
    writer.WriteBytes(&bgra).map_err(failed)?;
    let buffer = writer.DetachBuffer().map_err(failed)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.w as i32,
        image.h as i32,
    )
    .map_err(failed)?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(failed)?
        .join()
        .map_err(failed)?;
    let lines = result.Lines().map_err(failed)?;
    let mut out = Vec::new();
    for i in 0..lines.Size().map_err(failed)? {
        let line = lines.GetAt(i).map_err(failed)?;
        let words = line.Words().map_err(failed)?;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, 0f32, 0f32);
        for k in 0..words.Size().map_err(failed)? {
            let r = words
                .GetAt(k)
                .map_err(failed)?
                .BoundingRect()
                .map_err(failed)?;
            x0 = x0.min(r.X);
            y0 = y0.min(r.Y);
            x1 = x1.max(r.X + r.Width);
            y1 = y1.max(r.Y + r.Height);
        }
        if x0 > x1 {
            continue;
        }
        out.push(Line {
            text: line.Text().map_err(failed)?.to_string(),
            x: x0.max(0.0) as u32,
            y: y0.max(0.0) as u32,
            w: (x1 - x0).ceil() as u32,
            h: (y1 - y0).ceil() as u32,
        });
    }
    Ok(out)
}
