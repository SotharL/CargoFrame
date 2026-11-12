//! While a trigger records: the sign on screen that says so (see `cargoframe_indicator`), and
//! which program has the focus, once a second. When the recording stops, the program that had
//! the focus the longest in the part that is kept is the one the capture's data comes from
//! (its folder, files and values), even if the person has moved to another one to press the
//! shortcut. Both end with the recording, also when it stops by itself.

use crate::state::AppState;
use cargoframe_capture::FocusedApp;
use cargoframe_config::{RecordingSign, SignCorner};
use cargoframe_indicator::{Corner, Indicator, Look, Rect};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// A program seen with the focus, and when (milliseconds after the recording started).
#[derive(Debug, Clone)]
pub struct Seen {
    pub at_ms: u64,
    pub app: FocusedApp,
}

/// What was seen, and how long the recording ran when it stopped.
#[derive(Debug, Clone, Default)]
pub struct Watched {
    pub seen: Vec<Seen>,
    /// Where the followed window was, when it moved (see [`Places`]).
    pub places: Vec<Place>,
    /// The keys pressed, when they were read.
    pub presses: Vec<cargoframe_keys::Press>,
    pub ran_ms: u64,
}

pub struct Watch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Watched>>,
    /// What was seen so far and when watching began, to look at while it goes on (a continuous
    /// recording hands out its last seconds without stopping).
    seen: Arc<std::sync::Mutex<Vec<Seen>>>,
    places: Arc<std::sync::Mutex<Vec<Place>>>,
    keys: Option<cargoframe_keys::Tracker>,
    keys_problem: Option<String>,
    began: Instant,
}

fn rect_of(b: cargoframe_capture::PixelRect) -> Rect {
    Rect {
        x: b.x,
        y: b.y,
        w: b.w,
        h: b.h,
    }
}

pub fn look_of(sign: &RecordingSign) -> Look {
    Look {
        outline: sign.outline,
        dot: sign.dot,
        corner: match sign.corner {
            SignCorner::TopLeft => Corner::TopLeft,
            SignCorner::TopRight => Corner::TopRight,
            SignCorner::BottomLeft => Corner::BottomLeft,
            SignCorner::BottomRight => Corner::BottomRight,
        },
    }
}

/// The monitors as one rectangle, and the one that holds `area` (by its middle).
fn screens(app: &AppHandle, area: Option<Rect>) -> Option<(Rect, Rect)> {
    let monitors = app.available_monitors().ok()?;
    let rects: Vec<Rect> = monitors
        .iter()
        .map(|m| Rect {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        })
        .collect();
    let first = *rects.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (
        first.x,
        first.y,
        first.x + first.w as i32,
        first.y + first.h as i32,
    );
    for r in &rects {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w as i32);
        y1 = y1.max(r.y + r.h as i32);
    }
    let desktop = Rect {
        x: x0,
        y: y0,
        w: (x1 - x0) as u32,
        h: (y1 - y0) as u32,
    };
    let screen = match area {
        Some(a) => {
            let (cx, cy) = (a.x + a.w as i32 / 2, a.y + a.h as i32 / 2);
            rects
                .iter()
                .copied()
                .find(|r| cx >= r.x && cy >= r.y && cx < r.x + r.w as i32 && cy < r.y + r.h as i32)
                .unwrap_or(desktop)
        }
        None => desktop,
    };
    Some((desktop, screen))
}

/// The whole desktop (every monitor as one rectangle).
pub fn desktop_of(app: &AppHandle) -> Option<Rect> {
    screens(app, None).map(|(desktop, _)| desktop)
}

/// How desktop pixels map to a recording of the whole screen (`size`, in its pixels): the
/// recording is the desktop, perhaps made smaller (the height setting), or one screen of its
/// size at the origin.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Onto {
    origin: (f64, f64),
    scale: f64,
}

