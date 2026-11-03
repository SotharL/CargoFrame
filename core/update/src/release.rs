use crate::error::UpdateError;
use cargoframe_net::{Http, Request};
use semver::Version;
use serde::{Deserialize, Serialize};

/// Where releases are read from. `api_base` is `https://api.github.com` for the real thing; a
/// test points it at a local server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub api_base: String,
    /// `owner/name`, for example `SotharL/CargoFrame`.
    pub repository: String,
    /// A read-only token, needed only while the repository is private.
    pub token: Option<String>,
}

impl Source {
    pub fn github(repository: &str, token: Option<String>) -> Self {
        Self {
            api_base: "https://api.github.com".into(),
            repository: repository.to_string(),
            token: token.filter(|t| !t.trim().is_empty()),
        }
    }

    pub(crate) fn latest_url(&self) -> String {
        format!(
            "{}/repos/{}/releases/latest",
            self.api_base.trim_end_matches('/'),
            self.repository
        )
    }

    /// Asset addresses must be on the same API, under this repository: a token is sent to them,
    /// so an address from an answer is not trusted until it is checked.
    pub(crate) fn owns_asset_url(&self, url: &str) -> bool {
        let prefix = format!(
            "{}/repos/{}/releases/assets/",
            self.api_base.trim_end_matches('/'),
            self.repository
        );
        url.starts_with(&prefix) && !url[prefix.len()..].is_empty() && !url.contains("..")
    }

    /// A request to the API with the headers GitHub asks for, and the token if there is one.
    pub(crate) fn request(&self, url: &str, accept: &str) -> Request {
        let mut r = Request::get(url)
            .header("Accept", accept)
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(t) = &self.token {
            r = r.header("Authorization", format!("Bearer {t}"));
        }
        r
    }
}

/// The kinds of computer an installer is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Windows,
    Linux,
}

impl Platform {
    /// The platform this program was built for, when installers exist for it (64-bit only).
    pub fn current() -> Option<Self> {
        if !cfg!(target_pointer_width = "64") || !cfg!(target_arch = "x86_64") {
            return None;
        }
        if cfg!(windows) {
            Some(Platform::Windows)
        } else if cfg!(target_os = "linux") {
            Some(Platform::Linux)
        } else {
            None
        }
    }

    /// The end of the installer's name in a release (`CargoFrame-<version>-<this>`).
    pub fn installer_suffix(self) -> &'static str {
        match self {
            Platform::Windows => "-windows-x64-setup.exe",
            Platform::Linux => "-linux-x64.tar.gz",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    /// The API address that serves the file (with `Accept: application/octet-stream`).
    pub url: String,
}

/// A release newer than the running one, with what is needed to install it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Available {
    pub version: String,
    pub tag: String,
    pub notes: String,
    pub published: Option<String>,
    /// The release's page on GitHub, for people who would rather read it there.
    pub page: String,
    pub installer: Asset,
    pub sums: Asset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Check {
    UpToDate { current: String, latest: String },
    Available(Available),
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    #[serde(default)]
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

const SUMS_NAME: &str = "SHA256SUMS.txt";

/// `v0.2.0` and `0.2.0` both read as 0.2.0.
fn parse_version(tag: &str) -> Option<Version> {
    Version::parse(tag.trim().trim_start_matches(['v', 'V'])).ok()
}

/// A plain file name: no folders, so it cannot name a place outside the download folder.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.starts_with('.')
}

/// Ask the repository for its newest release and compare it with `current`.
pub fn check(
    http: &dyn Http,
    source: &Source,
    current: &Version,
    platform: Option<Platform>,
) -> Result<Check, UpdateError> {
    let response =
        http.send(&source.request(&source.latest_url(), "application/vnd.github+json"))?;
    match response.status {
        200..=299 => {}
        401 => return Err(UpdateError::TokenRejected),
        403 | 429 => {
            let out_of_calls = response.header("x-ratelimit-remaining") == Some("0")
                || response.header("retry-after").is_some()
                || response.status == 429;
            return Err(if out_of_calls {
                UpdateError::RateLimited
            } else {
                UpdateError::NoAccess
            });
        }
        404 => return Err(UpdateError::NotFound),
        _ => return Err(UpdateError::BadAnswer),
    }
    let raw: RawRelease =
        serde_json::from_slice(&response.body).map_err(|_| UpdateError::BadAnswer)?;
    if raw.draft || raw.prerelease {
        // `latest` never returns these; a mock or a proxy might. Treat as "nothing newer".
        return Ok(Check::UpToDate {
            current: current.to_string(),
            latest: current.to_string(),
        });
    }
    let latest = parse_version(&raw.tag_name).ok_or(UpdateError::BadAnswer)?;
    if latest <= *current {
        return Ok(Check::UpToDate {
            current: current.to_string(),
            latest: latest.to_string(),
        });
    }

    let platform = platform.ok_or_else(|| UpdateError::NoInstaller {
        tag: raw.tag_name.clone(),
    })?;
    let wanted = format!("CargoFrame-{latest}{}", platform.installer_suffix());
    let pick = |name: &str| -> Option<Asset> {
        raw.assets
            .iter()
            .find(|a| a.name == name && is_plain_name(&a.name))
            .map(|a| Asset {
                name: a.name.clone(),
                size: a.size,
                url: a.url.clone(),
            })
    };
    let installer = pick(&wanted).ok_or_else(|| UpdateError::NoInstaller {
        tag: raw.tag_name.clone(),
    })?;
    let sums = pick(SUMS_NAME).ok_or_else(|| UpdateError::NoChecksums {
        tag: raw.tag_name.clone(),
    })?;
    for asset in [&installer, &sums] {
        if !source.owns_asset_url(&asset.url) {
            return Err(UpdateError::UntrustedAddress(asset.url.clone()));
        }
    }
    Ok(Check::Available(Available {
        version: latest.to_string(),
        tag: raw.tag_name,
        notes: raw.body.unwrap_or_default().trim().to_string(),
        published: raw.published_at,
        page: raw.html_url,
        installer,
        sums,
    }))
}

#[cfg(test)]
mod tests;
