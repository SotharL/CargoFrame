use super::*;
use cargoframe_connectors::MemorySecrets;
use cargoframe_net::mock::{MockResponse, MockServer};
use cargoframe_net::UreqHttp;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};

const NAME: &str = "CargoFrame-0.2.0-windows-x64-setup.exe";

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn payload() -> Vec<u8> {
    (0..120_000u32).map(|i| (i % 249) as u8).collect()
}

/// A repository with release `tag`, whose installer is `payload` and whose list of fingerprints
/// says `sums`.
fn repository(tag: &'static str, payload: Vec<u8>, sums: String) -> MockServer {
    MockServer::start(move |rec| {
        let host = rec.header("host").unwrap_or("127.0.0.1").to_string();
        match rec.path() {
            "/repos/me/CargoFrame/releases/latest" => MockResponse::json(
                200,
                json!({
                    "tag_name": tag,
                    "body": "Notes",
                    "draft": false,
                    "prerelease": false,
                    "published_at": "2026-11-03T10:00:00Z",
                    "html_url": format!("https://github.com/me/CargoFrame/releases/tag/{tag}"),
                    "assets": [
                        {"name": NAME, "size": payload.len(), "url": format!("http://{host}/repos/me/CargoFrame/releases/assets/1")},
                        {"name": "SHA256SUMS.txt", "size": 90, "url": format!("http://{host}/repos/me/CargoFrame/releases/assets/2")},
                    ],
                }),
            ),
            "/repos/me/CargoFrame/releases/assets/1" => MockResponse::ok().body(payload.clone()),
            "/repos/me/CargoFrame/releases/assets/2" => MockResponse::text(200, &sums),
            _ => MockResponse::text(404, "Not Found"),
        }
    })
}

struct Calls(Mutex<Vec<(Platform, PathBuf, PathBuf)>>);

fn updates(
    server: &MockServer,
    current: &str,
    fail_install: Option<UpdateError>,
) -> (Arc<Updates>, Arc<MemorySecrets>, Arc<Calls>) {
    let secrets = Arc::new(MemorySecrets::default());
    let calls = Arc::new(Calls(Mutex::new(Vec::new())));
    let seen = calls.clone();
    let install: Arc<Installer> = Arc::new(move |p, f, w| {
        seen.0
            .lock()
            .unwrap()
            .push((p, f.to_path_buf(), w.to_path_buf()));
        fail_install.clone().map_or(Ok(()), Err)
    });
    let u = Updates::for_tests(
        Arc::new(UreqHttp::new()),
        secrets.clone(),
        &server.url(""),
        current,
        install,
    );
    (Arc::new(u), secrets, calls)
}

fn good(tag: &'static str) -> (MockServer, Vec<u8>) {
    let p = payload();
    let sums = format!("{}  {NAME}\n", hex(&p));
    (repository(tag, p.clone(), sums), p)
}

#[test]
fn before_anything_the_view_is_calm() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    let v = u.view();
    assert_eq!(v.current, "0.1.0");
    assert!(v.supported);
    assert!(!v.token_set);
    assert_eq!(v.last, None);
    assert_eq!(v.phase, Phase::Idle);
    assert!(
        server.requests().is_empty(),
        "looking at the view asks nobody"
    );
}

#[test]
fn a_newer_release_is_found_and_described() {
    let (server, p) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    let v = u.check();
    assert_eq!(
        v.last,
        Some(Last::Available {
            version: "0.2.0".into(),
            notes: "Notes".into(),
            page: "https://github.com/me/CargoFrame/releases/tag/v0.2.0".into(),
            installer: NAME.into(),
            size: p.len() as u64,
            published: Some("2026-11-03T10:00:00Z".into()),
        })
    );
    assert_eq!(v.phase, Phase::Idle);
}

#[test]
fn the_latest_release_is_reported_as_up_to_date() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.2.0", None);
    assert_eq!(
        u.check().last,
        Some(Last::UpToDate {
            latest: "0.2.0".into()
        })
    );
    assert_eq!(u.news(), None);
}

#[test]
fn a_missing_release_asks_for_a_token() {
    let server = MockServer::start(|_| MockResponse::text(404, "Not Found"));
    let (u, _, _) = updates(&server, "0.1.0", None);
    let Some(Last::Failed {
        message,
        needs_token,
    }) = u.check().last
    else {
        panic!("expected a failure");
    };
    assert!(needs_token);
    assert!(message.contains("read-only token"), "{message}");
}

#[test]
fn the_token_is_kept_in_the_credential_store_and_sent_as_a_bearer_token() {
    let (server, _) = good("v0.2.0");
    let (u, secrets, _) = updates(&server, "0.1.0", None);
    u.set_token("  ghp_abc  ").unwrap();
    assert_eq!(secrets.get(TOKEN_KEY).unwrap().as_deref(), Some("ghp_abc"));
    assert!(u.view().token_set);
    u.check();
    assert_eq!(
        server.requests()[0].header("authorization"),
        Some("Bearer ghp_abc")
    );
    u.set_token("   ").unwrap();
    assert_eq!(secrets.get(TOKEN_KEY).unwrap(), None);
    assert!(!u.view().token_set);
}