impl Onto {
    fn new(desktop: Option<&Rect>, size: (u32, u32)) -> Self {
        let (w, h) = (f64::from(size.0), f64::from(size.1));
        match desktop {
            // the same shape as the desktop: it is the desktop, at its size or smaller
            Some(d)
                if d.w > 0
                    && d.h > 0
                    && h > 0.0
                    && ((w / h) - f64::from(d.w) / f64::from(d.h)).abs() < 0.01 =>
            {
                Self {
                    origin: (f64::from(d.x), f64::from(d.y)),
                    scale: w / f64::from(d.w),
                }
            }
            _ => Self {
                origin: (0.0, 0.0),
                scale: 1.0,
            },
        }
    }

    fn point(&self, x: i32, y: i32) -> (i64, i64) {
        (
            ((f64::from(x) - self.origin.0) * self.scale).round() as i64,
            ((f64::from(y) - self.origin.1) * self.scale).round() as i64,
        )
    }
}

/// The part of a recording of the whole screen (`size`, in its pixels) that `window` (on the
/// desktop) covers: the recording covers the desktop (perhaps made smaller), or a screen of its
/// size at the origin. `None` when it cannot be placed, or is (nearly) all of it.
pub fn crop_in(
    window: Rect,
    desktop: Option<&Rect>,
    size: (u32, u32),
) -> Option<cargoframe_window::view::CropView> {
    let (w, h) = (i64::from(size.0), i64::from(size.1));
    let onto = Onto::new(desktop, size);
    let (x0, y0) = onto.point(window.x, window.y);
    let (x1, y1) = onto.point(
        window.x.saturating_add(window.w as i32),
        window.y.saturating_add(window.h as i32),
    );
    let (x0, y0, x1, y1) = (
        x0.clamp(0, w),
        y0.clamp(0, h),
        x1.clamp(0, w),
        y1.clamp(0, h),
    );
    // even sides, as the video needs them
    let (cw, ch) = ((x1 - x0) & !1, (y1 - y0) & !1);
    if cw < 16 || ch < 16 || (cw >= w - 1 && ch >= h - 1) {
        return None;
    }
    Some(cargoframe_window::view::CropView {
        x: (x0 & !1) as u32,
        y: (y0 & !1) as u32,
        w: cw as u32,
        h: ch as u32,
    })
}

/// The part of the recording a clip covers, on the watch's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub from_ms: u64,
    pub to_ms: u64,
}

impl Span {
    /// A clip of `clip_ms` that ends at `to_ms`.
    pub fn ending(to_ms: u64, clip_ms: u64) -> Self {
        Self {
            from_ms: to_ms.saturating_sub(clip_ms),
            to_ms,
        }
    }
}

/// The most places a clip's crop goes through (see `cargoframe_video::edit::MAX_PLACES`).
const MAX_PATH: usize = 300;

/// Where the crop of a recording of the whole screen goes to follow `window` as it was moved
/// during `span`: the crop where the clip starts, and the places its corner goes through on
/// the clip's timeline (empty when the window stayed put). `None` when the window was not seen.
pub fn follow_path(
    places: &[Place],
    window: u64,
    span: Span,
    desktop: Option<&Rect>,
    size: (u32, u32),
) -> Option<(
    cargoframe_window::view::CropView,
    Vec<cargoframe_window::view::PlaceView>,
)> {
    let mine: Vec<&Place> = places.iter().filter(|p| p.window == window).collect();
    // where it was when the clip starts: the last place before, else the first one in it
    let start = mine
        .iter()
        .rev()
        .find(|p| p.at_ms <= span.from_ms)
        .or_else(|| mine.iter().find(|p| p.at_ms <= span.to_ms))?;
    let crop = crop_in(start.rect, desktop, size)?;
    let onto = Onto::new(desktop, size);
    let (max_x, max_y) = (
        i64::from(size.0.saturating_sub(crop.w)),
        i64::from(size.1.saturating_sub(crop.h)),
    );
    let corner = |p: &Place| {
        let (x, y) = onto.point(p.rect.x, p.rect.y);
        (
            (x.clamp(0, max_x) & !1) as u32,
            (y.clamp(0, max_y) & !1) as u32,
        )
    };
    let mut path = vec![cargoframe_window::view::PlaceView {
        at_ms: 0,
        x: crop.x,
        y: crop.y,
    }];
    for p in mine
        .iter()
        .filter(|p| p.at_ms > span.from_ms && p.at_ms <= span.to_ms)
    {
        let (x, y) = corner(p);
        path.push(cargoframe_window::view::PlaceView {
            at_ms: p.at_ms - span.from_ms,
            x,
            y,
        });
    }
    if path.iter().all(|p| (p.x, p.y) == (crop.x, crop.y)) {
        return Some((crop, Vec::new()));
    }
    // a long drag is kept to a number of places ffmpeg works through quickly
    if path.len() > MAX_PATH {
        let step = path.len().div_ceil(MAX_PATH);
        let last = *path.last().expect("not empty");
        path = path.into_iter().step_by(step).collect();
        if path.last() != Some(&last) {
            path.push(last);
        }
    }
    Some((crop, path))
}

