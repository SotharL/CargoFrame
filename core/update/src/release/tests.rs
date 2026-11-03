use super::*;
use cargoframe_net::mock::{MockResponse, MockServer};
use cargoframe_net::UreqHttp;
use serde_json::json;

const ALL: [&str; 3] = [
    "CargoFrame-0.2.0-windows-x64-setup.exe",
    "CargoFrame-0.2.0-linux-x64.tar.gz",
    "SHA256SUMS.txt",
];

/// A release as GitHub shows it, with asset addresses on the host that is asked (`host`).
fn release_json(host: &str, tag: &str, assets: &[&str]) -> serde_json::Value {
    let assets: Vec<_> = assets
        .iter()
        .enumerate()
        .map(|(i, name)| {
            json!({
                "id": i + 1,
                "name": name,
                "size": 1234,
                "url": format!("http://{host}/repos/me/CargoFrame/releases/assets/{}", i + 1),
                "browser_download_url": format!("https://github.com/me/CargoFrame/releases/download/{tag}/{name}"),
            })
        })
        .collect();
    json!({
        "tag_name": tag,
        "name": tag,
        "body": "  What changed\n- one\n",
        "draft": false,
        "prerelease": false,
        "published_at": "2026-11-03T10:00:00Z",
        "html_url": format!("https://github.com/me/CargoFrame/releases/tag/{tag}"),
        "assets": assets,
    })
}

