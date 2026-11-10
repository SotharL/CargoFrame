//! Where the outline and the dot go, worked out without any window so it can be tested.

/// A rectangle in desktop pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    fn right(&self) -> i32 {
        self.x + self.w as i32
    }
    fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }
    /// Whether `inner` lies wholly inside.
    fn holds(&self, inner: &Rect) -> bool {
        inner.x >= self.x
            && inner.y >= self.y
            && inner.right() <= self.right()
            && inner.bottom() <= self.bottom()
    }
}

/// The corner the dot is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

/// How thick the outline is.
pub const EDGE: u32 = 3;
/// How wide the dot is.
pub const DOT: u32 = 14;
/// Space between the dot and the edge.
const GAP: i32 = 8;

/// What goes on screen: the outline as four bands, and the dot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub bands: Vec<Rect>,
    pub dot: Option<Rect>,
}

/// The four bands of an outline around `area`: just outside it when the screen has room (so a
/// recording of `area` does not show them), on its inner edge otherwise.
pub fn frame_bands(area: Rect, screen: Rect) -> Vec<Rect> {
    let e = EDGE as i32;
    let outside = Rect {
        x: area.x - e,
        y: area.y - e,
        w: area.w + 2 * EDGE,
        h: area.h + 2 * EDGE,
    };
    let f = if screen.holds(&outside) {
        outside
    } else {
        area
    };
    if f.w < 2 * EDGE || f.h < 2 * EDGE {
        return Vec::new();
    }
    vec![
        Rect {
            x: f.x,
            y: f.y,
            w: f.w,
            h: EDGE,
        },
        Rect {
            x: f.x,
            y: f.bottom() - e,
            w: f.w,
            h: EDGE,
        },
        Rect {
            x: f.x,
            y: f.y + e,
            w: EDGE,
            h: f.h - 2 * EDGE,
        },
        Rect {
            x: f.right() - e,
            y: f.y + e,
            w: EDGE,
            h: f.h - 2 * EDGE,
        },
    ]
}

/// The dot in `corner`: outside `area` when the screen has room beside it (so a recording of
/// `area` does not show it), inside the corner otherwise.
pub fn dot_at(area: Rect, screen: Rect, corner: Corner) -> Rect {
    let d = DOT as i32;
    let left = matches!(corner, Corner::TopLeft | Corner::BottomLeft);
    let top = matches!(corner, Corner::TopLeft | Corner::TopRight);
    // outside: above (or below) the area, at its corner
    let ox = if left { area.x } else { area.right() - d };
    let oy = if top {
        area.y - d - GAP
    } else {
        area.bottom() + GAP
    };
    let outside = Rect {
        x: ox,
        y: oy,
        w: DOT,
        h: DOT,
    };
    if screen.holds(&outside) {
        return outside;
    }
    let ix = if left {
        area.x + GAP
    } else {
        area.right() - d - GAP
    };
    let iy = if top {
        area.y + GAP
    } else {
        area.bottom() - d - GAP
    };
    Rect {
        x: ix,
        y: iy,
        w: DOT,
        h: DOT,
    }
}

pub fn layout(area: Rect, screen: Rect, outline: bool, dot: bool, corner: Corner) -> Layout {
    Layout {
        bands: if outline {
            frame_bands(area, screen)
        } else {
            Vec::new()
        },
        dot: dot.then(|| dot_at(area, screen, corner)),
    }
}

/// A round shape `size` wide as one span of pixels per row, `(y, x, width)`, for systems that
/// cut a window to a shape.
pub fn round_spans(size: u32) -> Vec<(u32, u32, u32)> {
    let r = size as f64 / 2.0;
    (0..size)
        .filter_map(|y| {
            let dy = y as f64 + 0.5 - r;
            let half = (r * r - dy * dy).max(0.0).sqrt();
            let x0 = (r - half).round() as u32;
            let x1 = (r + half).round() as u32;
            (x1 > x0).then_some((y, x0, x1 - x0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    const WINDOW: Rect = Rect {
        x: 100,
        y: 100,
        w: 800,
        h: 600,
    };

    #[test]
    fn the_outline_of_a_window_is_just_outside_it() {
        let b = frame_bands(WINDOW, SCREEN);
        assert_eq!(
            b[0],
            Rect {
                x: 97,
                y: 97,
                w: 806,
                h: 3
            }
        );
        assert_eq!(
            b[1],
            Rect {
                x: 97,
                y: 700,
                w: 806,
                h: 3
            }
        );
        assert_eq!(
            b[2],
            Rect {
                x: 97,
                y: 100,
                w: 3,
                h: 600
            }
        );
        assert_eq!(
            b[3],
            Rect {
                x: 900,
                y: 100,
                w: 3,
                h: 600
            }
        );
        // none of it is inside the window
        for r in &b {
            assert!(
                r.right() <= WINDOW.x
                    || r.x >= WINDOW.right()
                    || r.bottom() <= WINDOW.y
                    || r.y >= WINDOW.bottom()
            );
        }
    }

    #[test]
    fn the_outline_of_a_whole_screen_is_on_its_edge() {
        let b = frame_bands(SCREEN, SCREEN);
        assert_eq!(
            b[0],
            Rect {
                x: 0,
                y: 0,
                w: 1920,
                h: 3
            }
        );
        assert_eq!(
            b[3],
            Rect {
                x: 1917,
                y: 3,
                w: 3,
                h: 1074
            }
        );
        assert!(frame_bands(
            Rect {
                x: 0,
                y: 0,
                w: 4,
                h: 4
            },
            SCREEN
        )
        .is_empty());
    }

    #[test]
    fn the_dot_sits_beside_a_window_and_inside_a_full_screen() {
        assert_eq!(
            dot_at(WINDOW, SCREEN, Corner::TopRight),
            Rect {
                x: 886,
                y: 78,
                w: 14,
                h: 14
            }
        );
        assert_eq!(
            dot_at(WINDOW, SCREEN, Corner::BottomLeft),
            Rect {
                x: 100,
                y: 708,
                w: 14,
                h: 14
            }
        );
        assert_eq!(
            dot_at(SCREEN, SCREEN, Corner::TopLeft),
            Rect {
                x: 8,
                y: 8,
                w: 14,
                h: 14
            }
        );
        assert_eq!(
            dot_at(SCREEN, SCREEN, Corner::BottomRight),
            Rect {
                x: 1898,
                y: 1058,
                w: 14,
                h: 14
            }
        );
    }

    #[test]
    fn nothing_asked_nothing_shown() {
        let l = layout(WINDOW, SCREEN, false, false, Corner::TopRight);
        assert!(l.bands.is_empty() && l.dot.is_none());
        let l = layout(WINDOW, SCREEN, false, true, Corner::TopRight);
        assert!(l.bands.is_empty() && l.dot.is_some());
    }

    #[test]
    fn the_dot_is_round() {
        let s = round_spans(14);
        assert_eq!(s.len(), 14);
        // narrow at the top, the full width in the middle
        assert!(s[0].2 < 8);
        assert_eq!(s[7], (7, 0, 14));
        for (_, x, w) in &s {
            assert!(x + w <= 14);
        }
    }
}