/// The lines of keys a clip shows, from the presses read during `span`.
pub fn key_lines(
    presses: &[cargoframe_keys::Press],
    span: Span,
) -> Vec<cargoframe_window::view::KeyLine> {
    cargoframe_keys::captions(presses, span.from_ms, span.to_ms)
        .into_iter()
        .map(|c| cargoframe_window::view::KeyLine {
            from_ms: c.from_ms,
            to_ms: c.to_ms,
            text: c.text,
        })
        .collect()
}

/// What a watch looks at besides the program in focus.
#[derive(Debug, Clone, Copy, Default)]
pub struct Plan {
    /// The window recorded, where the outline goes; `None` is the whole desktop.
    pub area: Option<Rect>,
    /// A continuous recording: the outline follows the window in focus, and only the last
    /// `keep_ms` (twice over) of what was seen is kept.
    pub keep_ms: Option<u64>,
    /// Read the keys pressed (the layout's Video block asks for them).
    pub keys: bool,
    /// Note where a window is, ten times a second, so the clip can follow it as it is moved.
    pub places: Places,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Places {
    #[default]
    None,
    /// This window (as `FocusedApp::window` names it).
    Window(u64),
    /// Whichever window has the focus (a continuous recording).
    Focused,
}

/// Where a window was at a moment (milliseconds after the recording started).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub at_ms: u64,
    pub window: u64,
    pub rect: Rect,
}

/// The most places kept, whatever happens.
const MAX_PLACES: usize = 20_000;

/// Forget the places older than `from`, but each window's last one before it: that is where
/// the window was when the kept part starts.
fn prune(places: &mut Vec<Place>, from: u64) {
    let mut last_before = std::collections::HashMap::new();
    for (i, q) in places.iter().enumerate() {
        if q.at_ms < from {
            last_before.insert(q.window, i);
        }
    }
    let mut i = 0;
    places.retain(|q| {
        let keep = q.at_ms >= from || last_before.get(&q.window) == Some(&i);
        i += 1;
        keep
    });
}

