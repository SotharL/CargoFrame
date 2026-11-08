//! Wayland recording: the desktop portal's ScreenCast service hands out a PipeWire stream of the
//! screen the person picks (GNOME and KDE ask in a dialog; wlroots desktops use their chooser).
//! The stream is read by GStreamer's `pipewiresrc`, outside CargoFrame (see the recorder).
//!
//! With a version 4 portal the choice is remembered: the portal gives a restore token, and the
//! next recording passes it back so the desktop does not ask again.

use crate::error::{grab, CaptureError};
use crate::portal::request_path;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const OBJECT: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
/// The desktop may show a dialog: give the person time to choose.
const PICK_WAIT: Duration = Duration::from_secs(120);
const QUICK_WAIT: Duration = Duration::from_secs(20);

/// Source types and cursor modes of the ScreenCast portal.
const MONITOR: u32 = 1;
const CURSOR_EMBEDDED: u32 = 2;
/// Remember the choice until the person takes it back.
const PERSIST_UNTIL_REVOKED: u32 = 2;

type Results = HashMap<String, OwnedValue>;

/// One PipeWire stream, with where it lies on the desktop (logical pixels) when the portal says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    pub node: u32,
    pub position: Option<(i32, i32)>,
    pub size: Option<(u32, u32)>,
}

/// An open screen cast. The stream lives as long as this does; dropping it ends the session.
pub struct ScreenCast {
    conn: Connection,
    session: OwnedObjectPath,
    pub stream: Stream,
    /// Pass this to the next [`ScreenCast::start`] so the desktop does not ask again.
    pub restore_token: Option<String>,
}

impl Drop for ScreenCast {
    fn drop(&mut self) {
        let _ = Proxy::new(
            &self.conn,
            DESTINATION,
            self.session.as_str(),
            "org.freedesktop.portal.Session",
        )
        .and_then(|p| p.call::<_, _, ()>("Close", &()));
    }
}

/// Whether the portal offers screen casting on this session bus.
pub fn available() -> bool {
    crate::portal::interface_version(INTERFACE).is_some()
}

fn value_str(v: &OwnedValue) -> Option<String> {
    <&str>::try_from(&**v).ok().map(str::to_string)
}

/// Read the `streams` answer of `Start`: `a(ua{sv})`.
pub fn streams(results: &Results) -> Vec<Stream> {
    let Some(list) = results.get("streams").and_then(|v| v.try_clone().ok()) else {
        return Vec::new();
    };
    let Ok(items) = <Vec<(u32, Results)>>::try_from(list) else {
        return Vec::new();
    };
    items
        .into_iter()
        .map(|(node, props)| Stream {
            node,
            position: props
                .get("position")
                .and_then(|v| <(i32, i32)>::try_from(v.try_clone().ok()?).ok()),
            size: props
                .get("size")
                .and_then(|v| <(i32, i32)>::try_from(v.try_clone().ok()?).ok())
                .and_then(|(w, h)| Some((u32::try_from(w).ok()?, u32::try_from(h).ok()?))),
        })
        .collect()
}

struct Asker<'a> {
    conn: &'a Connection,
    unique: String,
    n: u32,
}

impl Asker<'_> {
    fn token(&mut self) -> String {
        self.n += 1;
        format!("cargoframe_cast{}_{}", std::process::id(), self.n)
    }

    /// Send a request and wait for its `Response`.
    fn ask(
        &self,
        token: &str,
        wait: Duration,
        call: impl FnOnce() -> zbus::Result<OwnedObjectPath>,
    ) -> Result<Results, CaptureError> {
        let path = request_path(&self.unique, token);
        let request = Proxy::new(
            self.conn,
            DESTINATION,
            path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .map_err(grab)?;
        let mut responses = request.receive_signal("Response").map_err(grab)?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let answer = responses.next().map(|m| {
                m.body()
                    .deserialize::<(u32, Results)>()
                    .map_err(|e| e.to_string())
            });
            let _ = tx.send(answer);
        });
        call().map_err(grab)?;
        let (code, results) = rx
            .recv_timeout(wait)
            .map_err(|_| {
                CaptureError::Grab("the desktop did not answer the screen-sharing request".into())
            })?
            .ok_or_else(|| {
                CaptureError::Grab("the screen-sharing request ended without an answer".into())
            })?
            .map_err(CaptureError::Grab)?;
        match code {
            0 => Ok(results),
            _ => Err(CaptureError::Refused),
        }
    }
}

