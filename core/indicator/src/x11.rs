//! X11: plain windows the window manager leaves alone (override-redirect), each cut to its shape
//! with the Shape extension, so no compositor is needed: four red bands for the outline and a
//! round red dot. Clicks go through them (their input shape is empty). A thread of its own makes
//! the dot blink, and takes everything down when the indicator is dropped.

use crate::geometry::{round_spans, Layout, Rect};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shape::{self, ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{
    ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, EventMask, Rectangle, StackMode,
    Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

const RED: u32 = 0xE5_32_2D;

pub struct Shown {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Shown {
    pub fn start(lay: &Layout) -> Result<Shown, String> {
        let (conn, screen_num) = x11rb::connect(None).map_err(|e| e.to_string())?;
        // the Shape extension is what makes the dot round and lets clicks through
        if conn
            .extension_information(shape::X11_EXTENSION_NAME)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("this X server has no Shape extension".into());
        }
        let root = conn.setup().roots[screen_num].root;
        let mut bands = Vec::new();
        for b in &lay.bands {
            bands.push(window(&conn, root, *b, None)?);
        }
        let dot = match lay.dot {
            Some(d) => Some(window(&conn, root, d, Some(round_spans(d.w)))?),
            None => None,
        };
        for w in bands.iter().chain(dot.iter()) {
            conn.map_window(*w).map_err(|e| e.to_string())?;
        }
        conn.flush().map_err(|e| e.to_string())?;

        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("recording-sign".into())
            .spawn(move || {
                let mut on = true;
                let mut ticks = 0u32;
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(100));
                    ticks += 1;
                    // the dot is on for a second, off for a second; the bands stay on top
                    if ticks % 10 == 0 {
                        on = !on;
                        if let Some(d) = dot {
                            let _ = if on {
                                conn.map_window(d)
                            } else {
                                conn.unmap_window(d)
                            };
                        }
                        for w in bands.iter().chain(dot.iter()) {
                            let _ = conn.configure_window(
                                *w,
                                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
                            );
                        }
                        let _ = conn.flush();
                    }
                }
                for w in bands.iter().chain(dot.iter()) {
                    let _ = conn.destroy_window(*w);
                }
                let _ = conn.flush();
            })
            .map_err(|e| e.to_string())?;
        Ok(Shown {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Shown {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A red window at `r`, cut to `spans` (rows of pixels) when given, that clicks go through.
fn window(
    conn: &RustConnection,
    root: Window,
    r: Rect,
    spans: Option<Vec<(u32, u32, u32)>>,
) -> Result<Window, String> {
    let id = conn.generate_id().map_err(|e| e.to_string())?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        id,
        root,
        r.x as i16,
        r.y as i16,
        r.w.max(1) as u16,
        r.h.max(1) as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new()
            .background_pixel(RED)
            .override_redirect(1)
            .event_mask(EventMask::NO_EVENT),
    )
    .map_err(|e| e.to_string())?;
    if let Some(spans) = spans {
        let rects: Vec<Rectangle> = spans
            .iter()
            .map(|(y, x, w)| Rectangle {
                x: *x as i16,
                y: *y as i16,
                width: *w as u16,
                height: 1,
            })
            .collect();
        conn.shape_rectangles(
            SO::SET,
            SK::BOUNDING,
            x11rb::protocol::xproto::ClipOrdering::UNSORTED,
            id,
            0,
            0,
            &rects,
        )
        .map_err(|e| e.to_string())?;
    }
    // no input shape: clicks reach whatever is under it
    conn.shape_rectangles(
        SO::SET,
        SK::INPUT,
        x11rb::protocol::xproto::ClipOrdering::UNSORTED,
        id,
        0,
        0,
        &[],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{layout, Corner};
    use x11rb::protocol::xproto::MapState;

    /// Needs an X server (`DISPLAY`): run by the end-to-end check, skipped elsewhere.
    #[test]
    fn shows_red_bands_and_a_round_dot_that_blinks_then_takes_them_down() {
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        let (conn, n) = x11rb::connect(None).unwrap();
        let root = conn.setup().roots[n].root;
        let screen = Rect {
            x: 0,
            y: 0,
            w: conn.setup().roots[n].width_in_pixels as u32,
            h: conn.setup().roots[n].height_in_pixels as u32,
        };
        let mapped = || -> Vec<(i16, i16, u16, u16)> {
            conn.query_tree(root)
                .unwrap()
                .reply()
                .unwrap()
                .children
                .iter()
                .filter_map(|w| {
                    let a = conn.get_window_attributes(*w).ok()?.reply().ok()?;
                    if a.map_state != MapState::VIEWABLE || !a.override_redirect {
                        return None;
                    }
                    let g = conn.get_geometry(*w).ok()?.reply().ok()?;
                    Some((g.x, g.y, g.width, g.height))
                })
                .collect()
        };
        let before = mapped().len();
        let area = Rect {
            x: 100,
            y: 100,
            w: 400,
            h: 300,
        };
        let lay = layout(area, screen, true, true, Corner::TopRight);
        let shown = Shown::start(&lay).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let now = mapped();
        assert_eq!(now.len() - before, 5, "{now:?}");
        assert!(now.contains(&(97, 97, 406, 3)), "{now:?}");
        // a second later the dot is off
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(mapped().len() - before, 4);
        drop(shown);
        assert_eq!(mapped().len(), before);
    }
}
