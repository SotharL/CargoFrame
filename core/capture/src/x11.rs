//! X11 backend, in pure Rust (no libxcb needed): the window in focus from the window manager's
//! `_NET_ACTIVE_WINDOW` (or the input focus when there is no window manager), pixels from the
//! root window, displays from RandR.

use crate::error::{grab, CaptureError};
use crate::frame::{Frame, PixelRect};
use crate::policy::{Backend, FocusedApp};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, ImageFormat, ImageOrder, Window,
};
use x11rb::rust_connection::RustConnection;

pub struct X11 {
    conn: RustConnection,
    screen: usize,
}

impl X11 {
    pub fn connect() -> Result<Self, CaptureError> {
        let (conn, screen) =
            RustConnection::connect(None).map_err(|e| CaptureError::NoDisplay(e.to_string()))?;
        Ok(Self { conn, screen })
    }

    fn root(&self) -> Window {
        self.conn.setup().roots[self.screen].root
    }

    fn atom(&self, name: &str) -> Result<Atom, CaptureError> {
        Ok(self
            .conn
            .intern_atom(false, name.as_bytes())
            .map_err(grab)?
            .reply()
            .map_err(grab)?
            .atom)
    }

    fn cardinals(&self, win: Window, property: &str) -> Result<Vec<u32>, CaptureError> {
        let atom = self.atom(property)?;
        let reply = self
            .conn
            .get_property(false, win, atom, AtomEnum::CARDINAL, 0, 16)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        Ok(reply.value32().map(|v| v.collect()).unwrap_or_default())
    }

    fn text(
        &self,
        win: Window,
        property: &str,
        kind: Atom,
    ) -> Result<Option<String>, CaptureError> {
        let atom = self.atom(property)?;
        let reply = self
            .conn
            .get_property(false, win, atom, kind, 0, 1024)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        if reply.value.is_empty() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&reply.value).into_owned()))
    }

    /// The window the window manager calls active, if it says.
    fn net_active_window(&self) -> Result<Option<Window>, CaptureError> {
        let root = self.root();
        let active = self.atom("_NET_ACTIVE_WINDOW")?;
        let reply = self
            .conn
            .get_property(false, root, active, AtomEnum::WINDOW, 0, 1)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        Ok(reply
            .value32()
            .and_then(|mut v| v.next())
            .filter(|w| *w != 0))
    }

    /// The window the person is working in.
    fn active_window(&self) -> Result<Option<Window>, CaptureError> {
        let root = self.root();
        if let Some(w) = self.net_active_window()? {
            return Ok(Some(w));
        }
        // No window manager (or one that does not say): ask the input focus. 0 is None and
        // 1 is PointerRoot.
        let focus = self
            .conn
            .get_input_focus()
            .map_err(grab)?
            .reply()
            .map_err(grab)?
            .focus;
        if focus <= 1 || focus == root {
            return Ok(None);
        }
        Ok(Some(self.toplevel(focus)?))
    }

    /// Climb from a (possibly inner) window to the child of the root.
    fn toplevel(&self, mut win: Window) -> Result<Window, CaptureError> {
        let root = self.root();
        for _ in 0..32 {
            let tree = self
                .conn
                .query_tree(win)
                .map_err(grab)?
                .reply()
                .map_err(grab)?;
            if tree.parent == root || tree.parent == 0 {
                return Ok(win);
            }
            win = tree.parent;
        }
        Ok(win)
    }

    /// Window rectangle in desktop coordinates, including the title bar and borders the window
    /// manager reports through `_NET_FRAME_EXTENTS`.
    fn bounds(&self, win: Window) -> Result<PixelRect, CaptureError> {
        let geo = self
            .conn
            .get_geometry(win)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        let origin = self
            .conn
            .translate_coordinates(win, self.root(), 0, 0)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        let (mut x, mut y) = (i32::from(origin.dst_x), i32::from(origin.dst_y));
        let (mut w, mut h) = (u32::from(geo.width), u32::from(geo.height));
        if let [left, right, top, bottom] = self.cardinals(win, "_NET_FRAME_EXTENTS")?[..] {
            x -= left as i32;
            y -= top as i32;
            w += left + right;
            h += top + bottom;
        }
        Ok(PixelRect::new(x, y, w, h))
    }
}

