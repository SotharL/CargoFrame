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

impl Watch {
    /// Start watching a recording of `area` (a window), or of the whole desktop.
    pub fn start(
        app: &AppHandle,
        sign: &RecordingSign,
        area: Option<Rect>,
        wayland: bool,
    ) -> Watch {
        // a Wayland desktop shows its own sign, and does not let windows be placed like this
        let shown = if wayland {
            Indicator::none()
        } else {
            match screens(app, area) {
                Some((desktop, screen)) => {
                    Indicator::show(area.unwrap_or(desktop), screen, look_of(sign))
                }
                None => Indicator::none(),
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let app = app.clone();
        let own = std::process::id();
        let thread = std::thread::Builder::new()
            .name("recording-watch".into())
            .spawn(move || {
                let _sign = shown;
                let began = Instant::now();
                let mut seen = Vec::new();
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
                                seen.push(Seen {
                                    at_ms: ran.as_millis() as u64,
                                    app: f,
                                });
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Watched {
                    seen,
                    ran_ms: began.elapsed().as_millis() as u64,
                }
            })
            .ok();
        Watch { stop, thread }
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
}
