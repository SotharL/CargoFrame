//! Making a picture fit what a service accepts: under a size, and in a format it takes.
//!
//! A screenshot of a large screen is a PNG of several megabytes, more than Bluesky (about 1 MB) or
//! X (5 MB) take. Rather than fail, the picture is made smaller, half the width and height at a
//! time, until it fits. Text stays sharp because it stays a PNG; only Instagram, which takes JPEG
//! alone, gets a JPEG.

use crate::payload::Attachment;

/// The smallest picture worth sending: below this, the picture is refused rather than shrunk.
const MIN_SIDE: u32 = 240;

/// A picture as plain pixels, 8 bits per channel, red, green, blue and alpha.
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn decode_png(data: &[u8]) -> Result<Pixels, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    // 16-bit pictures become 8-bit, palettes become colours
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("the picture could not be read ({e})"))?;
    let size = reader
        .output_buffer_size()
        .ok_or("the picture is too large to read")?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("the picture could not be read ({e})"))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("the picture's colours could not be read".into()),
    };
    Ok(Pixels {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn encode_png(p: &Pixels) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, p.width, p.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("the picture could not be written ({e})"))?;
        writer
            .write_image_data(&p.rgba)
            .map_err(|e| format!("the picture could not be written ({e})"))?;
    }
    Ok(out)
}

/// Half the width and height: each new pixel is the average of four.
fn half(p: &Pixels) -> Pixels {
    let (w, h) = ((p.width / 2).max(1), (p.height / 2).max(1));
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let at = |x: u32, y: u32, c: usize| -> u32 {
        let x = x.min(p.width - 1);
        let y = y.min(p.height - 1);
        u32::from(p.rgba[((y * p.width + x) * 4) as usize + c])
    };
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let sum = at(2 * x, 2 * y, c)
                    + at(2 * x + 1, 2 * y, c)
                    + at(2 * x, 2 * y + 1, c)
                    + at(2 * x + 1, 2 * y + 1, c);
                rgba.push(((sum + 2) / 4) as u8);
            }
        }
    }
    Pixels {
        width: w,
        height: h,
        rgba,
    }
}

/// Width and height from a PNG's first chunk (`IHDR`), or `None` when it is not there.
fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[12..16] != b"IHDR" {
        return None;
    }
    let num = |at: usize| u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    Some((num(16), num(20)))
}

/// A picture that fits in `max_bytes`, with its width and height. A PNG already small enough is
/// returned as it is; a larger one is made smaller until it fits. Anything but a PNG is returned
/// as it is when small enough and refused when not, with a sentence that says why.
pub fn fit(picture: &Attachment, max_bytes: usize, service: &str) -> Result<Fitted, String> {
    let is_png = picture.data.starts_with(b"\x89PNG");
    if picture.data.len() <= max_bytes && !is_png {
        return Ok(Fitted {
            picture: picture.clone(),
            width: None,
            height: None,
            shrunk: false,
        });
    }
    if !is_png {
        return Err(format!(
            "The picture is {} and {service} takes at most {}",
            size_text(picture.data.len()),
            size_text(max_bytes)
        ));
    }
    if picture.data.len() <= max_bytes {
        // small enough: sent as it is, its size read from the header without decoding it
        let (width, height) = png_size(&picture.data).unzip();
        return Ok(Fitted {
            picture: picture.clone(),
            width,
            height,
            shrunk: false,
        });
    }
    let mut pixels = decode_png(&picture.data)?;
    loop {
        if pixels.width / 2 < MIN_SIDE && pixels.height / 2 < MIN_SIDE {
            return Err(format!(
                "The picture cannot be made small enough for {service} ({} at most)",
                size_text(max_bytes)
            ));
        }
        pixels = half(&pixels);
        let data = encode_png(&pixels)?;
        if data.len() <= max_bytes {
            return Ok(Fitted {
                picture: Attachment {
                    name: picture.name.clone(),
                    mime: "image/png".into(),
                    data,
                },
                width: Some(pixels.width),
                height: Some(pixels.height),
                shrunk: true,
            });
        }
    }
}

/// A picture made to fit, and its size when it was read.
#[derive(Debug, Clone)]
pub struct Fitted {
    pub picture: Attachment,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// It was made smaller than it was captured.
    pub shrunk: bool,
}

