//! Make a picture easier to read: grey, larger when small, dark text on a light ground, and (for
//! the last try) only black and white.

use crate::MAX_SIDE;

/// A grey picture, one byte a pixel, row after row.
#[derive(Debug, Clone, PartialEq)]
pub struct Gray {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

/// A picture made ready to read, and how much larger than the one given it is.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub image: Gray,
    pub scale: f32,
}

/// Pictures larger than this are not read (a selection on a large screen stays well under).
const MAX_PIXELS: u64 = 40_000_000;

impl Gray {
    /// A PNG turned grey (any colour type, 8 or 16 bits).
    pub fn from_png(bytes: &[u8]) -> Result<Gray, String> {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = dec.read_info().map_err(|e| e.to_string())?;
        let (w, h) = {
            let i = reader.info();
            (i.width, i.height)
        };
        if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err(format!("a picture of {w}×{h} pixels cannot be read"));
        }
        let size = reader
            .output_buffer_size()
            .ok_or_else(|| "the picture is too large".to_string())?;
        let mut buf = vec![0; size];
        let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let data = &buf[..frame.buffer_size()];
        let n = (w * h) as usize;
        let chans = data.len() / n;
        let mut px = Vec::with_capacity(n);
        for i in 0..n {
            let p = &data[i * chans..i * chans + chans];
            let (g, a) = match chans {
                1 => (p[0] as f32, 255.0),
                2 => (p[0] as f32, p[1] as f32),
                3 => (luma(p[0], p[1], p[2]), 255.0),
                _ => (luma(p[0], p[1], p[2]), p[3] as f32),
            };
            // a see-through pixel is the white ground it would be read on
            let a = a / 255.0;
            px.push((g * a + 255.0 * (1.0 - a)).round() as u8);
        }
        Ok(Gray { w, h, px })
    }

    /// The picture as a grey PNG.
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.w, self.h);
            enc.set_color(png::ColorType::Grayscale);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().expect("a header in memory");
            w.write_image_data(&self.px).expect("a picture in memory");
        }
        out
    }

    fn at(&self, x: u32, y: u32) -> f32 {
        self.px[(y * self.w + x) as usize] as f32
    }

    /// The picture `f` times larger (or smaller), each pixel from the four nearest.
    fn scaled(&self, f: f32) -> Gray {
        let w = ((self.w as f32 * f).round() as u32).max(1);
        let h = ((self.h as f32 * f).round() as u32).max(1);
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let sy = ((y as f32 + 0.5) / f - 0.5).clamp(0.0, (self.h - 1) as f32);
            let (y0, ty) = (sy.floor() as u32, sy.fract());
            let y1 = (y0 + 1).min(self.h - 1);
            for x in 0..w {
                let sx = ((x as f32 + 0.5) / f - 0.5).clamp(0.0, (self.w - 1) as f32);
                let (x0, tx) = (sx.floor() as u32, sx.fract());
                let x1 = (x0 + 1).min(self.w - 1);
                let top = self.at(x0, y0) * (1.0 - tx) + self.at(x1, y0) * tx;
                let bottom = self.at(x0, y1) * (1.0 - tx) + self.at(x1, y1) * tx;
                px.push((top * (1.0 - ty) + bottom * ty).round() as u8);
            }
        }
        Gray { w, h, px }
    }

    fn mean(&self) -> f32 {
        self.px.iter().map(|&p| p as u64).sum::<u64>() as f32 / self.px.len() as f32
    }

    /// Light text on a dark ground (a dark theme) turned dark on light, which readers prefer.
    fn dark_on_light(&mut self) {
        if self.mean() < 128.0 {
            for p in &mut self.px {
                *p = 255 - *p;
            }
        }
    }

    /// The darkest and the lightest pixels (past a few odd ones) stretched to black and white.
    fn stretched(&mut self) {
        let mut hist = [0u32; 256];
        for &p in &self.px {
            hist[p as usize] += 1;
        }
        let cut = (self.px.len() / 100) as u32;
        let (mut lo, mut hi) = (0usize, 255usize);
        let mut seen = 0;
        for (i, n) in hist.iter().enumerate() {
            seen += n;
            if seen > cut {
                lo = i;
                break;
            }
        }
        seen = 0;
        for (i, n) in hist.iter().enumerate().rev() {
            seen += n;
            if seen > cut {
                hi = i;
                break;
            }
        }
        if hi <= lo + 10 {
            return;
        }
        let span = (hi - lo) as f32;
        for p in &mut self.px {
            *p = (((*p as f32 - lo as f32) / span) * 255.0).clamp(0.0, 255.0) as u8;
        }
    }

    /// Only black and white, cut where the two sides differ most (Otsu's way).
    fn black_and_white(&mut self) {
        let mut hist = [0u64; 256];
        for &p in &self.px {
            hist[p as usize] += 1;
        }
        let total = self.px.len() as f64;
        let sum: f64 = hist
            .iter()
            .enumerate()
            .map(|(i, n)| i as f64 * *n as f64)
            .sum();
        let (mut below, mut below_sum, mut best, mut cut) = (0f64, 0f64, -1f64, 128u8);
        for (t, n) in hist.iter().enumerate() {
            below += *n as f64;
            if below == 0.0 {
                continue;
            }
            let above = total - below;
            if above == 0.0 {
                break;
            }
            below_sum += t as f64 * *n as f64;
            let m0 = below_sum / below;
            let m1 = (sum - below_sum) / above;
            let between = below * above * (m0 - m1) * (m0 - m1);
            if between > best {
                best = between;
                cut = t as u8;
            }
        }
        for p in &mut self.px {
            *p = if *p > cut { 255 } else { 0 };
        }
    }
}

