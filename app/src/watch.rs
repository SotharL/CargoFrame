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
    pub ran_ms: u64,
}

pub struct Watch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Watched>>,
    /// What was seen so far and when watching began, to look at while it goes on (a continuous
    /// recording hands out its last seconds without stopping).
    seen: Arc<std::sync::Mutex<Vec<Seen>>>,
    began: Instant,
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

/// The part of a recording of the whole screen (`size`, in its pixels) that `window` (on the
/// desktop) covers: the recording covers the desktop, or a screen of its size at the origin.
/// `None` when it cannot be placed, or is (nearly) all of it.
pub fn crop_in(
    window: Rect,
    desktop: Option<&Rect>,
    size: (u32, u32),
) -> Option<cargoframe_window::view::CropView> {
    let (w, h) = (i64::from(size.0), i64::from(size.1));
    // where the recording's top-left corner is on the desktop
    let origin = match desktop {
        Some(d) if i64::from(d.w) == w && i64::from(d.h) == h => (i64::from(d.x), i64::from(d.y)),
        _ => (0, 0),
    };
    let x0 = (i64::from(window.x) - origin.0).clamp(0, w);
    let y0 = (i64::from(window.y) - origin.1).clamp(0, h);
    let x1 = (i64::from(window.x) + i64::from(window.w) - origin.0).clamp(0, w);
    let y1 = (i64::from(window.y) + i64::from(window.h) - origin.1).clamp(0, h);
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

impl Watch {
    /// Start watching a recording of `area` (a window), or of the whole desktop.
    pub fn start(
        app: &AppHandle,
        sign: &RecordingSign,
        area: Option<Rect>,
        wayland: bool,
    ) -> Watch {
        Self::begin(app, sign, area, wayland, None)
    }

    /// Start watching a continuous recording of the whole screen: the outline goes around the
    /// window in focus, and follows it as the focus changes; only the last `keep_ms` (twice
    /// over) of what was seen is kept.
    pub fn start_following(
        app: &AppHandle,
        sign: &RecordingSign,
        wayland: bool,
        keep_ms: u64,
    ) -> Watch {
        Self::begin(app, sign, None, wayland, Some(keep_ms))
    }

    fn begin(
        app: &AppHandle,
        sign: &RecordingSign,
        area: Option<Rect>,
        wayland: bool,
        follow: Option<u64>,
    ) -> Watch {
        // a Wayland desktop shows its own sign, and does not let windows be placed like this
        let look = look_of(sign);
        let mut shown = if wayland {
            Indicator::none()
        } else {
            match screens(app, area) {
                Some((desktop, screen)) => Indicator::show(area.unwrap_or(desktop), screen, look),
                None => Indicator::none(),
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let app = app.clone();
        let own = std::process::id();
        let seen_shared: Arc<std::sync::Mutex<Vec<Seen>>> = Arc::default();
        let seen_out = seen_shared.clone();
        let began = Instant::now();
        let thread = std::thread::Builder::new()
            .name("recording-watch".into())
            .spawn(move || {
                let mut followed: Option<Rect> = None;
                let mut next = Duration::ZERO;
                loop {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let ran = began.elapsed();
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
                                let b = f.bounds;
                                let at = Rect {
                                    x: b.x,
                                    y: b.y,
                                    w: b.w,
                                    h: b.h,
                                };
                                // the outline moves with the window in focus
                                if follow.is_some() && !wayland && followed != Some(at) {
                                    followed = Some(at);
                                    shown = Indicator::none();
                                    if let Some((_, screen)) = screens(&app, Some(at)) {
                                        shown = Indicator::show(at, screen, look);
                                    }
                                }
                                let mut seen =
                                    seen_shared.lock().unwrap_or_else(|e| e.into_inner());
                                let now_ms = ran.as_millis() as u64;
                                seen.push(Seen {
                                    at_ms: now_ms,
                                    app: f,
                                });
                                if let Some(keep) = follow {
                                    let from = now_ms.saturating_sub(keep * 2);
                                    seen.retain(|s| s.at_ms >= from);
                                }
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                drop(shown);
                let seen =
                    std::mem::take(&mut *seen_shared.lock().unwrap_or_else(|e| e.into_inner()));
                Watched {
                    seen,
                    ran_ms: began.elapsed().as_millis() as u64,
                }
            })
            .ok();
        Watch {
            stop,
            thread,
            seen: seen_out,
            began,
        }
    }

    /// What was seen so far, while watching goes on.
    pub fn peek(&self) -> Watched {
        Watched {
            seen: self.seen.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            ran_ms: self.began.elapsed().as_millis() as u64,
        }
    }

    /// Stop watching: the sign goes, and what was seen is handed back.
    pub fn finish(mut self) -> Watched {
        self.stop.store(true, Ordering::SeqCst);
        self.thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default()
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
}