/// The picture as a JPEG of at most `max_bytes` (Instagram takes nothing else). Transparent parts
/// become white.
#[cfg(feature = "instagram")]
pub fn to_jpeg(picture: &Attachment, max_bytes: usize) -> Result<Fitted, String> {
    use jpeg_encoder::{ColorType, Encoder};
    let mut pixels = decode_png(&picture.data)?;
    let mut shrunk = false;
    loop {
        let rgb: Vec<u8> = pixels
            .rgba
            .chunks_exact(4)
            .flat_map(|p| {
                let a = u32::from(p[3]);
                let over_white = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
                [over_white(p[0]), over_white(p[1]), over_white(p[2])]
            })
            .collect();
        let mut out = Vec::new();
        let width = u16::try_from(pixels.width).map_err(|_| "the picture is too wide")?;
        let height = u16::try_from(pixels.height).map_err(|_| "the picture is too tall")?;
        Encoder::new(&mut out, 90)
            .encode(&rgb, width, height, ColorType::Rgb)
            .map_err(|e| format!("the picture could not be made a JPEG ({e})"))?;
        if out.len() <= max_bytes {
            let stem = picture
                .name
                .rsplit_once('.')
                .map_or(picture.name.as_str(), |(s, _)| s);
            return Ok(Fitted {
                picture: Attachment {
                    name: format!("{stem}.jpg"),
                    mime: "image/jpeg".into(),
                    data: out,
                },
                width: Some(pixels.width),
                height: Some(pixels.height),
                shrunk,
            });
        }
        if pixels.width / 2 < MIN_SIDE && pixels.height / 2 < MIN_SIDE {
            return Err(format!(
                "The picture cannot be made small enough ({} at most)",
                size_text(max_bytes)
            ));
        }
        pixels = half(&pixels);
        shrunk = true;
    }
}

fn size_text(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} KB", bytes.div_ceil(1000))
    }
}

/// The text with the formatting marks taken out (for services that show plain text only), and
/// at most `max` characters long, cut at a word with "…" when it had to be cut.
pub fn plain_text(text: &str, max: usize) -> (String, bool) {
    let plain = crate::richtext::render(text, crate::richtext::Style::Plain);
    let plain = plain.trim().to_string();
    if plain.chars().count() <= max {
        return (plain, false);
    }
    let cut: String = plain.chars().take(max.saturating_sub(1)).collect();
    let at_word = cut
        .rfind(char::is_whitespace)
        .filter(|&i| i > cut.len() / 2)
        .map_or(cut.as_str(), |i| &cut[..i]);
    (format!("{}…", at_word.trim_end()), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A noisy picture, so it does not compress to nothing.
    fn picture(w: u32, h: u32) -> Attachment {
        let mut seed = 7u32;
        let rgba: Vec<u8> = (0..w * h * 4)
            .map(|i| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                if i % 4 == 3 {
                    255
                } else {
                    (seed >> 16) as u8
                }
            })
            .collect();
        let data = encode_png(&Pixels {
            width: w,
            height: h,
            rgba,
        })
        .unwrap();
        Attachment {
            name: "screenshot.png".into(),
            mime: "image/png".into(),
            data,
        }
    }

    #[test]
    fn a_small_picture_is_sent_as_it_is() {
        let p = picture(100, 80);
        let f = fit(&p, 1_000_000, "Bluesky").unwrap();
        assert_eq!(f.picture.data, p.data);
        assert!(!f.shrunk);
        assert_eq!((f.width, f.height), (Some(100), Some(80)));
    }

    #[test]
    fn a_big_picture_is_halved_until_it_fits() {
        let p = picture(1200, 800);
        let limit = p.data.len() / 3;
        let f = fit(&p, limit, "Bluesky").unwrap();
        assert!(f.picture.data.len() <= limit);
        assert!(f.shrunk);
        assert_eq!(f.width, Some(600));
        assert_eq!(f.height, Some(400));
        assert_eq!(f.picture.mime, "image/png");
        // and it is still a picture that can be read
        assert_eq!(decode_png(&f.picture.data).unwrap().width, 600);
    }

    #[test]
    fn a_picture_that_cannot_fit_is_refused_with_the_limit() {
        let p = picture(300, 300);
        let e = fit(&p, 100, "Bluesky").unwrap_err();
        assert!(e.contains("Bluesky") && e.contains("1 KB"), "{e}");
    }

    #[test]
    fn something_that_is_not_a_png_is_only_checked_for_size() {
        let p = Attachment {
            name: "a.jpg".into(),
            mime: "image/jpeg".into(),
            data: vec![0xFF, 0xD8, 1, 2, 3],
        };
        assert!(fit(&p, 10, "X").is_ok());
        let e = fit(&p, 2, "X").unwrap_err();
        assert!(e.contains("X takes at most"), "{e}");
    }

    #[cfg(feature = "instagram")]
    #[test]
    fn a_jpeg_is_made_for_a_service_that_takes_nothing_else() {
        let p = picture(1000, 800);
        let j = to_jpeg(&p, 8_000_000).unwrap();
        assert!(j.picture.data.starts_with(&[0xFF, 0xD8]));
        assert_eq!(j.picture.mime, "image/jpeg");
        assert_eq!(j.picture.name, "screenshot.jpg");
        let small = to_jpeg(&p, j.picture.data.len() / 3).unwrap();
        assert!(small.shrunk);
    }

    #[test]
    fn plain_text_drops_the_marks_and_cuts_at_a_word() {
        let (t, cut) = plain_text("It **crashes** on `save`", 300);
        assert_eq!(t, "It crashes on save");
        assert!(!cut);
        let (t, cut) = plain_text("one two three four five", 12);
        assert!(cut);
        assert_eq!(t, "one two…");
        assert!(t.chars().count() <= 12);
    }
}