impl ScreenCast {
    /// Ask the desktop for a stream of one screen, with the pointer drawn in.
    pub fn start(restore_token: Option<&str>) -> Result<Self, CaptureError> {
        let conn = Connection::session().map_err(|e| CaptureError::NoDisplay(e.to_string()))?;
        let unique = conn
            .unique_name()
            .ok_or_else(|| CaptureError::Grab("no D-Bus name".into()))?
            .to_string();
        let portal = Proxy::new(&conn, DESTINATION, OBJECT, INTERFACE).map_err(grab)?;
        let version = portal.get_property::<u32>("version").map_err(|e| {
            CaptureError::Unsupported(format!(
                "the desktop portal has no screen sharing ({e}). Install xdg-desktop-portal with your desktop's portal"
            ))
        })?;
        let cursors = portal
            .get_property::<u32>("AvailableCursorModes")
            .unwrap_or(0);
        let mut asker = Asker {
            conn: &conn,
            unique,
            n: 0,
        };

        let token = asker.token();
        let session_token = asker.token();
        let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
        opts.insert("handle_token", Value::from(token.as_str()));
        opts.insert("session_handle_token", Value::from(session_token.as_str()));
        let created = asker.ask(&token, QUICK_WAIT, || {
            portal.call("CreateSession", &(opts,))
        })?;
        let session = created
            .get("session_handle")
            .and_then(value_str)
            .and_then(|s| OwnedObjectPath::try_from(s).ok())
            .ok_or_else(|| {
                CaptureError::Grab("the desktop did not open a screen-sharing session".into())
            })?;
        // From here on, the session is closed whatever happens.
        let mut cast = ScreenCast {
            conn: conn.clone(),
            session: session.clone(),
            stream: Stream {
                node: 0,
                position: None,
                size: None,
            },
            restore_token: None,
        };

        let token = asker.token();
        let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
        opts.insert("handle_token", Value::from(token.as_str()));
        opts.insert("types", Value::from(MONITOR));
        opts.insert("multiple", Value::from(false));
        if cursors & CURSOR_EMBEDDED != 0 {
            opts.insert("cursor_mode", Value::from(CURSOR_EMBEDDED));
        }
        if version >= 4 {
            opts.insert("persist_mode", Value::from(PERSIST_UNTIL_REVOKED));
            if let Some(t) = restore_token.filter(|t| !t.is_empty()) {
                opts.insert("restore_token", Value::from(t));
            }
        }
        asker.ask(&token, QUICK_WAIT, || {
            portal.call("SelectSources", &(&session, opts))
        })?;

        let token = asker.token();
        let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
        opts.insert("handle_token", Value::from(token.as_str()));
        let started = asker.ask(&token, PICK_WAIT, || {
            portal.call("Start", &(&session, "", opts))
        })?;
        cast.restore_token = started.get("restore_token").and_then(value_str);
        cast.stream = streams(&started)
            .into_iter()
            .next()
            .ok_or_else(|| CaptureError::Grab("the desktop shared no screen".into()))?;
        Ok(cast)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(v: Value<'_>) -> OwnedValue {
        OwnedValue::try_from(v).unwrap()
    }

    #[test]
    fn the_streams_answer_is_read() {
        let mut props: HashMap<String, OwnedValue> = HashMap::new();
        props.insert("position".into(), owned(Value::from((1280i32, 0i32))));
        props.insert("size".into(), owned(Value::from((1920i32, 1080i32))));
        let list = vec![(57u32, props)];
        let mut results: Results = HashMap::new();
        results.insert("streams".into(), owned(Value::from(list)));
        assert_eq!(
            streams(&results),
            vec![Stream {
                node: 57,
                position: Some((1280, 0)),
                size: Some((1920, 1080)),
            }]
        );
        assert!(streams(&HashMap::new()).is_empty());
    }
}
