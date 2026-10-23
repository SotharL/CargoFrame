//! The connections the person has set up, as the rest of the app sees them: whether each is
//! signed in, what it can fill, which lists it offers, and the operations behind the Connections
//! page (set a secret, sign in, sign out, test). Nothing here knows about Tauri, so the whole
//! thing is tested with a fake network and an in-memory credential store.

use cargoframe_config::{Kind as ConfigKind, Store};
use cargoframe_connectors::{
    self as conn, Browser, Clipboard, Connection, Connector, Ctx, Describe, Kind, OptionItem,
    Progress, SecretStore, Status,
};
use cargoframe_layout::{ConnectionInfo, ConnectionKind, ConnectionState};
use cargoframe_net::Http;
use cargoframe_safe::SendError;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a fetched dropdown list is reused. Lists are small and change rarely; this keeps a
/// capture snappy without showing something stale for long.
const OPTIONS_TTL: Duration = Duration::from_secs(120);

#[derive(Default)]
struct Cache {
    status: BTreeMap<String, Status>,
    /// Connections whose sign-in the service rejected. Cleared by signing in again.
    expired: BTreeSet<String>,
    options: BTreeMap<(String, String), (Instant, Vec<OptionItem>)>,
}

pub struct Connections {
    http: Arc<dyn Http>,
    secrets: Arc<dyn SecretStore>,
    cache: Mutex<Cache>,
    sign_in_cancelled: AtomicBool,
}

struct NoBrowser;
impl Browser for NoBrowser {
    fn open(&self, _url: &str) -> Result<(), String> {
        Err("a browser is not available here".into())
    }
}

struct NoClipboard;
impl Clipboard for NoClipboard {
    fn copy_image(&self, _png: &[u8]) -> Result<(), String> {
        Err("the clipboard is not available here".into())
    }
    fn copy_text(&self, _text: &str) -> Result<(), String> {
        Err("the clipboard is not available here".into())
    }
}