/// `/proc/<pid>/exe`, without the " (deleted)" suffix Linux adds when the file was replaced
/// (as it is by an upgrade while the program runs).
pub(crate) fn exe_of(pid: u32) -> Option<std::path::PathBuf> {
    let target = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let text = target.to_string_lossy();
    match text.strip_suffix(" (deleted)") {
        Some(clean) => Some(clean.into()),
        None => Some(target),
    }
}

impl X11 {
    /// Under Wayland, the XWayland window in focus. The compositor marks an XWayland window
    /// active only while it has the focus and clears the mark when a Wayland window takes it,
    /// so only that mark is trusted here (the X input focus can be left on an old window).
    pub fn xwayland_focused(&self) -> Result<Option<FocusedApp>, CaptureError> {
        match self.net_active_window()? {
            Some(win) => self.app_of(win).map(Some),
            None => Ok(None),
        }
    }

    fn app_of(&self, win: Window) -> Result<FocusedApp, CaptureError> {
        let utf8 = self.atom("UTF8_STRING")?;
        let title = match self.text(win, "_NET_WM_NAME", utf8)? {
            Some(t) => t,
            None => self
                .text(win, "WM_NAME", AtomEnum::STRING.into())?
                .unwrap_or_default(),
        };
        let pid = self.cardinals(win, "_NET_WM_PID")?.first().copied();
        Ok(FocusedApp {
            window: u64::from(win),
            exe_path: pid.and_then(exe_of),
            title,
            pid,
            bounds: self.bounds(win)?,
        })
    }
}

impl Backend for X11 {
    fn focused(&self) -> Result<Option<FocusedApp>, CaptureError> {
        match self.active_window()? {
            Some(win) => self.app_of(win).map(Some),
            None => Ok(None),
        }
    }

    fn pointer_display(&self) -> Result<Option<PixelRect>, CaptureError> {
        let root = self.root();
        let p = self
            .conn
            .query_pointer(root)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        let monitors = self
            .conn
            .randr_get_monitors(root, true)
            .map_err(grab)?
            .reply()
            .map_err(grab)?
            .monitors;
        Ok(monitors
            .iter()
            .map(|m| {
                PixelRect::new(
                    i32::from(m.x),
                    i32::from(m.y),
                    u32::from(m.width),
                    u32::from(m.height),
                )
            })
            .find(|r| r.contains(i32::from(p.root_x), i32::from(p.root_y))))
    }

    fn desktop(&self) -> Result<PixelRect, CaptureError> {
        let s = &self.conn.setup().roots[self.screen];
        Ok(PixelRect::new(
            0,
            0,
            u32::from(s.width_in_pixels),
            u32::from(s.height_in_pixels),
        ))
    }

    fn grab(&self, area: PixelRect) -> Result<Frame, CaptureError> {
        let setup = self.conn.setup();
        let depth = setup.roots[self.screen].root_depth;
        let format = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == depth)
            .ok_or_else(|| {
                CaptureError::Unsupported(format!("no pixel format for depth {depth}"))
            })?;
        if format.bits_per_pixel != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST {
            return Err(CaptureError::Unsupported(format!(
                "{} bits per pixel, byte order {:?}",
                format.bits_per_pixel, setup.image_byte_order
            )));
        }
        let (x, y) = (
            i16::try_from(area.x).map_err(grab)?,
            i16::try_from(area.y).map_err(grab)?,
        );
        let (w, h) = (
            u16::try_from(area.w).map_err(grab)?,
            u16::try_from(area.h).map_err(grab)?,
        );
        let reply = self
            .conn
            .get_image(ImageFormat::Z_PIXMAP, self.root(), x, y, w, h, !0)
            .map_err(grab)?
            .reply()
            .map_err(grab)?;
        let expected = area.w as usize * area.h as usize * 4;
        if reply.data.len() < expected {
            return Err(CaptureError::Grab(format!(
                "the server sent {} bytes, expected {expected}",
                reply.data.len()
            )));
        }
        // Standard 24-bit colour in a 32-bit pixel, little-endian: bytes are B, G, R, unused.
        let mut rgba = reply.data;
        rgba.truncate(expected);
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Frame::new(area.w, area.h, rgba)
    }
}