const fn luma(r: u8, g: u8, b: u8) -> f32 {
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

/// How much larger to make a picture for the `attempt`-th try: small text reads better large.
fn scale_for(image: &Gray, attempt: u32) -> f32 {
    let want: f32 = match attempt {
        0 => 2.0,
        1 => 3.0,
        _ => 2.5,
    };
    // a large selection already has large enough text
    let want = if image.h > 600 { want.min(1.5) } else { want };
    let longest = image.w.max(image.h) as f32;
    want.min(MAX_SIDE as f32 / longest).max(0.1)
}

/// The picture made ready for the `attempt`-th try.
pub fn prepare(image: &Gray, attempt: u32) -> Prepared {
    let scale = scale_for(image, attempt);
    let mut out = if (scale - 1.0).abs() < 0.01 {
        image.clone()
    } else {
        image.scaled(scale)
    };
    out.dark_on_light();
    match attempt {
        0 => {}
        1 => out.stretched(),
        _ => {
            out.stretched();
            out.black_and_white();
        }
    }
    Prepared { image: out, scale }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_rgba(w: u32, h: u32, px: &[[u8; 4]]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&px.concat())
            .unwrap();
        out
    }

    #[test]
    fn a_colour_picture_is_read_grey_and_see_through_is_white() {
        let png = png_rgba(2, 1, &[[255, 0, 0, 255], [0, 0, 0, 0]]);
        let g = Gray::from_png(&png).unwrap();
        assert_eq!((g.w, g.h), (2, 1));
        assert_eq!(g.px, vec![76, 255]);
        // and back: a grey PNG of the same size
        assert_eq!(Gray::from_png(&g.to_png()).unwrap(), g);
        assert!(Gray::from_png(b"not a png").is_err());
    }

    #[test]
    fn small_text_is_made_larger_and_a_large_picture_is_kept_within_what_can_be_read() {
        let small = Gray {
            w: 200,
            h: 30,
            px: vec![255; 6000],
        };
        let p = prepare(&small, 0);
        assert_eq!(p.scale, 2.0);
        assert_eq!((p.image.w, p.image.h), (400, 60));
        assert_eq!(prepare(&small, 1).scale, 3.0);
        let wide = Gray {
            w: 2000,
            h: 700,
            px: vec![255; 1_400_000],
        };
        assert!(prepare(&wide, 1).image.w <= MAX_SIDE);
        let huge = Gray {
            w: 5200,
            h: 100,
            px: vec![255; 520_000],
        };
        assert_eq!(prepare(&huge, 0).image.w, MAX_SIDE);
    }

    #[test]
    fn light_text_on_dark_is_turned_dark_on_light() {
        let mut px = vec![20u8; 100];
        px[55] = 230;
        let mut g = Gray { w: 10, h: 10, px };
        g.dark_on_light();
        assert_eq!(g.px[0], 235);
        assert_eq!(g.px[55], 25);
    }

    #[test]
    fn the_last_try_is_black_and_white() {
        let px: Vec<u8> = (0..100)
            .map(|i| if i % 3 == 0 { 60 } else { 200 })
            .collect();
        let p = prepare(&Gray { w: 10, h: 10, px }, 2);
        assert!(p.image.px.iter().all(|&v| v == 0 || v == 255));
        assert!(p.image.px.contains(&0));
    }
}
