//! Wayland: the desktop portal's Screenshot request, which works on GNOME, KDE and wlroots
//! desktops that have a portal with a screenshot service. The desktop may ask the person to allow
//! it the first time. Verified on sway with xdg-desktop-portal-wlr (scripts/e2e/wayland-capture.sh).

use crate::error::{grab, CaptureError};
use crate::frame::Frame;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const OBJECT: &str = "/org/freedesktop/portal/desktop";
/// Long enough for a person to read and answer the desktop's permission question.
const WAIT: Duration = Duration::from_secs(60);

/// The version of one of the portal's services, or `None` when the portal does not offer it.
/// (Asked directly, without a proxy that would complain about a missing service in the log.)
pub fn interface_version(interface: &str) -> Option<u32> {
    let conn = Connection::session().ok()?;
    let reply = conn
        .call_method(
            Some(DESTINATION),
            OBJECT,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(interface, "version"),
        )
        .ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    u32::try_from(&value).ok()
}

/// Whether the desktop portal has a screenshot service on this session bus.
pub fn available() -> bool {
    interface_version("org.freedesktop.portal.Screenshot").is_some()
}

/// `:1.234` becomes `1_234`, the form the portal uses in request paths.
pub fn sender_path_part(unique_name: &str) -> String {
    unique_name.trim_start_matches(':').replace('.', "_")
}

pub fn request_path(unique_name: &str, token: &str) -> String {
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

/// The whole desktop through the portal's Screenshot request.
pub(crate) fn screenshot() -> Result<Frame, CaptureError> {
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
