//! Wayland: the desktop portal's Screenshot request, which works on GNOME and KDE.
//!
//! Wayland lets programs neither read the screen nor ask which window has the focus, so this
//! backend captures the whole desktop and reports no focused window. The desktop may ask the
//! person to allow it the first time. Not yet verified on a real session (spike S3).

use crate::error::{grab, CaptureError};
use crate::frame::{Frame, PixelRect};
use crate::policy::{Backend, FocusedApp};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const OBJECT: &str = "/org/freedesktop/portal/desktop";
/// Long enough for a person to read and answer the desktop's permission question.
const WAIT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct Portal {
    cached: RefCell<Option<Frame>>,
}

impl Portal {
    pub fn new() -> Self {
        Self::default()
    }

    fn whole_screen(&self) -> Result<Frame, CaptureError> {
        if let Some(f) = self.cached.borrow().as_ref() {
            return Ok(f.clone());
        }
        let frame = screenshot()?;
        *self.cached.borrow_mut() = Some(frame.clone());
        Ok(frame)
    }
}

impl Backend for Portal {
    fn focused(&self) -> Result<Option<FocusedApp>, CaptureError> {
        Ok(None)
    }

    fn pointer_display(&self) -> Result<Option<PixelRect>, CaptureError> {
        Ok(None)
    }

    fn desktop(&self) -> Result<PixelRect, CaptureError> {
        let f = self.whole_screen()?;
        Ok(PixelRect::new(0, 0, f.width(), f.height()))
    }

    fn grab(&self, area: PixelRect) -> Result<Frame, CaptureError> {
        self.whole_screen()?
            .crop(area)
            .ok_or_else(|| CaptureError::Grab("the area is outside the screenshot".into()))
    }
}

/// `:1.234` becomes `1_234`, the form the portal uses in request paths.
pub(crate) fn sender_path_part(unique_name: &str) -> String {
    unique_name.trim_start_matches(':').replace('.', "_")
}

pub(crate) fn request_path(unique_name: &str, token: &str) -> String {
    format!(
        "/org/freedesktop/portal/desktop/request/{}/{token}",
        sender_path_part(unique_name)
    )
}

/// `file:///home/me/Pictures/Screenshot%20from%202026.png` to a path.
pub(crate) fn path_from_uri(uri: &str) -> Option<std::path::PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // An optional host ("localhost") may sit before the path.
    let rest = &rest[rest.find('/')?..];
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(std::path::PathBuf::from(String::from_utf8(out).ok()?))
}

fn screenshot() -> Result<Frame, CaptureError> {
    let conn = Connection::session().map_err(|e| CaptureError::NoDisplay(e.to_string()))?;
    let unique = conn
        .unique_name()
        .ok_or_else(|| CaptureError::Grab("no D-Bus name".into()))?
        .to_string();
    let token = format!(
        "cargoframe{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let path = request_path(&unique, &token);

    // Listen for the answer before asking, so it cannot be missed.
    let request = Proxy::new(
        &conn,
        DESTINATION,
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(grab)?;
    let mut responses = request.receive_signal("Response").map_err(grab)?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let answer = responses.next().map(|msg| {
            msg.body()
                .deserialize::<(u32, HashMap<String, OwnedValue>)>()
                .map_err(|e| e.to_string())
        });
        let _ = tx.send(answer);
    });

    let portal = Proxy::new(
        &conn,
        DESTINATION,
        OBJECT,
        "org.freedesktop.portal.Screenshot",
    )
    .map_err(grab)?;
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("interactive", Value::from(false));
    let _handle: zbus::zvariant::OwnedObjectPath =
        portal.call("Screenshot", &("", options)).map_err(grab)?;

    let answer = rx
        .recv_timeout(WAIT)
        .map_err(|_| {
            CaptureError::Grab("the desktop did not answer the screenshot request".into())
        })?
        .ok_or_else(|| CaptureError::Grab("the screenshot request ended without an answer".into()))?
        .map_err(CaptureError::Grab)?;
    let (code, results) = answer;
    if code != 0 {
        return Err(CaptureError::Refused);
    }
    let uri: String = results
        .get("uri")
        .and_then(|v| <&str>::try_from(&**v).ok())
        .map(str::to_owned)
        .ok_or_else(|| CaptureError::Grab("the answer has no picture".into()))?;
    let file = path_from_uri(&uri)
        .ok_or_else(|| CaptureError::Grab(format!("unexpected picture location: {uri}")))?;
    let bytes = std::fs::read(&file).map_err(grab)?;
    // The portal leaves the file in the person's Pictures folder; it was only a handover.
    let _ = std::fs::remove_file(&file);
    Frame::from_png(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_paths_follow_the_portal_convention() {
        assert_eq!(
            request_path(":1.42", "abc"),
            "/org/freedesktop/portal/desktop/request/1_42/abc"
        );
    }

    #[test]
    fn file_uris_are_decoded() {
        assert_eq!(
            path_from_uri("file:///home/me/Pictures/Screenshot%20from%202026.png"),
            Some("/home/me/Pictures/Screenshot from 2026.png".into())
        );
        assert_eq!(
            path_from_uri("file://localhost/tmp/a.png"),
            Some("/tmp/a.png".into())
        );
        assert_eq!(path_from_uri("https://example.com/a.png"), None);
        assert_eq!(path_from_uri("file:///tmp/%zz.png"), None);
    }
}
