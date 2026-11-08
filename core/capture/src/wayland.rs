//! Wayland: the picture from `grim` (wlroots desktops) or the desktop portal, and the window in
//! focus from the compositor itself, because Wayland has no common way to ask for it.
//!
//! | Desktop | Window in focus | Picture |
//! |---------|-----------------|---------|
//! | sway | its IPC socket (`SWAYSOCK`) | `grim`, else the portal |
//! | Hyprland | its IPC socket | `grim`, else the portal |
//! | KDE Plasma | a one-line KWin script that calls CargoFrame back over D-Bus | the portal |
//! | GNOME and others | only XWayland programs (their `_NET_ACTIVE_WINDOW`) | the portal |
//!
//! Compositors give window places in *logical* pixels; the picture is in real pixels. With a
//! scaled screen (HiDPI) the places are scaled to the picture before cropping.

use crate::error::CaptureError;
use crate::frame::{Frame, PixelRect};
use crate::policy::{Backend, FocusedApp};
use serde_json::Value;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The compositor of this Wayland session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compositor {
    Sway { socket: PathBuf },
    Hyprland { socket: PathBuf },
    Kde,
    Gnome,
    Other,
}

impl Compositor {
    /// Decide from the environment (`get` reads one variable).
    pub fn detect(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let var = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        if let Some(sock) = var("SWAYSOCK") {
            return Self::Sway {
                socket: sock.into(),
            };
        }
        if let Some(sig) = var("HYPRLAND_INSTANCE_SIGNATURE") {
            // Hyprland 0.40 moved its sockets from /tmp/hypr to the runtime folder.
            let runtime = var("XDG_RUNTIME_DIR").map(PathBuf::from);
            let new = runtime.map(|r| r.join("hypr").join(&sig).join(".socket.sock"));
            let old = PathBuf::from("/tmp/hypr").join(&sig).join(".socket.sock");
            let socket = match new {
                Some(n) if n.exists() || !old.exists() => n,
                _ => old,
            };
            return Self::Hyprland { socket };
        }
        let desktop = var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase();
        let parts: Vec<&str> = desktop.split(':').collect();
        if parts.contains(&"KDE") {
            Self::Kde
        } else if parts.iter().any(|p| p.contains("GNOME")) {
            Self::Gnome
        } else {
            Self::Other
        }
    }

    pub fn detect_env() -> Self {
        Self::detect(&|k| std::env::var(k).ok())
    }

    /// Compositors built on wlroots, where `grim` takes the picture without a portal.
    pub fn is_wlroots(&self) -> bool {
        matches!(self, Self::Sway { .. } | Self::Hyprland { .. })
    }

    /// The desktop's name, for messages.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sway { .. } => "sway",
            Self::Hyprland { .. } => "Hyprland",
            Self::Kde => "KDE Plasma",
            Self::Gnome => "GNOME",
            Self::Other => "this desktop",
        }
    }

    /// Whether this desktop can tell CargoFrame which Wayland window is in focus.
    pub fn knows_focus(&self) -> bool {
        matches!(self, Self::Sway { .. } | Self::Hyprland { .. } | Self::Kde)
    }
}

/// What the compositor says about its screens and the window in focus, in logical pixels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// The window in focus; `None` when nothing has the focus.
    pub focused: Option<FocusedApp>,
    /// The screen in focus (sway, Hyprland), or the one the window is on.
    pub output: Option<PixelRect>,
    /// All screens together.
    pub desktop: Option<PixelRect>,
}

/// The rectangle that holds all of `rects`.
fn union(rects: &[PixelRect]) -> Option<PixelRect> {
    let x0 = rects.iter().map(|r| r.x).min()?;
    let y0 = rects.iter().map(|r| r.y).min()?;
    let x1 = rects.iter().map(|r| r.right()).max()?;
    let y1 = rects.iter().map(|r| r.bottom()).max()?;
    Some(PixelRect::new(
        x0,
        y0,
        u32::try_from(x1 - i64::from(x0)).ok()?,
        u32::try_from(y1 - i64::from(y0)).ok()?,
    ))
}