pub fn layout_kind(kind: Kind) -> ConnectionKind {
    match kind {
        Kind::Jira => ConnectionKind::Jira,
        Kind::Google => ConnectionKind::Google,
        Kind::Reddit => ConnectionKind::Reddit,
        Kind::Claude => ConnectionKind::Claude,
        Kind::Custom => ConnectionKind::CustomApi,
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What the Connections page shows for one connection.
#[derive(Debug, Serialize)]
pub struct ConnectionView {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub kind_label: &'static str,
    pub enabled: bool,
    /// `connected`, `disconnected`, `expired` or `disabled`.
    pub state: &'static str,
    /// Which of the secrets the person types are stored (never their values).
    pub secrets: Vec<SecretFlag>,
    pub describe: Describe,
}

#[derive(Debug, Serialize)]
pub struct SecretFlag {
    pub name: &'static str,
    pub stored: bool,
}

#[derive(Debug, Serialize)]
pub struct ConnectionsView {
    pub items: Vec<ConnectionView>,
    /// Files that could not be read.
    pub problems: Vec<String>,
    /// Set when the operating system's credential store cannot be used.
    pub store_problem: Option<String>,
}

fn state_name(s: ConnectionState, enabled: bool) -> &'static str {
    if !enabled {
        return "disabled";
    }
    match s {
        ConnectionState::Connected => "connected",
        ConnectionState::Disconnected => "disconnected",
        ConnectionState::Expired => "expired",
    }
}

impl Connections {
    pub fn new(http: Arc<dyn Http>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            http,
            secrets,
            cache: Mutex::new(Cache::default()),
            sign_in_cancelled: AtomicBool::new(false),
        }
    }

    pub fn secrets(&self) -> &dyn SecretStore {
        self.secrets.as_ref()
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn load(&self, store: &Store, id: &str) -> Result<Connection, String> {
        if !cargoframe_config::valid_id(id) {
            return Err("That is not a connection".into());
        }
        store
            .load_item::<Connection>(ConfigKind::Connections, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("The connection \"{id}\" no longer exists"))
    }

    /// Run `f` with a connector and the things it needs. `cancelled` ends waits (a sign-in, a
    /// retry pause) early.
    #[allow(clippy::too_many_arguments)]
    pub fn with_ctx<R>(
        &self,
        id: &str,
        connection: &Connection,
        browser: &dyn Browser,
        clipboard: &dyn Clipboard,
        progress: &Progress,
        timeout: Duration,
        cancelled: &dyn Fn() -> bool,
        f: impl FnOnce(&dyn Connector, &Ctx) -> Result<R, SendError>,
    ) -> Result<R, SendError> {
        let connector = conn::build(connection).ok_or_else(|| {
            cargoframe_safe::SendError::new(
                cargoframe_safe::FailureKind::ConnectorUnavailable,
                "This connection is turned off",
            )
        })?;
        let now = unix_now;
        let ctx = Ctx {
            connection_id: id,
            http: self.http.as_ref(),
            secrets: self.secrets.as_ref(),
            browser,
            clipboard,
            progress,
            timeout,
            now: &now,
            cancelled,
        };
        f(connector.as_ref(), &ctx)
    }

    /// Run `f` for something that needs neither browser nor clipboard.
    fn quietly<R>(
        &self,
        id: &str,
        connection: &Connection,
        f: impl FnOnce(&dyn Connector, &Ctx) -> Result<R, SendError>,
    ) -> Result<R, SendError> {
        let progress = Progress::new();
        self.with_ctx(
            id,
            connection,
            &NoBrowser,
            &NoClipboard,
            &progress,
            Duration::from_secs(30),
            &|| false,
            f,
        )
    }

    fn status_of(&self, id: &str, connection: &Connection) -> ConnectionState {
        if !connection.enabled {
            return ConnectionState::Disconnected;
        }
        if self.cache().expired.contains(id) {
            return ConnectionState::Expired;
        }
        if let Some(s) = self.cache().status.get(id).copied() {
            return from_status(s);
        }
        // Not holding the lock while the credential store answers: it can take a moment.
        let status = self
            .quietly(id, connection, |c, ctx| Ok(c.status(ctx)))
            .unwrap_or(Status::Disconnected);
        self.cache().status.insert(id.to_string(), status);
        from_status(status)
    }

    /// What the layout checks and the window need to know about a connection.
    pub fn info(&self, store: &Store, id: &str) -> Option<ConnectionInfo> {
        let connection = self.load(store, id).ok()?;
        Some(ConnectionInfo {
            kind: layout_kind(connection.settings.kind()),
            state: self.status_of(id, &connection),
        })
    }

    /// Forget what is known about a connection's sign-in and lists. Call after anything that
    /// changes a secret, the settings, or the sign-in.
    pub fn invalidate(&self, id: &str) {
        let mut c = self.cache();
        c.status.remove(id);
        c.expired.remove(id);
        c.options.retain(|(conn, _), _| conn != id);
    }

    /// The service rejected the sign-in during a share.
    pub fn mark_expired(&self, id: &str) {
        self.cache().expired.insert(id.to_string());
    }

    /// Probe the credential store. `None` when it works.
    pub fn store_problem(&self) -> Option<String> {
        match self.secrets.get("probe/none") {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        }
    }

    pub fn view(&self, store: &Store) -> ConnectionsView {
        let loaded = match store.load_all::<Connection>(ConfigKind::Connections) {
            Ok(l) => l,
            Err(e) => {
                return ConnectionsView {
                    items: vec![],
                    problems: vec![e.to_string()],
                    store_problem: self.store_problem(),
                }
            }
        };
        let items = loaded
            .items
            .iter()
            .map(|(id, c)| {
                let kind = c.settings.kind();
                let secrets = kind
                    .typed_secrets()
                    .iter()
                    .map(|name| SecretFlag {
                        name,
                        stored: matches!(
                            self.secrets.get(&conn::secrets::key(id, name)),
                            Ok(Some(_))
                        ),
                    })
                    .collect();
                let describe = conn::build(&Connection {
                    enabled: true,
                    ..c.clone()
                })
                .map(|b| b.describe())
                .unwrap_or_default();
                ConnectionView {
                    id: id.clone(),
                    name: c.name.clone(),
                    kind,
                    kind_label: kind.label(),
                    enabled: c.enabled,
                    state: state_name(self.status_of(id, c), c.enabled),
                    secrets,
                    describe,
                }
            })
            .collect();
        ConnectionsView {
            items,
            problems: loaded.errors.iter().map(ToString::to_string).collect(),
            store_problem: self.store_problem(),
        }
    }

    /// Store a secret the person typed. Only the names this kind of connection uses are
    /// accepted, and an empty value removes the secret.
    pub fn set_secret(
        &self,
        store: &Store,
        id: &str,
        name: &str,
        value: &str,
    ) -> Result<(), String> {
        let connection = self.load(store, id)?;
        let kind = connection.settings.kind();
        if !kind.typed_secrets().contains(&name) {
            return Err(format!("{} has no \"{name}\" to set", kind.label()));
        }
        let key = conn::secrets::key(id, name);
        let result = if value.is_empty() {
            self.secrets.delete(&key)
        } else {
            self.secrets.set(&key, value)
        };
        self.invalidate(id);
        result.map_err(|e| e.to_string())
    }

    /// Delete a connection's secrets (not the connection file).
    pub fn forget_secrets(&self, id: &str, kind: Kind) {
        conn::secrets::delete_all(self.secrets.as_ref(), id, kind.secret_names());
        self.invalidate(id);
    }

    pub fn begin_sign_in(&self) {
        self.sign_in_cancelled.store(false, Ordering::SeqCst);
    }

    pub fn cancel_sign_in(&self) {
        self.sign_in_cancelled.store(true, Ordering::SeqCst);
    }

    /// Sign in. Opens the browser for OAuth connections and waits for the person.
    pub fn connect(
        &self,
        store: &Store,
        id: &str,
        browser: &dyn Browser,
        clipboard: &dyn Clipboard,
    ) -> Result<String, String> {
        let connection = self.load(store, id)?;
        self.begin_sign_in();
        let progress = Progress::new();
        let cancelled = || self.sign_in_cancelled.load(Ordering::SeqCst);
        let result = self.with_ctx(
            id,
            &connection,
            browser,
            clipboard,
            &progress,
            Duration::from_secs(30),
            &cancelled,
            |c, ctx| c.connect(ctx),
        );
        self.invalidate(id);
        result.map_err(|e| e.message)
    }

    pub fn disconnect(&self, store: &Store, id: &str) -> Result<(), String> {
        let connection = self.load(store, id)?;
        let result = self.quietly(id, &connection, |c, ctx| {
            c.disconnect(ctx);
            Ok(())
        });
        self.invalidate(id);
        result.map_err(|e| e.message)
    }

    /// Ask the service who we are.
    pub fn test(&self, store: &Store, id: &str) -> Result<String, String> {
        let connection = self.load(store, id)?;
        let result = self.quietly(id, &connection, |c, ctx| c.test(ctx));
        match &result {
            Err(e) if e.kind.needs_reconnect() => self.mark_expired(id),
            Ok(_) => {
                self.cache().expired.remove(id);
                self.cache().status.remove(id);
            }
            _ => {}
        }
        result.map_err(|e| e.message)
    }

    /// A dropdown list from the service, reused for a short while.
    pub fn options(
        &self,
        store: &Store,
        id: &str,
        list: &str,
    ) -> Result<Vec<OptionItem>, SendError> {
        let key = (id.to_string(), list.to_string());
        if let Some((at, items)) = self.cache().options.get(&key) {
            if at.elapsed() < OPTIONS_TTL {
                return Ok(items.clone());
            }
        }
        let connection = self
            .load(store, id)
            .map_err(conn::error::invalid)?;
        let result = self.quietly(id, &connection, |c, ctx| c.options(ctx, list));
        match &result {
            Ok(items) => {
                self.cache()
                    .options
                    .insert(key, (Instant::now(), items.clone()));
            }
            Err(e) if e.kind.needs_reconnect() => self.mark_expired(id),
            _ => {}
        }
        result
    }

    /// What a connection can fill and which lists it offers.
    pub fn describe(&self, store: &Store, id: &str) -> Result<Describe, String> {
        let connection = self.load(store, id)?;
        conn::build(&Connection {
            enabled: true,
            ..connection
        })
        .map(|b| b.describe())
        .ok_or_else(|| "This connection cannot be described".to_string())
    }
}

fn from_status(s: Status) -> ConnectionState {
    match s {
        Status::Connected => ConnectionState::Connected,
        Status::Disconnected => ConnectionState::Disconnected,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cargoframe_config::Dirs;
    use cargoframe_connectors::MemorySecrets;
    use cargoframe_net::mock::{MockResponse, MockServer};
    use cargoframe_net::UreqHttp;
    use serde_json::json;

    pub struct Fixture {
        pub _dir: tempfile::TempDir,
        pub store: Store,
        pub connections: Connections,
        pub secrets: Arc<MemorySecrets>,
    }

    pub fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Dirs::under(dir.path()));
        let secrets = Arc::new(MemorySecrets::new());
        let connections = Connections::new(Arc::new(UreqHttp::new()), secrets.clone());
        Fixture {
            _dir: dir,
            store,
            connections,
            secrets,
        }
    }

    pub fn save(store: &Store, id: &str, value: serde_json::Value) {
        let c: Connection = serde_json::from_value(value).unwrap();
        store.save_item(ConfigKind::Connections, id, &c).unwrap();
    }

    fn custom(base: &str, auth: serde_json::Value) -> serde_json::Value {
        json!({
            "name": "Mine", "kind": "custom", "base_url": base, "auth": auth,
            "send": {"method": "POST", "path": "/hook", "body": {"type": "none"}}
        })
    }

    #[test]
    fn status_follows_secrets_and_is_cached_until_invalidated() {
        let f = fixture();
        save(
            &f.store,
            "jira",
            json!({"name": "Work", "kind": "jira", "site_url": "https://acme.atlassian.net",
                   "auth": "api_token", "email": "a@b.c", "project": "P"}),
        );
        let info = f.connections.info(&f.store, "jira").unwrap();
        assert_eq!(info.kind, ConnectionKind::Jira);
        assert_eq!(info.state, ConnectionState::Disconnected);

        f.connections
            .set_secret(&f.store, "jira", "token", "s3cret")
            .unwrap();
        // set_secret drops the cached answer, so the new state shows at once.
        assert_eq!(
            f.connections.info(&f.store, "jira").unwrap().state,
            ConnectionState::Connected
        );

        f.connections.mark_expired("jira");
        assert_eq!(
            f.connections.info(&f.store, "jira").unwrap().state,
            ConnectionState::Expired
        );
        f.connections.invalidate("jira");
        assert_eq!(
            f.connections.info(&f.store, "jira").unwrap().state,
            ConnectionState::Connected
        );
        assert!(f.connections.info(&f.store, "gone").is_none());
        assert!(f.connections.info(&f.store, "../x").is_none());
    }

    #[test]
    fn only_the_secrets_of_the_kind_can_be_set_and_empty_removes() {
        let f = fixture();
        save(&f.store, "claude", json!({"name": "Claude", "kind": "claude"}));
        let err = f
            .connections
            .set_secret(&f.store, "claude", "token", "x")
            .unwrap_err();
        assert!(err.contains("no \"token\""), "{err}");

        save(
            &f.store,
            "jira",
            json!({"name": "Work", "kind": "jira", "site_url": "https://acme.atlassian.net",
                   "auth": "api_token", "email": "a@b.c", "project": "P"}),
        );
        let err = f
            .connections
            .set_secret(&f.store, "jira", "oauth", "x")
            .unwrap_err();
        assert!(err.contains("no \"oauth\""), "{err}");
        f.connections.set_secret(&f.store, "jira", "token", "t").unwrap();
        assert_eq!(f.secrets.keys(), vec!["jira/token".to_string()]);
        f.connections.set_secret(&f.store, "jira", "token", "").unwrap();
        assert!(f.secrets.keys().is_empty());
    }

    #[test]
    fn the_view_lists_state_and_whether_secrets_are_stored_but_never_their_values() {
        let f = fixture();
        save(
            &f.store,
            "jira",
            json!({"name": "Work", "kind": "jira", "site_url": "https://acme.atlassian.net",
                   "auth": "api_token", "email": "a@b.c", "project": "P"}),
        );
        save(&f.store, "off", {
            let mut v = custom("https://example.com", json!({"type": "none"}));
            v["enabled"] = json!(false);
            v
        });
        f.connections.set_secret(&f.store, "jira", "token", "TOPSECRET").unwrap();
        let view = f.connections.view(&f.store);
        assert!(view.store_problem.is_none());
        let jira = view.items.iter().find(|c| c.id == "jira").unwrap();
        assert_eq!(jira.state, "connected");
        assert_eq!(jira.secrets.len(), 1);
        assert!(jira.secrets[0].stored);
        assert!(!jira.describe.fields.is_empty());
        let off = view.items.iter().find(|c| c.id == "off").unwrap();
        assert_eq!(off.state, "disabled");
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("TOPSECRET"));
    }

    #[test]
    fn options_are_fetched_once_and_reused_then_dropped_on_invalidate() {
        let f = fixture();
        let server = MockServer::start(|req| {
            if req.path() == "/projects" {
                MockResponse::json(200, json!([{"id": "1", "name": "One"}]))
            } else {
                MockResponse::json(404, json!({}))
            }
        });
        let mut v = custom(&server.base(), json!({"type": "none"}));
        v["lists"] = json!([{
            "id": "projects", "label": "Projects", "path": "/projects",
            "items_path": "", "value_path": "id", "label_path": "name"
        }]);
        save(&f.store, "api", v);
        let first = f.connections.options(&f.store, "api", "projects");
        assert!(first.is_ok(), "{first:?}");
        assert_eq!(first.unwrap()[0].label, "One");
        f.connections.options(&f.store, "api", "projects").unwrap();
        assert_eq!(server.requests().len(), 1, "the second call was cached");
        f.connections.invalidate("api");
        f.connections.options(&f.store, "api", "projects").unwrap();
        assert_eq!(server.requests().len(), 2);
    }
}