#[test]
fn the_view_never_carries_the_token() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    u.set_token("ghp_supersecret").unwrap();
    u.check();
    let json = serde_json::to_string(&u.view()).unwrap();
    assert!(!json.contains("supersecret"), "{json}");
}

#[test]
fn news_is_told_once_per_version() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    assert_eq!(u.news(), None, "nothing found yet");
    u.check();
    assert_eq!(u.news().as_deref(), Some("0.2.0"));
    assert_eq!(u.news(), None, "already told");
    u.check();
    assert_eq!(u.news(), None, "still the same version");
}

#[test]
fn installing_downloads_checks_and_hands_over_the_verified_installer() {
    let (server, p) = good("v0.2.0");
    let (u, _, calls) = updates(&server, "0.1.0", None);
    u.check();
    let dir = tempfile::tempdir().unwrap();
    let changes = Arc::new(AtomicUsize::new(0));
    let counted = changes.clone();
    let available = u.lock().available.clone().unwrap();
    u.run_install(&available, dir.path(), &move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    let made = calls.0.lock().unwrap();
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].0, Platform::Windows);
    assert_eq!(made[0].1, dir.path().join(NAME));
    assert_eq!(std::fs::read(&made[0].1).unwrap(), p);
    assert!(
        changes.load(Ordering::SeqCst) >= 3,
        "the window is told as it goes"
    );
    // The installer never quit this program (it is a fake), so after the wait it says so.
    let Phase::Failed {
        message, installer, ..
    } = u.view().phase
    else {
        panic!("expected the hand-over warning, got {:?}", u.view().phase);
    };
    assert!(message.contains("did not take over"), "{message}");
    assert_eq!(
        installer,
        Some(dir.path().join(NAME).to_string_lossy().into_owned())
    );
}

#[test]
fn a_download_that_does_not_match_is_never_handed_to_the_installer() {
    let p = payload();
    let server = repository("v0.2.0", p, format!("{}  {NAME}\n", hex(b"another file")));
    let (u, _, calls) = updates(&server, "0.1.0", None);
    u.check();
    let dir = tempfile::tempdir().unwrap();
    let available = u.lock().available.clone().unwrap();
    u.run_install(&available, dir.path(), &|| {});
    assert!(calls.0.lock().unwrap().is_empty());
    let Phase::Failed {
        message,
        installer,
        needs_token,
    } = u.view().phase
    else {
        panic!("expected a failure");
    };
    assert!(message.contains("fingerprint differs"), "{message}");
    assert_eq!(installer, None);
    assert!(!needs_token);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn an_installer_that_cannot_start_leaves_the_file_for_the_person() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(
        &server,
        "0.1.0",
        Some(UpdateError::Install("access denied".into())),
    );
    u.check();
    let dir = tempfile::tempdir().unwrap();
    let available = u.lock().available.clone().unwrap();
    u.run_install(&available, dir.path(), &|| {});
    let Phase::Failed {
        message, installer, ..
    } = u.view().phase
    else {
        panic!("expected a failure");
    };
    assert!(message.contains("access denied"), "{message}");
    assert!(installer.is_some());
}

#[test]
fn installing_needs_a_check_first_and_refuses_to_run_twice() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        u.begin_install(dir.path().to_path_buf(), || {})
            .unwrap_err(),
        "Check for updates first"
    );
    u.check();
    u.lock().phase = Phase::Downloading {
        done: 1,
        total: None,
    };
    assert_eq!(
        u.begin_install(dir.path().to_path_buf(), || {})
            .unwrap_err(),
        "An update is already in progress"
    );
}

#[test]
fn checking_again_while_installing_changes_nothing() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    u.check();
    u.lock().phase = Phase::Installing;
    let before = server.requests().len();
    assert_eq!(u.check().phase, Phase::Installing);
    assert_eq!(server.requests().len(), before);
}

#[test]
fn begin_install_runs_on_its_own_thread_and_tells_the_window() {
    let (server, _) = good("v0.2.0");
    let (u, _, calls) = updates(&server, "0.1.0", None);
    u.check();
    let dir = tempfile::tempdir().unwrap();
    let changes = Arc::new(AtomicUsize::new(0));
    let counted = changes.clone();
    u.begin_install(dir.path().to_path_buf(), move || {
        counted.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
    for _ in 0..100 {
        if !calls.0.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(calls.0.lock().unwrap().len(), 1);
    assert!(changes.load(Ordering::SeqCst) >= 2);
}

#[test]
fn checking_again_after_a_failure_starts_clean() {
    let (server, _) = good("v0.2.0");
    let (u, _, _) = updates(&server, "0.1.0", None);
    u.lock().phase = Phase::Failed {
        message: "x".into(),
        installer: None,
        needs_token: false,
    };
    assert_eq!(u.check().phase, Phase::Idle);
}

#[test]
fn old_downloads_are_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("updates");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("old-setup.exe"), b"x").unwrap();
    clear_downloads(&sub);
    assert!(!sub.exists());
    clear_downloads(&sub); // nothing there is not an error
}