fn rect_of(v: &Value) -> Option<PixelRect> {
    let n = |k: &str| v.get(k).and_then(Value::as_f64);
    Some(PixelRect::new(
        n("x")?.round() as i32,
        n("y")?.round() as i32,
        n("width")?.round().max(0.0) as u32,
        n("height")?.round().max(0.0) as u32,
    ))
}

fn app(pid: Option<u32>, title: String, bounds: PixelRect, id: u64) -> FocusedApp {
    FocusedApp {
        window: id,
        exe_path: pid.and_then(crate::x11::exe_of),
        title,
        pid,
        bounds,
    }
}

// ---------------------------------------------------------------- sway

const SWAY_GET_OUTPUTS: u32 = 3;
const SWAY_GET_TREE: u32 = 4;

/// sway's command request (`RUN_COMMAND`).
pub const SWAY_RUN_COMMAND: u32 = 0;

/// One request on sway's IPC socket: `i3-ipc`, the length and the type, then the answer.
pub fn sway_request(socket: &Path, kind: u32, payload: &str) -> std::io::Result<Value> {
    use std::io::{Error, ErrorKind};
    let mut s = UnixStream::connect(socket)?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut msg = b"i3-ipc".to_vec();
    msg.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    msg.extend_from_slice(&kind.to_ne_bytes());
    msg.extend_from_slice(payload.as_bytes());
    s.write_all(&msg)?;
    let mut head = [0u8; 14];
    s.read_exact(&mut head)?;
    if &head[..6] != b"i3-ipc" {
        return Err(Error::new(ErrorKind::InvalidData, "not an i3-ipc answer"));
    }
    let len = u32::from_ne_bytes([head[6], head[7], head[8], head[9]]) as usize;
    if len > 64 * 1024 * 1024 {
        return Err(Error::new(ErrorKind::InvalidData, "the answer is too long"));
    }
    let mut body = vec![0u8; len];
    s.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|e| Error::new(ErrorKind::InvalidData, e))
}

fn sway_ask(socket: &Path, kind: u32) -> Result<Value, CaptureError> {
    sway_request(socket, kind, "")
        .map_err(|e| CaptureError::FocusUnknown(format!("sway did not answer: {e}")))
}

/// sway's `rect` leaves out the title bar it draws above a window (`deco_rect`); the picture
/// keeps it, as on X11 and Windows.
fn with_title_bar(r: PixelRect, deco: Option<&Value>) -> PixelRect {
    let h = deco
        .and_then(|d| d.get("height"))
        .and_then(Value::as_u64)
        .and_then(|h| u32::try_from(h).ok())
        .unwrap_or(0);
    if h == 0 || h > r.h {
        return r;
    }
    PixelRect::new(r.x, r.y - h as i32, r.w, r.h + h)
}

/// The focused window and screens from sway's tree and outputs.
pub fn sway_layout(tree: &Value, outputs: &Value) -> Layout {
    fn find(node: &Value) -> Option<&Value> {
        if node.get("focused").and_then(Value::as_bool) == Some(true) {
            return Some(node);
        }
        ["nodes", "floating_nodes"]
            .iter()
            .filter_map(|k| node.get(*k).and_then(Value::as_array))
            .flatten()
            .find_map(find)
    }
    let screens: Vec<(PixelRect, bool)> = outputs
        .as_array()
        .into_iter()
        .flatten()
        .filter(|o| o.get("active").and_then(Value::as_bool) != Some(false))
        .filter_map(|o| {
            Some((
                rect_of(o.get("rect")?)?,
                o.get("focused").and_then(Value::as_bool) == Some(true),
            ))
        })
        .collect();
    let desktop = union(&screens.iter().map(|(r, _)| *r).collect::<Vec<_>>());
    let output = screens.iter().find(|(_, f)| *f).map(|(r, _)| *r);
    // Only a window is an application: a focused workspace or output means "nothing".
    let focused = find(tree)
        .filter(|n| {
            matches!(
                n.get("type").and_then(Value::as_str),
                Some("con" | "floating_con")
            )
        })
        .filter(|n| {
            n.get("pid").is_some() || n.get("app_id").is_some() || n.get("window").is_some()
        })
        .and_then(|n| {
            Some(app(
                n.get("pid")
                    .and_then(Value::as_u64)
                    .and_then(|p| u32::try_from(p).ok()),
                n.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                with_title_bar(rect_of(n.get("rect")?)?, n.get("deco_rect")),
                n.get("id").and_then(Value::as_u64).unwrap_or(0),
            ))
        });
    Layout {
        focused,
        output,
        desktop,
    }
}

