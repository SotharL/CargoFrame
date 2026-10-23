//! Pixels and rectangles. A `Frame` is always RGBA, 8 bits per channel, top row first.

use crate::error::CaptureError;

/// A rectangle in pixels. `x`/`y` may be negative (displays left of or above the primary one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl PixelRect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.w)
    }

    pub fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.h)
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        i64::from(px) >= i64::from(self.x)
            && i64::from(px) < self.right()
            && i64::from(py) >= i64::from(self.y)
            && i64::from(py) < self.bottom()
    }

    /// The part of `self` that lies inside `other`, if any.
    pub fn intersect(&self, other: &PixelRect) -> Option<PixelRect> {
        let left = i64::from(self.x).max(i64::from(other.x));
        let top = i64::from(self.y).max(i64::from(other.y));
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= left || bottom <= top {
            return None;
        }
        Some(PixelRect {
            x: i32::try_from(left).ok()?,
            y: i32::try_from(top).ok()?,
            w: u32::try_from(right - left).ok()?,
            h: u32::try_from(bottom - top).ok()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Frame {
    /// `rgba` must hold exactly `width * height * 4` bytes, and the picture cannot be empty.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, CaptureError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| CaptureError::Image("picture is too large".into()))?;
        if width == 0 || height == 0 {
            return Err(CaptureError::Image("picture is empty".into()));
        }
        if rgba.len() != expected {
            return Err(CaptureError::Image(format!(
                "expected {expected} bytes for {width}x{height}, got {}",
                rgba.len()
            )));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    /// RGBA of one pixel; `None` outside the picture.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        Some([
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ])
    }

    /// The part of the picture inside `area` (relative to the picture's top-left corner).
    /// Anything outside the picture is dropped; `None` if nothing is left.
    pub fn crop(&self, area: PixelRect) -> Option<Frame> {
        let whole = PixelRect::new(0, 0, self.width, self.height);
        let r = area.intersect(&whole)?;
        let (x, y, w, h) = (r.x as usize, r.y as usize, r.w as usize, r.h as usize);
        let stride = self.width as usize * 4;
        let mut out = Vec::with_capacity(w * h * 4);
        for row in y..y + h {
            let start = row * stride + x * 4;
            out.extend_from_slice(&self.rgba[start..start + w * 4]);
        }
        Frame::new(r.w, r.h, out).ok()
    }

    /// PNG bytes, tuned for speed over size: the picture is shown at once and only compressed
    /// harder when it is sent.
    pub fn to_png(&self) -> Result<Vec<u8>, CaptureError> {
        self.encode_png(png::Compression::Fast)
    }

    /// Smaller PNG for sharing and zipping.
    pub fn to_png_small(&self) -> Result<Vec<u8>, CaptureError> {
        self.encode_png(png::Compression::High)
    }

    fn encode_png(&self, compression: png::Compression) -> Result<Vec<u8>, CaptureError> {
        let mut out = Vec::with_capacity(self.rgba.len() / 4);
        let mut enc = png::Encoder::new(&mut out, self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(compression);
        enc.set_filter(png::Filter::Sub);
        let mut writer = enc
            .write_header()
            .map_err(|e| CaptureError::Image(e.to_string()))?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|e| CaptureError::Image(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| CaptureError::Image(e.to_string()))?;
        Ok(out)
    }

    /// Decode an 8-bit PNG (RGB, RGBA, gray or gray+alpha). Used for portal screenshots.
    pub fn from_png(bytes: &[u8]) -> Result<Frame, CaptureError> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder
            .read_info()
            .map_err(|e| CaptureError::Image(e.to_string()))?;
        let mut buf = vec![
            0;
            reader.output_buffer_size().ok_or_else(|| {
                CaptureError::Image("picture is too large".into())
            })?
        ];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| CaptureError::Image(e.to_string()))?;
        buf.truncate(info.buffer_size());
        let pixels = (info.width as usize) * (info.height as usize);
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::Rgb => {
                let mut out = Vec::with_capacity(pixels * 4);
                for c in buf.chunks_exact(3) {
                    out.extend_from_slice(&[c[0], c[1], c[2], 255]);
                }
                out
            }
            png::ColorType::Grayscale => {
                let mut out = Vec::with_capacity(pixels * 4);
                for &g in &buf {
                    out.extend_from_slice(&[g, g, g, 255]);
                }
                out
            }
            png::ColorType::GrayscaleAlpha => {
                let mut out = Vec::with_capacity(pixels * 4);
                for c in buf.chunks_exact(2) {
                    out.extend_from_slice(&[c[0], c[0], c[0], c[1]]);
                }
                out
            }
            png::ColorType::Indexed => {
                return Err(CaptureError::Image("indexed PNG was not expanded".into()))
            }
        };
        Frame::new(info.width, info.height, rgba)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Frame {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                v.extend_from_slice(&[x as u8, y as u8, (x + y) as u8, 255]);
            }
        }
        Frame::new(w, h, v).unwrap()
    }

    #[test]
    fn rejects_wrong_sizes() {
        assert!(Frame::new(2, 2, vec![0; 15]).is_err());
        assert!(Frame::new(0, 2, vec![]).is_err());
        assert!(Frame::new(2, 2, vec![0; 16]).is_ok());
    }

    #[test]
    fn intersect_clamps_and_detects_no_overlap() {
        let desk = PixelRect::new(-100, 0, 300, 200);
        let win = PixelRect::new(-150, 50, 100, 400);
        assert_eq!(
            win.intersect(&desk),
            Some(PixelRect::new(-100, 50, 50, 150))
        );
        assert_eq!(PixelRect::new(500, 0, 10, 10).intersect(&desk), None);
        assert_eq!(PixelRect::new(-100, 200, 10, 10).intersect(&desk), None);
    }

    #[test]
    fn contains_is_half_open() {
        let r = PixelRect::new(10, 10, 5, 5);
        assert!(r.contains(10, 10) && r.contains(14, 14));
        assert!(!r.contains(15, 14) && !r.contains(9, 10));
    }

    #[test]
    fn crop_copies_the_right_pixels_and_clamps() {
        let f = gradient(8, 6);
        let c = f.crop(PixelRect::new(2, 1, 3, 2)).unwrap();
        assert_eq!((c.width(), c.height()), (3, 2));
        assert_eq!(c.pixel(0, 0), f.pixel(2, 1));
        assert_eq!(c.pixel(2, 1), f.pixel(4, 2));
        let edge = f.crop(PixelRect::new(6, 4, 10, 10)).unwrap();
        assert_eq!((edge.width(), edge.height()), (2, 2));
        assert!(f.crop(PixelRect::new(8, 0, 1, 1)).is_none());
    }

    #[test]
    fn png_round_trip_is_lossless() {
        let f = gradient(37, 23);
        let back = Frame::from_png(&f.to_png().unwrap()).unwrap();
        assert_eq!(back, f);
        let small = Frame::from_png(&f.to_png_small().unwrap()).unwrap();
        assert_eq!(small, f);
    }

    #[test]
    fn png_without_alpha_gets_an_opaque_channel() {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 1);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
        }
        let f = Frame::from_png(&out).unwrap();
        assert_eq!(f.pixel(0, 0), Some([1, 2, 3, 255]));
        assert_eq!(f.pixel(1, 0), Some([4, 5, 6, 255]));
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(Frame::from_png(b"not a png").is_err());
    }
}