impl Watch {
    /// Start watching as `plan` says. The keys are read only when it asks; why they cannot be
    /// (Wayland) is in [`Watch::keys_problem`].
    pub fn begin(app: &AppHandle, sign: &RecordingSign, wayland: bool, plan: Plan) -> Watch {
        // a Wayland desktop shows its own sign, and does not let windows be placed like this
        let look = look_of(sign);
        let area = plan.area;
        let mut shown = if wayland {
            Indicator::none()
        } else {
            match screens(app, area) {
                Some((desktop, screen)) => Indicator::show(area.unwrap_or(desktop), screen, look),
                None => Indicator::none(),
            }
        };
        let began = Instant::now();
        let (keys, keys_problem) = if plan.keys {
            match cargoframe_keys::Tracker::start(began, plan.keep_ms) {
                Ok(t) => (Some(t), None),
                Err(e) => (None, Some(e)),
            }
        } else {
            (None, None)
        };
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let app = app.clone();
        let own = std::process::id();
        let seen_shared: Arc<std::sync::Mutex<Vec<Seen>>> = Arc::default();
        let seen_out = seen_shared.clone();
        let places_shared: Arc<std::sync::Mutex<Vec<Place>>> = Arc::default();
        let places_out = places_shared.clone();
        let follow_focus = plan.keep_ms.is_some();
        let thread = std::thread::Builder::new()
            .name("recording-watch".into())
            .spawn(move || {
                let placer = (plan.places != Places::None && !wayland)
                    .then(cargoframe_capture::WindowPlace::open)
                    .flatten();
                let mut tracked = match plan.places {
                    Places::Window(w) => Some(w),
                    _ => None,
                };
                let mut followed: Option<Rect> = area;
                let mut last_sample: Option<Place> = None;
                let mut next = Duration::ZERO;
                loop {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let ran = began.elapsed();
                    let now_ms = ran.as_millis() as u64;
                    if ran >= next {
                        next = ran + Duration::from_secs(1);
                        // the recording stopped by itself: the sign goes at once
                        if !matches!(
                            app.state::<AppState>().recording.view(),
                            crate::video::RecordingView::Recording { .. }
                        ) {
                            break;
                        }
                        if let Ok(Some(f)) = cargoframe_capture::focused_app() {
                            if f.pid != Some(own) {
                                let at = rect_of(f.bounds);
                                if plan.places == Places::Focused {
                                    tracked = Some(f.window);
                                }
                                // the outline moves with the window in focus
                                if follow_focus && !wayland && followed != Some(at) {
                                    followed = Some(at);
                                    shown = Indicator::none();
                                    if let Some((_, screen)) = screens(&app, Some(at)) {
                                        shown = Indicator::show(at, screen, look);
                                    }
                                }
                                let mut seen =
                                    seen_shared.lock().unwrap_or_else(|e| e.into_inner());
                                seen.push(Seen {
                                    at_ms: now_ms,
                                    app: f,
                                });
                                if let Some(keep) = plan.keep_ms {
                                    let from = now_ms.saturating_sub(keep * 2);
                                    seen.retain(|s| s.at_ms >= from);
                                }
                            }
                        }
                    }
                    // where the followed window is now
                    if let (Some(p), Some(window)) = (&placer, tracked) {
                        if let Some(b) = p.bounds(window) {
                            let here = Place {
                                at_ms: now_ms,
                                window,
                                rect: rect_of(b),
                            };
                            let mut places =
                                places_shared.lock().unwrap_or_else(|e| e.into_inner());
                            let last = places.iter().rev().find(|q| q.window == window).copied();
                            if last.map(|q| q.rect) != Some(here.rect) {
                                // it stood still until the last look: so it slides from there
                                if let Some(before) = last_sample.filter(|b| {
                                    b.window == window && Some(b.at_ms) != last.map(|q| q.at_ms)
                                }) {
                                    places.push(before);
                                }
                                places.push(here);
                                // the outline goes with it (a window followed by itself only)
                                if matches!(plan.places, Places::Window(_)) && !follow_focus {
                                    shown = Indicator::none();
                                    if let Some((_, screen)) = screens(&app, Some(here.rect)) {
                                        shown = Indicator::show(here.rect, screen, look);
                                    }
                                }
                                if let Some(keep) = plan.keep_ms {
                                    prune(&mut places, now_ms.saturating_sub(keep * 2));
                                }
                                if places.len() > MAX_PLACES {
                                    let over = places.len() - MAX_PLACES;
                                    places.drain(..over);
                                }
                            }
                            last_sample = Some(here);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                drop(shown);
                let seen =
                    std::mem::take(&mut *seen_shared.lock().unwrap_or_else(|e| e.into_inner()));
                let places =
                    std::mem::take(&mut *places_shared.lock().unwrap_or_else(|e| e.into_inner()));
                Watched {
                    seen,
                    places,
                    presses: Vec::new(),
                    ran_ms: began.elapsed().as_millis() as u64,
                }
            })
            .ok();
        Watch {
            stop,
            thread,
            seen: seen_out,
            places: places_out,
            keys,
            keys_problem,
            began,
        }
    }

    /// Why the keys pressed could not be read, when they were asked for.
    pub fn keys_problem(&self) -> Option<&str> {
        self.keys_problem.as_deref()
    }

    /// What was seen so far, while watching goes on.
    pub fn peek(&self) -> Watched {
        Watched {
            seen: self.seen.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            places: self
                .places
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            presses: self.keys.as_ref().map(|k| k.presses()).unwrap_or_default(),
            ran_ms: self.began.elapsed().as_millis() as u64,
        }
    }

    /// Stop watching: the sign goes, and what was seen is handed back.
    pub fn finish(mut self) -> Watched {
        self.stop.store(true, Ordering::SeqCst);
        let presses = self.keys.take().map(|k| k.finish()).unwrap_or_default();
        let mut w = self
            .thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default();
        w.presses = presses;
        w
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The program that had the focus the longest in the last `keep_ms` of the recording, and the
/// other programs seen then (by name, without repeats).
pub fn longest(w: &Watched, keep_ms: u64) -> Option<(FocusedApp, Vec<String>)> {
    let from = w.ran_ms.saturating_sub(keep_ms);
    let kept: Vec<&Seen> = w.seen.iter().filter(|s| s.at_ms >= from).collect();
    let key = |a: &FocusedApp| {
        a.exe_path
            .clone()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| a.title.clone())
    };
    let mut counts: Vec<(String, usize, &FocusedApp)> = Vec::new();
    for s in &kept {
        let k = key(&s.app);
        match counts.iter_mut().find(|c| c.0 == k) {
            // the latest sight of a program is the one kept (its window may have moved)
            Some(c) => {
                c.1 += 1;
                c.2 = &s.app;
            }
            None => counts.push((k, 1, &s.app)),
        }
    }
    // the most seen; on a tie, the one seen last
    let best = counts.iter().enumerate().max_by_key(|(i, c)| (c.1, *i))?.1;
    let name = |a: &FocusedApp| {
        a.exe_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| a.title.clone())
    };
    let others = counts
        .iter()
        .filter(|c| c.0 != best.0)
        .map(|c| name(c.2))
        .collect();
    Some((best.2.clone(), others))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn app(exe: &str, title: &str) -> FocusedApp {
        FocusedApp {
            window: 1,
            exe_path: Some(PathBuf::from(exe)),
            title: title.into(),
            pid: Some(10),
            bounds: cargoframe_capture::PixelRect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
        }
    }

    #[test]
    fn the_program_in_focus_longest_in_the_kept_part_gives_the_data() {
        let maya = app("/apps/maya", "Maya - scene");
        let chrome = app("/apps/chrome", "Docs");
        let mut seen = Vec::new();
        // 0-20 s: Chrome; 20-27 s: Maya; 27-30 s: Chrome again (to press the shortcut)
        for s in 0..30u64 {
            let a = if (20..27).contains(&s) {
                &maya
            } else {
                &chrome
            };
            seen.push(Seen {
                at_ms: s * 1000,
                app: a.clone(),
            });
        }
        let w = Watched {
            seen,
            ran_ms: 30_000,
            ..Watched::default()
        };
        // the last 10 s: Maya 7 times, Chrome 3 times
        let (best, others) = longest(&w, 10_000).unwrap();
        assert_eq!(best.title, "Maya - scene");
        assert_eq!(others, vec!["chrome".to_string()]);
        // over all 30 s, Chrome
        assert_eq!(longest(&w, 60_000).unwrap().0.title, "Docs");
        assert!(longest(&Watched::default(), 10_000).is_none());
    }

    #[test]
    fn the_sign_follows_the_settings() {
        let l = look_of(&RecordingSign {
            outline: false,
            dot: true,
            corner: SignCorner::BottomLeft,
        });
        assert_eq!(
            l,
            Look {
                outline: false,
                dot: true,
                corner: Corner::BottomLeft
            }
        );
    }

    #[test]
    fn the_window_in_focus_is_placed_in_a_recording_of_the_whole_screen() {
        let desktop = Rect {
            x: -1920,
            y: 0,
            w: 3840,
            h: 1080,
        };
        let window = Rect {
            x: -1000,
            y: 101,
            w: 801,
            h: 600,
        };
        // a recording of the whole desktop: its corner is the desktop's
        let c = crop_in(window, Some(&desktop), (3840, 1080)).unwrap();
        assert_eq!((c.x, c.y, c.w, c.h), (920, 100, 800, 600));
        // a recording of one screen at the origin
        let c = crop_in(
            Rect {
                x: 10,
                y: 10,
                w: 400,
                h: 300,
            },
            Some(&desktop),
            (1920, 1080),
        )
        .unwrap();
        assert_eq!((c.x, c.y, c.w, c.h), (10, 10, 400, 300));
        // the whole of it, or nothing of it: no crop
        assert!(crop_in(
            Rect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080
            },
            None,
            (1920, 1080)
        )
        .is_none());
        assert!(crop_in(
            Rect {
                x: 5000,
                y: 0,
                w: 100,
                h: 100
            },
            None,
            (1920, 1080)
        )
        .is_none());
    }

    fn place(at_ms: u64, window: u64, x: i32, y: i32) -> Place {
        Place {
            at_ms,
            window,
            rect: Rect {
                x,
                y,
                w: 400,
                h: 300,
            },
        }
    }

    #[test]
    fn old_places_go_but_each_window_keeps_where_it_was() {
        let mut places = vec![
            place(0, 1, 0, 0),
            place(100, 2, 0, 0),
            place(200, 1, 10, 0),
            place(5_000, 2, 50, 0),
        ];
        prune(&mut places, 1_000);
        let left: Vec<(u64, u64)> = places.iter().map(|p| (p.at_ms, p.window)).collect();
        // each window keeps its last place before the kept part: where it was when that starts
        assert_eq!(left, vec![(100, 2), (200, 1), (5_000, 2)]);
    }

    #[test]
    fn a_recording_made_smaller_is_cropped_to_scale() {
        let desktop = Rect {
            x: 0,
            y: 0,
            w: 3840,
            h: 2160,
        };
        // recorded at half the size (the height setting)
        let c = crop_in(
            Rect {
                x: 400,
                y: 200,
                w: 800,
                h: 600,
            },
            Some(&desktop),
            (1920, 1080),
        )
        .unwrap();
        assert_eq!((c.x, c.y, c.w, c.h), (200, 100, 400, 300));
    }

    #[test]
    fn the_crop_follows_the_window_through_the_clip() {
        let desktop = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let places = vec![
            place(0, 7, 100, 100),
            // another window, not followed
            place(500, 9, 0, 0),
            // dragged from 3.0 s to 3.2 s, after standing still since the start
            place(2_900, 7, 100, 100),
            place(3_000, 7, 150, 120),
            place(3_200, 7, 1_700, 120),
        ];
        // the clip is 2 s to 4 s of the recording
        let span = Span::ending(4_000, 2_000);
        let (crop, path) = follow_path(&places, 7, span, Some(&desktop), (1920, 1080)).unwrap();
        assert_eq!((crop.x, crop.y, crop.w, crop.h), (100, 100, 400, 300));
        let got: Vec<(u64, u32, u32)> = path.iter().map(|p| (p.at_ms, p.x, p.y)).collect();
        // on the clip's timeline; the last place is kept inside the picture (1920 - 400)
        assert_eq!(
            got,
            vec![
                (0, 100, 100),
                (900, 100, 100),
                (1_000, 150, 120),
                (1_200, 1_520, 120)
            ]
        );
        // a window that stood still needs only its crop
        let still = vec![place(0, 7, 100, 100), place(3_000, 7, 100, 100)];
        let (_, path) = follow_path(&still, 7, span, Some(&desktop), (1920, 1080)).unwrap();
        assert!(path.is_empty());
        // a window never seen is not followed
        assert!(follow_path(&places, 8, span, Some(&desktop), (1920, 1080)).is_none());
    }

    #[test]
    fn the_keys_of_the_clip_are_on_its_timeline() {
        use cargoframe_keys::{Key, Mods, Press};
        let press = |at_ms, c| Press {
            at_ms,
            key: Key::Char { typed: c, base: c },
            mods: Mods::default(),
        };
        let lines = key_lines(
            &[press(500, 'a'), press(2_500, 'b')],
            Span::ending(4_000, 2_000),
        );
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].from_ms, lines[0].text.as_str()), (500, "b"));
    }
}