// ---------------------------------------------------------------- Hyprland

/// One request on Hyprland's IPC socket; the answer is read to the end.
pub fn hypr_request(socket: &Path, command: &str) -> std::io::Result<Vec<u8>> {
    let mut s = UnixStream::connect(socket)?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    s.write_all(command.as_bytes())?;
    let mut body = Vec::new();
    s.take(64 * 1024 * 1024).read_to_end(&mut body)?;
    Ok(body)
}

fn hypr_ask(socket: &Path, command: &str) -> Result<Value, CaptureError> {
    let body = hypr_request(socket, command)
        .map_err(|e| CaptureError::FocusUnknown(format!("Hyprland did not answer: {e}")))?;
    serde_json::from_slice(&body)
        .map_err(|e| CaptureError::FocusUnknown(format!("Hyprland's answer is not readable: {e}")))
}

/// The focused window and screens from `j/activewindow` and `j/monitors`. Monitor sizes are in
/// real pixels; places (and window sizes) are logical.
pub fn hypr_layout(active: &Value, monitors: &Value) -> Layout {
    let screens: Vec<(PixelRect, bool)> = monitors
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let n = |k: &str| m.get(k).and_then(Value::as_f64);
            let scale = n("scale").filter(|s| *s > 0.0).unwrap_or(1.0);
            // a monitor turned a quarter swaps width and height
            let turned = m.get("transform").and_then(Value::as_u64).unwrap_or(0) % 2 == 1;
            let (mut w, mut h) = (n("width")? / scale, n("height")? / scale);
            if turned {
                std::mem::swap(&mut w, &mut h);
            }
            Some((
                PixelRect::new(
                    n("x")?.round() as i32,
                    n("y")?.round() as i32,
                    w.round() as u32,
                    h.round() as u32,
                ),
                m.get("focused").and_then(Value::as_bool) == Some(true),
            ))
        })
        .collect();
    let desktop = union(&screens.iter().map(|(r, _)| *r).collect::<Vec<_>>());
    let output = screens.iter().find(|(_, f)| *f).map(|(r, _)| *r);
    let pair = |k: &str| -> Option<(f64, f64)> {
        let a = active.get(k)?.as_array()?;
        Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
    };
    let focused = (|| {
        let (x, y) = pair("at")?;
        let (w, h) = pair("size")?;
        let id = active
            .get("address")
            .and_then(Value::as_str)
            .and_then(|a| u64::from_str_radix(a.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);
        Some(app(
            active
                .get("pid")
                .and_then(Value::as_i64)
                .and_then(|p| u32::try_from(p).ok()),
            active
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            PixelRect::new(
                x.round() as i32,
                y.round() as i32,
                w.round() as u32,
                h.round() as u32,
            ),
            id,
        ))
    })();
    Layout {
        focused,
        output,
        desktop,
    }
}

// ---------------------------------------------------------------- KDE

