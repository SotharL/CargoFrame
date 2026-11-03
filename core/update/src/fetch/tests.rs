use super::*;
use crate::release::{Asset, Platform};
use cargoframe_net::mock::{MockResponse, MockServer};
use cargoframe_net::UreqHttp;
use sha2::{Digest, Sha256};

const NAME: &str = "CargoFrame-0.2.0-linux-x64.tar.gz";

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

struct Fixture {
    server: MockServer,
    source: Source,
    available: Available,
}

/// A server that has the installer (`payload`) as asset 1 and a sums file saying `listed` as 2.
fn fixture(
    payload: Vec<u8>,
    sums_text: impl Fn(&str) -> String + Send + Sync + 'static,
) -> Fixture {
    let served = payload.clone();
    let server = MockServer::start(move |rec| match rec.path() {
        "/repos/me/CargoFrame/releases/assets/1" => MockResponse::ok().body(served.clone()),
        "/repos/me/CargoFrame/releases/assets/2" => MockResponse::text(200, &sums_text(NAME)),
        _ => MockResponse::text(404, "Not Found"),
    });
    let source = Source {
        api_base: server.url(""),
        repository: "me/CargoFrame".into(),
        token: Some("ghp_secret".into()),
    };
    let available = Available {
        version: "0.2.0".into(),
        tag: "v0.2.0".into(),
        notes: String::new(),
        published: None,
        page: String::new(),
        installer: Asset {
            name: NAME.into(),
            size: payload.len() as u64,
            url: server.url("/repos/me/CargoFrame/releases/assets/1"),
        },
        sums: Asset {
            name: "SHA256SUMS.txt".into(),
            size: 100,
            url: server.url("/repos/me/CargoFrame/releases/assets/2"),
        },
    };
    Fixture {
        server,
        source,
        available,
    }
}

fn payload() -> Vec<u8> {
    (0..300_000u32).map(|i| (i % 253) as u8).collect()
}

#[test]
fn a_download_that_matches_its_fingerprint_is_kept() {
    let p = payload();
    let want = hex(&p);
    let f = fixture(p.clone(), move |name| format!("{want}  {name}\n"));
    let dir = tempfile::tempdir().unwrap();
    let mut last = None;
    let got = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |p| last = Some(p),
    )
    .unwrap();
    assert_eq!(got.path, dir.path().join(NAME));
    assert_eq!(got.bytes, p.len() as u64);
    assert_eq!(got.sha256, hex(&p));
    assert_eq!(std::fs::read(&got.path).unwrap(), p);
    assert!(!dir.path().join(format!("{NAME}.part")).exists());
    let last = last.expect("progress was reported");
    assert_eq!(last.done, p.len() as u64);
    assert_eq!(last.total, Some(p.len() as u64));
}

#[test]
fn the_token_goes_to_the_api_and_the_files_are_asked_for_as_bytes() {
    let p = payload();
    let want = hex(&p);
    let f = fixture(p, move |name| format!("{want}  {name}\n"));
    let dir = tempfile::tempdir().unwrap();
    fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |_| {},
    )
    .unwrap();
    let seen = f.server.requests();
    assert_eq!(seen.len(), 2);
    for r in &seen {
        assert_eq!(r.header("authorization"), Some("Bearer ghp_secret"));
        assert_eq!(r.header("accept"), Some("application/octet-stream"));
    }
}

#[test]
fn a_file_that_does_not_match_is_deleted_and_not_installable() {
    let f = fixture(payload(), |name| {
        format!("{}  {name}\n", hex(b"something else"))
    });
    let dir = tempfile::tempdir().unwrap();
    let err = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(err, UpdateError::ChecksumMismatch);
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "nothing is left behind"
    );
}

#[test]
fn a_release_that_does_not_list_the_installer_is_not_downloaded_at_all() {
    let f = fixture(payload(), |_| {
        format!("{}  some-other-file.exe\n", hex(b"x"))
    });
    let dir = tempfile::tempdir().unwrap();
    let err = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(err, UpdateError::NotListed(NAME.into()));
    assert_eq!(
        f.server.requests().len(),
        1,
        "only the list of fingerprints was fetched"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn a_refused_download_says_why_and_leaves_nothing() {
    let p = payload();
    let want = hex(&p);
    let mut f = fixture(p, move |name| format!("{want}  {name}\n"));
    f.available.installer.url = f.server.url("/repos/me/CargoFrame/releases/assets/99");
    let dir = tempfile::tempdir().unwrap();
    let err = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(
        err,
        UpdateError::DownloadRefused {
            status: 404,
            message: "Not Found".into()
        }
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn an_asset_address_that_is_not_on_the_api_is_refused_before_anything_is_sent() {
    let p = payload();
    let want = hex(&p);
    let mut f = fixture(p, move |name| format!("{want}  {name}\n"));
    f.available.installer.url = "https://evil.example.com/x".into();
    let dir = tempfile::tempdir().unwrap();
    let err = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, UpdateError::UntrustedAddress(_)));
    assert!(f.server.requests().is_empty());
}

#[test]
fn an_older_download_of_the_same_name_is_replaced() {
    let p = payload();
    let want = hex(&p);
    let f = fixture(p.clone(), move |name| format!("{want}  {name}\n"));
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(NAME), b"old").unwrap();
    let got = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        dir.path(),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(std::fs::read(got.path).unwrap(), p);
}

#[test]
fn a_folder_that_cannot_be_created_is_a_disk_error() {
    let p = payload();
    let want = hex(&p);
    let f = fixture(p, move |name| format!("{want}  {name}\n"));
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("file");
    std::fs::write(&blocker, b"x").unwrap();
    let err = fetch(
        &UreqHttp::new(),
        &f.source,
        &f.available,
        &blocker.join("sub"),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, UpdateError::Disk(_)), "{err:?}");
}

#[test]
fn the_platform_suffixes_match_what_the_release_workflow_names() {
    assert_eq!(
        Platform::Windows.installer_suffix(),
        "-windows-x64-setup.exe"
    );
    assert_eq!(Platform::Linux.installer_suffix(), "-linux-x64.tar.gz");
}

#[test]
fn a_long_server_message_is_cut_to_one_line() {
    assert_eq!(short("one\ntwo"), "one");
    let long = "x".repeat(300);
    assert_eq!(short(&long).chars().count(), 161);
}
