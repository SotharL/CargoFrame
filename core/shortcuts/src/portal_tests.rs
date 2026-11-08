//! A fake GlobalShortcuts portal on a private bus, answering as xdg-desktop-portal does:
//! every request answers with a `Response` signal on its request path.

use super::*;
use std::sync::OnceLock;
use zbus::blocking::connection::Builder;
use zbus::message::Header;

struct Bus(std::process::Child, String);

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn private_bus() -> Option<Bus> {
    use std::io::BufRead;
    let mut child = std::process::Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut address = String::new();
    std::io::BufReader::new(child.stdout.as_mut()?)
        .read_line(&mut address)
        .ok()?;
    Some(Bus(child, address.trim().to_string()))
}

#[derive(Clone, Default)]
struct Fake {
    conn: Arc<OnceLock<Connection>>,
    bound: Arc<Mutex<Vec<(String, String)>>>,
    closed: Arc<Mutex<Vec<String>>>,
    sessions: Arc<Mutex<Vec<String>>>,
}

fn sender_part(hdr: &Header<'_>) -> String {
    cargoframe_capture::portal::sender_path_part(hdr.sender().map(|s| s.as_str()).unwrap_or(""))
}

fn token(options: &HashMap<String, OwnedValue>, key: &str) -> String {
    options
        .get(key)
        .and_then(|v| <&str>::try_from(&**v).ok())
        .unwrap_or("")
        .to_string()
}

impl Fake {
    /// Answer a request a moment later, from the portal's own connection.
    fn respond(&self, path: String, results: HashMap<String, OwnedValue>) {
        let conn = self.conn.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let conn = conn.get().unwrap();
            conn.emit_signal(
                None::<&str>,
                path.as_str(),
                "org.freedesktop.portal.Request",
                "Response",
                &(0u32, results),
            )
            .unwrap();
        });
    }
}

struct Session(Fake, String);

#[zbus::interface(name = "org.freedesktop.portal.Session")]
impl Session {
    #[zbus(name = "Close")]
    fn close(&self) {
        self.0.closed.lock().unwrap().push(self.1.clone());
    }
}

#[zbus::interface(name = "org.freedesktop.portal.GlobalShortcuts")]
impl Fake {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    #[zbus(name = "CreateSession")]
    fn create_session(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        options: HashMap<String, OwnedValue>,
    ) -> OwnedObjectPath {
        let me = sender_part(&hdr);
        let request = format!(
            "/org/freedesktop/portal/desktop/request/{me}/{}",
            token(&options, "handle_token")
        );
        let session = format!(
            "/org/freedesktop/portal/desktop/session/{me}/{}",
            token(&options, "session_handle_token")
        );
        self.sessions.lock().unwrap().push(session.clone());
        let conn = self.conn.get().unwrap().clone();
        conn.object_server()
            .at(session.as_str(), Session(self.clone(), session.clone()))
            .unwrap();
        let mut results = HashMap::new();
        results.insert(
            "session_handle".to_string(),
            OwnedValue::try_from(Value::from(session.as_str())).unwrap(),
        );
        self.respond(request.clone(), results);
        OwnedObjectPath::try_from(request).unwrap()
    }

    #[zbus(name = "BindShortcuts")]
    fn bind_shortcuts(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        _session: OwnedObjectPath,
        shortcuts: Vec<(String, HashMap<String, OwnedValue>)>,
        _parent: String,
        options: HashMap<String, OwnedValue>,
    ) -> OwnedObjectPath {
        let me = sender_part(&hdr);
        let request = format!(
            "/org/freedesktop/portal/desktop/request/{me}/{}",
            token(&options, "handle_token")
        );
        let mut answer: Vec<(String, HashMap<String, OwnedValue>)> = Vec::new();
        for (id, props) in shortcuts {
            let wanted = token(&props, "preferred_trigger");
            self.bound
                .lock()
                .unwrap()
                .push((id.clone(), wanted.clone()));
            let mut p = HashMap::new();
            p.insert(
                "trigger_description".to_string(),
                OwnedValue::try_from(Value::from(format!("Press {wanted}"))).unwrap(),
            );
            answer.push((id, p));
        }
        let mut results = HashMap::new();
        results.insert(
            "shortcuts".to_string(),
            OwnedValue::try_from(Value::from(answer)).unwrap(),
        );
        self.respond(request.clone(), results);
        OwnedObjectPath::try_from(request).unwrap()
    }
}

fn press(fake: &Fake, session: &str, id: &str) {
    let conn = fake.conn.get().unwrap();
    conn.emit_signal(
        None::<&str>,
        OBJECT,
        INTERFACE,
        "Activated",
        &(
            OwnedObjectPath::try_from(session.to_string()).unwrap(),
            id,
            0u64,
            HashMap::<String, OwnedValue>::new(),
        ),
    )
    .unwrap();
}

#[test]
fn shortcuts_are_bound_through_the_portal_and_their_presses_arrive() {
    let Some(bus) = private_bus() else {
        eprintln!("skipped: no dbus-daemon");
        return;
    };
    let fake = Fake::default();
    let service = Builder::address(bus.1.as_str())
        .unwrap()
        .name(DESTINATION)
        .unwrap()
        .serve_at(OBJECT, fake.clone())
        .unwrap()
        .build()
        .unwrap();
    fake.conn.set(service).ok().unwrap();

    let (tx, rx) = mpsc::channel();
    let client = Builder::address(bus.1.as_str()).unwrap().build().unwrap();
    let portal = Portal::start_on(client, move |id| {
        let _ = tx.send(id);
    })
    .unwrap();

    let keys = portal
        .bind(&[Binding {
            id: "bug".into(),
            name: "Bug report".into(),
            shortcut: "Ctrl+Shift+F9".into(),
        }])
        .unwrap();
    assert_eq!(keys["bug"], "Press CTRL+SHIFT+F9");
    assert_eq!(
        *fake.bound.lock().unwrap(),
        vec![("bug".to_string(), "CTRL+SHIFT+F9".to_string())]
    );
    let first = fake.sessions.lock().unwrap()[0].clone();
    press(&fake, &first, "bug");
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "bug");

    // a change binds in a new session and closes the old one, whose presses no longer count
    portal
        .bind(&[Binding {
            id: "chat".into(),
            name: "Chat".into(),
            shortcut: "Alt+F10".into(),
        }])
        .unwrap();
    assert_eq!(*fake.closed.lock().unwrap(), vec![first.clone()]);
    let second = fake.sessions.lock().unwrap()[1].clone();
    press(&fake, &first, "bug");
    press(&fake, &second, "chat");
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "chat");
    assert!(rx.try_recv().is_err());

    // nothing to bind closes the session and asks nothing
    assert!(portal.bind(&[]).unwrap().is_empty());
    assert_eq!(fake.closed.lock().unwrap().len(), 2);
    drop(bus);
}