/// The KWin script: it finds the active window (Plasma 6 and 5) and calls CargoFrame back with
/// one JSON string, so the call does not depend on how KWin converts numbers.
pub fn kwin_script(bus_name: &str) -> String {
    format!(
        r#"(function () {{
  var out = {{}};
  try {{
    var w = workspace.activeWindow || workspace.activeClient;
    if (w) {{
      var g = w.frameGeometry;
      out.window = {{ pid: w.pid, title: w.caption, x: g.x, y: g.y, width: g.width, height: g.height }};
    }}
    var v = workspace.virtualScreenSize;
    if (v) {{ out.desktop = {{ x: 0, y: 0, width: v.width, height: v.height }}; }}
  }} catch (e) {{ out.error = String(e); }}
  callDBus("{bus_name}", "{KWIN_REPLY_PATH}", "{KWIN_REPLY_INTERFACE}", "Window", JSON.stringify(out));
}})();
"#
    )
}

pub const KWIN_REPLY_PATH: &str = "/org/cargoframe/Focus";
pub const KWIN_REPLY_INTERFACE: &str = "org.cargoframe.Focus";

/// Read the script's answer.
pub fn kwin_layout(json: &str) -> Result<Layout, CaptureError> {
    let v: Value = serde_json::from_str(json)
        .map_err(|e| CaptureError::FocusUnknown(format!("KWin's answer is not readable: {e}")))?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        tracing::debug!("the KWin script failed: {e}");
    }
    let focused = v.get("window").and_then(|w| {
        Some(app(
            w.get("pid")
                .and_then(Value::as_f64)
                .filter(|p| *p > 0.0)
                .map(|p| p as u32),
            w.get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            rect_of(w)?,
            0,
        ))
    });
    Ok(Layout {
        focused,
        output: None,
        desktop: v.get("desktop").and_then(rect_of),
    })
}

pub(crate) mod kwin {
    use super::{kwin_layout, kwin_script, Layout, KWIN_REPLY_PATH};
    use crate::error::CaptureError;
    use std::sync::mpsc;
    use std::time::Duration;
    use zbus::blocking::{connection, Proxy};

    struct Reply(mpsc::Sender<String>);

    #[zbus::interface(name = "org.cargoframe.Focus")]
    impl Reply {
        fn window(&self, json: String) {
            let _ = self.0.send(json);
        }
    }

    fn unknown<E: std::fmt::Display>(e: E) -> CaptureError {
        CaptureError::FocusUnknown(format!(
            "KDE did not say which window is in focus ({e}), so the whole screen was captured."
        ))
    }

    /// Ask KWin, through a short script, for the window in focus.
    pub fn layout(wait: Duration) -> Result<Layout, CaptureError> {
        layout_on(connection::Builder::session().map_err(unknown)?, wait)
    }

    /// The same on a given bus (tests use a private one).
    pub fn layout_on(bus: connection::Builder<'_>, wait: Duration) -> Result<Layout, CaptureError> {
        let (tx, rx) = mpsc::channel();
        let conn = bus
            .serve_at(KWIN_REPLY_PATH, Reply(tx))
            .map_err(unknown)?
            .build()
            .map_err(unknown)?;
        let me = conn
            .unique_name()
            .ok_or_else(|| unknown("no D-Bus name"))?
            .to_string();
        let dir = tempfile::Builder::new()
            .prefix("cargoframe-kwin")
            .tempdir()
            .map_err(unknown)?;
        let file = dir.path().join("focus.js");
        std::fs::write(&file, kwin_script(&me)).map_err(unknown)?;
        let plugin = format!("cargoframe-focus-{}", std::process::id());

        let scripting = Proxy::new(
            &conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )
        .map_err(unknown)?;
        // A script left over by an earlier attempt would make loading fail.
        let _: Result<bool, _> = scripting.call("unloadScript", &(plugin.as_str(),));
        let id: i32 = scripting
            .call(
                "loadScript",
                &(file.to_string_lossy().as_ref(), plugin.as_str()),
            )
            .map_err(unknown)?;
        // Plasma 6 puts the script at /Scripting/Script<id>, Plasma 5 at /<id>.
        let ran = [format!("/Scripting/Script{id}"), format!("/{id}")]
            .iter()
            .any(|path| {
                Proxy::new(&conn, "org.kde.KWin", path.as_str(), "org.kde.kwin.Script")
                    .and_then(|p| p.call::<_, _, ()>("run", &()))
                    .is_ok()
            });
        let answer = if ran {
            rx.recv_timeout(wait).map_err(|_| unknown("no answer"))
        } else {
            Err(unknown("the script did not start"))
        };
        let _: Result<bool, _> = scripting.call("unloadScript", &(plugin.as_str(),));
        kwin_layout(&answer?)
    }
}

