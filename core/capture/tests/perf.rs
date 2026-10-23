//! Timing gates for the capture path, run in release mode by CI:
//!
//! ```text
//! cargo test --release -p cargoframe-capture --test perf -- --ignored --nocapture
//! ```
//!
//! They are `#[ignore]`d so a debug `cargo test` stays fast and is never judged on
//! unoptimized timings. A gate fails when the measured time is more than 20% over its budget
//! (the margin the performance plan allows for a noisy machine). `CARGOFRAME_PERF_SCALE`
//! (default 1) stretches every budget for slower machines such as shared CI runners.

use cargoframe_capture::Frame;
use std::time::{Duration, Instant};

const MARGIN: f64 = 1.2;

fn scale() -> f64 {
    std::env::var("CARGOFRAME_PERF_SCALE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(1.0)
}

/// A 4K picture that looks like a screenshot: flat panels, gradients, thin text-like lines and a
/// little noise, so the compressor has realistic work to do (neither all-flat nor all-random).
fn screenshot_4k() -> Frame {
    let (w, h) = (3840u32, 2160u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let (r, g, b) = if x < 480 {
                (0x2B, 0x30, 0x3B) // side bar
            } else if y < 90 {
                (0x20, 0x24, 0x2E) // title bar
            } else if (y / 24) % 2 == 0 && x % 11 < 6 && (next() % 5) != 0 {
                (0xD8, 0xDE, 0xE9) // a line of "text"
            } else {
                let t = (x * 255 / w) as u8;
                (0x1E, 0x22u8.saturating_add(t / 8), 0x2E) // editor background gradient
            };
            let noise = (next() % 3) as u8;
            rgba[i] = r + noise;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = 255;
        }
    }
    Frame::new(w, h, rgba).expect("a valid frame")
}

fn best_of<T>(runs: usize, mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut best = Duration::MAX;
    let mut last = None;
    for _ in 0..runs {
        let t = Instant::now();
        let v = f();
        best = best.min(t.elapsed());
        last = Some(v);
    }
    (best, last.expect("at least one run"))
}

fn within(name: &str, took: Duration, budget_ms: u64) {
    let limit = Duration::from_millis((budget_ms as f64 * MARGIN * scale()) as u64);
    println!("{name}: {took:?} (budget {budget_ms} ms, gate {limit:?})");
    assert!(
        took <= limit,
        "{name} took {took:?}, over the {budget_ms} ms budget plus 20% (scale {})",
        scale()
    );
}

#[test]
#[ignore = "timing gate; run in release mode"]
fn the_picture_is_ready_to_show_quickly_on_a_4k_screen() {
    let frame = screenshot_4k();
    let (took, png) = best_of(3, || frame.to_png().expect("encodes"));
    println!("fast PNG size: {} KiB", png.len() / 1024);
    // The plan's whole "trigger to capture complete" budget is 150 ms for a 4K monitor, and the
    // operating system's own grab comes out of it, so encoding gets a share.
    within("4K fast PNG (shown at once)", took, 120);
}

#[test]
#[ignore = "timing gate; run in release mode"]
fn cropping_a_4k_picture_is_instant() {
    let frame = screenshot_4k();
    let area = cargoframe_capture::PixelRect::new(100, 100, 1920, 1080);
    let (took, cropped) = best_of(5, || frame.crop(area).expect("inside"));
    assert_eq!((cropped.width(), cropped.height()), (1920, 1080));
    within("crop 1920x1080 out of 4K", took, 20);
}
