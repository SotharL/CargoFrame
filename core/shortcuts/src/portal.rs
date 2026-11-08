//! The desktop portal's GlobalShortcuts service (KDE Plasma 5.27+, GNOME 48+, others).
//!
//! One D-Bus connection lives as long as CargoFrame. Each change of the triggers closes the old
//! portal session and binds the new list in a fresh one (a session binds once); a thread listens
//! for `Activated` and runs the trigger when the signal comes from the current session.

use crate::Binding;
use cargoframe_capture::portal::request_path;
use cargoframe_config::shortcut;
use std::collections::{BTreeMap, HashMap};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub const DESTINATION: &str = "org.freedesktop.portal.Desktop";
pub const OBJECT: &str = "/org/freedesktop/portal/desktop";
pub const INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
/// The desktop may show a dialog to confirm the shortcuts: give the person time to answer.
const BIND_WAIT: Duration = Duration::from_secs(300);
const QUICK_WAIT: Duration = Duration::from_secs(20);

type Results = HashMap<String, OwnedValue>;

pub struct Portal {
    conn: Connection,
    session: Arc<Mutex<Option<OwnedObjectPath>>>,
    counter: Mutex<u64>,
}

/// Whether the portal offers GlobalShortcuts on this session bus.
pub fn available() -> bool {
    cargoframe_capture::portal::interface_version(INTERFACE).is_some_and(|v| v >= 1)
}

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Portal {
    /// Connect, and call `pressed` with the trigger id each time a bound shortcut is pressed.
    pub fn start(pressed: impl Fn(String) + Send + 'static) -> Result<Self, String> {
        Self::start_on(Connection::session().map_err(text)?, pressed)
    }

    /// The same on a given connection (tests use a private bus).
    pub fn start_on(
        conn: Connection,
        pressed: impl Fn(String) + Send + 'static,
    ) -> Result<Self, String> {
        let session: Arc<Mutex<Option<OwnedObjectPath>>> = Arc::new(Mutex::new(None));
        let proxy = Proxy::new(&conn, DESTINATION, OBJECT, INTERFACE).map_err(text)?;
        let signals = proxy.receive_signal("Activated").map_err(text)?;
        let current = session.clone();
        std::thread::Builder::new()
            .name("portal-shortcuts".into())
            .spawn(move || {
                for msg in signals {
                    let Ok((from, id, _time, _opts)) =
                        msg.body()
                            .deserialize::<(OwnedObjectPath, String, u64, Results)>()
                    else {
                        continue;
                    };
                    let ours = current
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_ref()
                        .is_some_and(|s| *s == from);
                    if ours {
                        pressed(id);
                    }
                }
            })
            .map_err(text)?;
        Ok(Self {
            conn,
            session,
            counter: Mutex::new(0),
        })
    }

    fn token(&self) -> String {
        let mut n = self.counter.lock().unwrap_or_else(|e| e.into_inner());
        *n += 1;
        format!("cargoframe{}_{}", std::process::id(), *n)
    }

    /// Send a request and wait for its answer (`Response` on the request's own path).
    fn request(
        &self,
        token: &str,
        wait: Duration,
        call: impl FnOnce() -> zbus::Result<OwnedObjectPath>,
    ) -> Result<Results, String> {
        let unique = self.conn.unique_name().ok_or("no D-Bus name")?.to_string();
        let path = request_path(&unique, token);
        let request = Proxy::new(
            &self.conn,
            DESTINATION,
            path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .map_err(text)?;
        let mut responses = request.receive_signal("Response").map_err(text)?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let answer = responses
                .next()
                .map(|m| m.body().deserialize::<(u32, Results)>().map_err(text));
            let _ = tx.send(answer);
        });
        call().map_err(text)?;
        let (code, results) = rx
            .recv_timeout(wait)
            .map_err(|_| "the desktop did not answer".to_string())?
            .ok_or("the request ended without an answer")??;
        match code {
            0 => Ok(results),
            1 => Err("you did not confirm the shortcuts".into()),
            _ => Err("the desktop refused the shortcuts".into()),
        }
    }

    /// Bind `wanted`, replacing what was bound before. Returns, per trigger, the keys the desktop
    /// says it uses (they may differ from the ones asked for).
    pub fn bind(&self, wanted: &[Binding]) -> Result<BTreeMap<String, String>, String> {
        let proxy = Proxy::new(&self.conn, DESTINATION, OBJECT, INTERFACE).map_err(text)?;
        if let Some(old) = self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = Proxy::new(
                &self.conn,
                DESTINATION,
                old.as_str(),
                "org.freedesktop.portal.Session",
            )
            .and_then(|p| p.call::<_, _, ()>("Close", &()));
        }
        if wanted.is_empty() {
            return Ok(BTreeMap::new());
        }

        let token = self.token();
        let session_token = self.token();
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        options.insert("handle_token", Value::from(token.as_str()));
        options.insert("session_handle_token", Value::from(session_token.as_str()));
        let created = self.request(&token, QUICK_WAIT, || {
            proxy.call("CreateSession", &(options,))
        })?;
        let session: OwnedObjectPath = created
            .get("session_handle")
            .and_then(|v| {
                <&str>::try_from(&**v)
                    .ok()
                    .and_then(|s| OwnedObjectPath::try_from(s).ok())
                    .or_else(|| OwnedObjectPath::try_from(v.try_clone().ok()?).ok())
            })
            .ok_or("the desktop did not open a shortcut session")?;

        let shortcuts: Vec<(String, HashMap<&str, Value<'_>>)> = wanted
            .iter()
            .map(|b| {
                let mut o: HashMap<&str, Value<'_>> = HashMap::new();
                o.insert(
                    "description",
                    Value::from(format!("CargoFrame: {}", b.name)),
                );
                o.insert(
                    "preferred_trigger",
                    Value::from(shortcut::for_portal(&b.shortcut)),
                );
                (b.id.clone(), o)
            })
            .collect();
        let token = self.token();
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        options.insert("handle_token", Value::from(token.as_str()));
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session.clone());
        let bound = self.request(&token, BIND_WAIT, || {
            proxy.call("BindShortcuts", &(&session, shortcuts, "", options))
        })?;

        let mut keys = BTreeMap::new();
        if let Some(list) = bound.get("shortcuts") {
            if let Ok(items) = <Vec<(String, Results)>>::try_from(list.try_clone().map_err(text)?) {
                for (id, props) in items {
                    let described = props
                        .get("trigger_description")
                        .and_then(|v| <&str>::try_from(&**v).ok())
                        .unwrap_or("")
                        .to_string();
                    keys.insert(id, described);
                }
            }
        }
        Ok(keys)
    }
}

#[cfg(test)]
#[path = "portal_tests.rs"]
mod tests;