// ---------------------------------------------------------------- the picture

/// A program on the PATH.
pub fn find_program(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// The whole desktop with `grim` (wlroots screencopy), as a PNG on its standard output.
fn grim(program: &Path) -> Result<Frame, CaptureError> {
    let out = std::process::Command::new(program)
        .args(["-t", "png", "-l", "0", "-"])
        .output()
        .map_err(|e| CaptureError::Grab(format!("grim could not start: {e}")))?;
    if !out.status.success() {
        return Err(CaptureError::Grab(format!(
            "grim failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Frame::from_png(&out.stdout)
}

/// The whole desktop: `grim` first on wlroots desktops (no portal, no question), the portal
/// otherwise, and `grim` again when the portal has no screenshot service.
fn whole_desktop(compositor: &Compositor) -> Result<Frame, CaptureError> {
    let grim_program = find_program("grim");
    if compositor.is_wlroots() {
        if let Some(g) = &grim_program {
            match grim(g) {
                Ok(f) => return Ok(f),
                Err(e) => tracing::debug!("grim did not work, asking the portal: {e}"),
            }
        }
    }
    match crate::portal::screenshot() {
        Ok(f) => Ok(f),
        Err(CaptureError::Refused) => Err(CaptureError::Refused),
        Err(e) => match &grim_program {
            Some(g) if !compositor.is_wlroots() => grim(g).map_err(|_| e),
            _ => Err(match e {
                CaptureError::NoDisplay(_) | CaptureError::Grab(_) => CaptureError::Grab(format!(
                    "{e}. Install xdg-desktop-portal with your desktop's portal (or grim on sway and Hyprland)"
                )),
                other => other,
            }),
        },
    }
}

// ---------------------------------------------------------------- the backend

/// Scale a logical rectangle into the picture, whose real size covers `logical`.
pub fn to_pixels(r: PixelRect, logical: PixelRect, picture: (u32, u32)) -> PixelRect {
    if logical.w == 0 || logical.h == 0 {
        return r;
    }
    let sx = f64::from(picture.0) / f64::from(logical.w);
    let sy = f64::from(picture.1) / f64::from(logical.h);
    let x = (f64::from(r.x - logical.x) * sx).round();
    let y = (f64::from(r.y - logical.y) * sy).round();
    PixelRect::new(
        x as i32,
        y as i32,
        (f64::from(r.w) * sx).round() as u32,
        (f64::from(r.h) * sy).round() as u32,
    )
}

/// The layout of the screens and the window in focus, as this desktop tells it.
pub fn layout(compositor: &Compositor) -> Result<Layout, CaptureError> {
    match compositor {
        Compositor::Sway { socket } => Ok(sway_layout(
            &sway_ask(socket, SWAY_GET_TREE)?,
            &sway_ask(socket, SWAY_GET_OUTPUTS)?,
        )),
        Compositor::Hyprland { socket } => Ok(hypr_layout(
            &hypr_ask(socket, "j/activewindow")?,
            &hypr_ask(socket, "j/monitors")?,
        )),
        Compositor::Kde => kwin::layout(Duration::from_millis(1500)),
        Compositor::Gnome | Compositor::Other => xwayland_layout(compositor),
    }
}

/// Only XWayland programs can be found on GNOME and other desktops.
fn xwayland_layout(compositor: &Compositor) -> Result<Layout, CaptureError> {
    let why = match compositor {
        Compositor::Gnome => "GNOME does not tell other programs which window is in focus, so the whole screen was captured.",
        _ => "This desktop does not tell other programs which window is in focus, so the whole screen was captured.",
    };
    let has_x = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
    if !has_x {
        return Err(CaptureError::FocusUnknown(why.into()));
    }
    let x = crate::x11::X11::connect().map_err(|_| CaptureError::FocusUnknown(why.into()))?;
    match x.xwayland_focused() {
        Ok(Some(app)) => Ok(Layout {
            focused: Some(app),
            output: None,
            desktop: Backend::desktop(&x).ok(),
        }),
        _ => Err(CaptureError::FocusUnknown(why.into())),
    }
}

/// The Wayland backend: one picture of the whole desktop, cropped to what is asked.
pub struct Wayland {
    compositor: Compositor,
    picture: RefCell<Option<Frame>>,
    layout: RefCell<Option<Result<Layout, String>>>,
}

impl Wayland {
    pub fn new(compositor: Compositor) -> Self {
        Self {
            compositor,
            picture: RefCell::new(None),
            layout: RefCell::new(None),
        }
    }

    fn picture(&self) -> Result<Frame, CaptureError> {
        if let Some(f) = self.picture.borrow().as_ref() {
            return Ok(f.clone());
        }
        let f = whole_desktop(&self.compositor)?;
        *self.picture.borrow_mut() = Some(f.clone());
        Ok(f)
    }

    fn layout(&self) -> Result<Layout, CaptureError> {
        if self.layout.borrow().is_none() {
            let got = layout(&self.compositor).map_err(|e| match e {
                CaptureError::FocusUnknown(why) => why,
                other => format!(
                    "{} did not say which window is in focus ({other}), so the whole screen was captured.",
                    self.compositor.name()
                ),
            });
            *self.layout.borrow_mut() = Some(got);
        }
        self.layout
            .borrow()
            .clone()
            .expect("just set")
            .map_err(CaptureError::FocusUnknown)
    }

    /// A logical rectangle in the picture's pixels.
    fn scaled(&self, r: PixelRect, layout: &Layout) -> Result<PixelRect, CaptureError> {
        let f = self.picture()?;
        let logical = layout
            .desktop
            .unwrap_or_else(|| PixelRect::new(0, 0, f.width(), f.height()));
        Ok(to_pixels(r, logical, (f.width(), f.height())))
    }
}

impl Backend for Wayland {
    fn focused(&self) -> Result<Option<FocusedApp>, CaptureError> {
        let layout = self.layout()?;
        match layout.focused.clone() {
            Some(mut app) => {
                app.bounds = self.scaled(app.bounds, &layout)?;
                Ok(Some(app))
            }
            None => Ok(None),
        }
    }

    fn pointer_display(&self) -> Result<Option<PixelRect>, CaptureError> {
        // Wayland does not say where the pointer is: the screen in focus stands in for it.
        match self.layout() {
            Ok(l) => match l.output {
                Some(o) => self.scaled(o, &l).map(Some),
                None => Ok(None),
            },
            Err(_) => Ok(None),
        }
    }

    fn desktop(&self) -> Result<PixelRect, CaptureError> {
        let f = self.picture()?;
        Ok(PixelRect::new(0, 0, f.width(), f.height()))
    }

    fn grab(&self, area: PixelRect) -> Result<Frame, CaptureError> {
        self.picture()?
            .crop(area)
            .ok_or_else(|| CaptureError::Grab("the area is outside the screenshot".into()))
    }
}

/// The window in focus, without a picture (for checks such as "is Claude in front?").
pub fn focused_app(compositor: &Compositor) -> Result<Option<FocusedApp>, CaptureError> {
    Ok(layout(compositor)?.focused)
}

#[cfg(test)]
#[path = "wayland_tests.rs"]
mod tests;