fn serving(tag: &'static str, assets: &'static [&'static str]) -> MockServer {
    MockServer::start(move |rec| {
        let host = rec.header("host").unwrap_or("127.0.0.1").to_string();
        MockResponse::json(200, release_json(&host, tag, assets))
    })
}

fn source(server: &MockServer, token: Option<&str>) -> Source {
    Source {
        api_base: server.url(""),
        repository: "me/CargoFrame".into(),
        token: token.map(str::to_string),
    }
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn run(
    server: &MockServer,
    current: &str,
    platform: Option<Platform>,
) -> Result<Check, UpdateError> {
    check(
        &UreqHttp::new(),
        &source(server, None),
        &v(current),
        platform,
    )
}

#[test]
fn a_newer_release_is_offered_with_its_installer_and_checksums() {
    let server = serving("v0.2.0", &ALL);
    let Check::Available(a) = run(&server, "0.1.0", Some(Platform::Windows)).unwrap() else {
        panic!("expected an update");
    };
    assert_eq!(a.version, "0.2.0");
    assert_eq!(a.tag, "v0.2.0");
    assert_eq!(a.installer.name, "CargoFrame-0.2.0-windows-x64-setup.exe");
    assert_eq!(a.sums.name, "SHA256SUMS.txt");
    assert_eq!(a.notes, "What changed\n- one");
    assert_eq!(a.published.as_deref(), Some("2026-11-03T10:00:00Z"));
    assert!(a.page.ends_with("/tag/v0.2.0"));
    let Check::Available(l) = run(&server, "0.1.0", Some(Platform::Linux)).unwrap() else {
        panic!("expected an update");
    };
    assert_eq!(l.installer.name, "CargoFrame-0.2.0-linux-x64.tar.gz");
}

#[test]
fn the_request_asks_the_right_address_with_the_headers_github_wants() {
    let server = serving("v0.2.0", &ALL);
    check(
        &UreqHttp::new(),
        &source(&server, Some("ghp_secret")),
        &v("0.1.0"),
        Some(Platform::Linux),
    )
    .unwrap();
    let seen = &server.requests()[0];
    assert_eq!(seen.method, "GET");
    assert_eq!(seen.path(), "/repos/me/CargoFrame/releases/latest");
    assert_eq!(seen.header("accept"), Some("application/vnd.github+json"));
    assert_eq!(seen.header("x-github-api-version"), Some("2022-11-28"));
    assert_eq!(seen.header("authorization"), Some("Bearer ghp_secret"));
}

#[test]
fn no_token_means_no_authorization_header() {
    let server = serving("v0.2.0", &ALL);
    run(&server, "0.1.0", Some(Platform::Linux)).unwrap();
    assert_eq!(server.requests()[0].header("authorization"), None);
}

#[test]
fn the_same_or_an_older_release_is_up_to_date() {
    let server = serving("v0.2.0", &ALL);
    for current in ["0.2.0", "0.3.0", "1.0.0"] {
        let c = run(&server, current, Some(Platform::Windows)).unwrap();
        assert!(matches!(c, Check::UpToDate { .. }), "{current}");
    }
    let Check::UpToDate { current, latest } = run(&server, "0.2.0", None).unwrap() else {
        panic!()
    };
    assert_eq!((current.as_str(), latest.as_str()), ("0.2.0", "0.2.0"));
}

#[test]
fn a_prerelease_of_the_running_version_is_older_than_the_release() {
    let server = serving("v0.2.0", &ALL);
    assert!(matches!(
        run(&server, "0.2.0-beta.1", Some(Platform::Linux)).unwrap(),
        Check::Available(_)
    ));
}

#[test]
fn a_computer_without_installers_hears_which_release_it_is() {
    let server = serving("v0.2.0", &ALL);
    let err = run(&server, "0.1.0", None).unwrap_err();
    assert_eq!(
        err,
        UpdateError::NoInstaller {
            tag: "v0.2.0".into()
        }
    );
}

#[test]
fn a_release_without_this_computers_installer_is_refused() {
    let server = serving(
        "v0.2.0",
        &["CargoFrame-0.2.0-linux-x64.tar.gz", "SHA256SUMS.txt"],
    );
    let err = run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err();
    assert!(matches!(err, UpdateError::NoInstaller { .. }), "{err:?}");
}

#[test]
fn an_installer_of_another_version_does_not_count() {
    let server = serving(
        "v0.2.0",
        &["CargoFrame-0.1.9-windows-x64-setup.exe", "SHA256SUMS.txt"],
    );
    let err = run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err();
    assert!(matches!(err, UpdateError::NoInstaller { .. }), "{err:?}");
}

#[test]
fn a_release_without_checksums_is_refused() {
    let server = serving("v0.2.0", &["CargoFrame-0.2.0-windows-x64-setup.exe"]);
    let err = run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err();
    assert_eq!(
        err,
        UpdateError::NoChecksums {
            tag: "v0.2.0".into()
        }
    );
}

#[test]
fn an_asset_address_on_another_site_is_not_trusted() {
    let server = MockServer::start(|_| {
        let mut j = release_json("127.0.0.1:1", "v0.2.0", &ALL);
        for a in j["assets"].as_array_mut().unwrap() {
            a["url"] = json!("https://evil.example.com/steal?token");
        }
        MockResponse::json(200, j)
    });
    let err = run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err();
    assert!(matches!(err, UpdateError::UntrustedAddress(_)), "{err:?}");
}

#[test]
fn an_asset_address_for_another_repository_is_not_trusted() {
    let server = MockServer::start(|rec| {
        let host = rec.header("host").unwrap_or("").to_string();
        let mut j = release_json(&host, "v0.2.0", &ALL);
        for a in j["assets"].as_array_mut().unwrap() {
            a["url"] = json!(format!(
                "http://{host}/repos/someone/else/releases/assets/9"
            ));
        }
        MockResponse::json(200, j)
    });
    let err = run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err();
    assert!(matches!(err, UpdateError::UntrustedAddress(_)), "{err:?}");
}

#[test]
fn a_file_name_that_names_a_folder_is_never_picked() {
    let server = MockServer::start(|rec| {
        let host = rec.header("host").unwrap_or("").to_string();
        let mut j = release_json(&host, "v0.2.0", &ALL);
        j["assets"][0]["name"] = json!("../CargoFrame-0.2.0-windows-x64-setup.exe");
        MockResponse::json(200, j)
    });
    let err = run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err();
    assert!(matches!(err, UpdateError::NoInstaller { .. }), "{err:?}");
}

#[test]
fn drafts_and_prereleases_are_never_offered() {
    for field in ["draft", "prerelease"] {
        let server = MockServer::start(move |rec| {
            let host = rec.header("host").unwrap_or("").to_string();
            let mut j = release_json(&host, "v9.0.0", &ALL);
            j[field] = json!(true);
            MockResponse::json(200, j)
        });
        let c = run(&server, "0.1.0", Some(Platform::Windows)).unwrap();
        assert!(matches!(c, Check::UpToDate { .. }), "{field}");
    }
}

#[test]
fn each_refusal_from_github_says_what_to_do() {
    let case = |status: u16, headers: &'static [(&'static str, &'static str)]| {
        let server = MockServer::start(move |_| {
            let mut r = MockResponse::text(status, "no");
            for (k, v) in headers {
                r = r.header(k, v);
            }
            r
        });
        run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err()
    };
    assert_eq!(case(404, &[]), UpdateError::NotFound);
    assert_eq!(case(401, &[]), UpdateError::TokenRejected);
    assert_eq!(case(403, &[]), UpdateError::NoAccess);
    assert_eq!(
        case(403, &[("X-RateLimit-Remaining", "0")]),
        UpdateError::RateLimited
    );
    assert_eq!(case(429, &[]), UpdateError::RateLimited);
    assert_eq!(case(500, &[]), UpdateError::BadAnswer);
    assert!(UpdateError::NotFound.needs_token());
    assert!(UpdateError::TokenRejected.needs_token());
    assert!(!UpdateError::RateLimited.needs_token());
}

#[test]
fn an_answer_that_is_not_a_release_is_bad() {
    let server = MockServer::start(|_| MockResponse::text(200, "<html>captive portal</html>"));
    assert_eq!(
        run(&server, "0.1.0", Some(Platform::Windows)).unwrap_err(),
        UpdateError::BadAnswer
    );
    let odd_tag = MockServer::start(|rec| {
        let host = rec.header("host").unwrap_or("").to_string();
        MockResponse::json(200, release_json(&host, "nightly", &ALL))
    });
    assert_eq!(
        run(&odd_tag, "0.1.0", Some(Platform::Windows)).unwrap_err(),
        UpdateError::BadAnswer
    );
}

#[test]
fn nothing_answering_is_an_offline_error() {
    let source = Source {
        api_base: "http://127.0.0.1:1".into(),
        repository: "me/CargoFrame".into(),
        token: None,
    };
    let err = check(
        &UreqHttp::new(),
        &source,
        &v("0.1.0"),
        Some(Platform::Windows),
    )
    .unwrap_err();
    assert!(matches!(err, UpdateError::Net(_)), "{err:?}");
}

#[test]
fn versions_read_with_or_without_a_v() {
    assert_eq!(parse_version("v1.2.3"), Some(v("1.2.3")));
    assert_eq!(parse_version("1.2.3"), Some(v("1.2.3")));
    assert_eq!(parse_version(" V1.2.3-rc.1 "), Some(v("1.2.3-rc.1")));
    assert_eq!(parse_version("latest"), None);
    assert_eq!(parse_version("1.2"), None);
}

#[test]
fn a_blank_token_is_no_token() {
    assert_eq!(Source::github("a/b", Some("  ".into())).token, None);
    assert_eq!(
        Source::github("a/b", Some("t".into())).token.as_deref(),
        Some("t")
    );
}

#[test]
fn only_plain_file_names_pass() {
    for ok in ["CargoFrame-0.2.0-linux-x64.tar.gz", "SHA256SUMS.txt"] {
        assert!(is_plain_name(ok), "{ok}");
    }
    for bad in ["", ".", "..", "a/b", "a\\b", "C:evil", ".hidden", "a\0b"] {
        assert!(!is_plain_name(bad), "{bad:?}");
    }
}
