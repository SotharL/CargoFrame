//! Getting ffmpeg from inside CargoFrame, on Windows and Linux alike: the static build of its
//! latest release branch from BtbN's FFmpeg-Builds on GitHub (the builds ffmpeg.org points to),
//! checked against the release's own list of SHA-256 fingerprints, unpacked into CargoFrame's
//! data folder (`tools/ffmpeg`), and found there first from then on. The same button updates it
//! when a newer release branch is out, whatever installed the ffmpeg in use. Nothing needs an
//! administrator, and nothing outside that folder is touched.

use cargoframe_i18n::{t, tf};
use cargoframe_net::{Http, Request};
use cargoframe_update::{Asset, Available, Progress, Source};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const REPOSITORY: &str = "BtbN/FFmpeg-Builds";
const SUMS: &str = "checksums.sha256";
/// What is kept about the copy CargoFrame unpacked.
const INSTALLED: &str = "installed.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Option<Os> {
        if !cfg!(target_arch = "x86_64") {
            None
        } else if cfg!(windows) {
            Some(Os::Windows)
        } else if cfg!(target_os = "linux") {
            Some(Os::Linux)
        } else {
            None
        }
    }

    fn platform(self) -> &'static str {
        match self {
            Os::Windows => "win64",
            Os::Linux => "linux64",
        }
    }

    fn archive(self) -> &'static str {
        match self {
            Os::Windows => ".zip",
            Os::Linux => ".tar.xz",
        }
    }

    fn exe(self, name: &str) -> String {
        match self {
            Os::Windows => format!("{name}.exe"),
            Os::Linux => name.to_string(),
        }
    }
}

/// A build to download: its archive, the fingerprints, and its release branch ("9.0").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub version: String,
    pub archive: Asset,
    pub sums: Asset,
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
    #[serde(default)]
    assets: Vec<RawAsset>,
}

/// `9.0` of `ffmpeg-n9.0-latest-win64-gpl-9.0.zip`, as numbers to compare.
fn branch(name: &str, os: Os) -> Option<(u32, u32)> {
    let rest = name.strip_prefix("ffmpeg-n")?;
    let (version, tail) = rest.split_once("-latest-")?;
    let wanted = format!("{}-gpl-{version}{}", os.platform(), os.archive());
    if tail != wanted {
        return None;
    }
    let (major, minor) = version.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// The newest release branch's static build for `os` in a release listing (the "latest"
/// release of FFmpeg-Builds), else the build of ffmpeg's development branch.
pub fn pick(body: &[u8], os: Os) -> Result<Offer, String> {
    let raw: RawRelease = serde_json::from_slice(body)
        .map_err(|_| t("GitHub's answer about ffmpeg could not be read.").to_string())?;
    let asset = |a: &RawAsset| Asset {
        name: a.name.clone(),
        size: a.size,
        url: a.url.clone(),
    };
    let sums = raw
        .assets
        .iter()
        .find(|a| a.name == SUMS)
        .map(asset)
        .ok_or_else(|| {
            t("The ffmpeg builds have no list of fingerprints, so none is downloaded.").to_string()
        })?;
    let best = raw
        .assets
        .iter()
        .filter_map(|a| branch(&a.name, os).map(|v| (v, a)))
        .max_by_key(|(v, _)| *v);
    if let Some(((major, minor), a)) = best {
        return Ok(Offer {
            version: format!("{major}.{minor}"),
            archive: asset(a),
            sums,
        });
    }
    let master = format!("ffmpeg-master-latest-{}-gpl{}", os.platform(), os.archive());
    raw.assets
        .iter()
        .find(|a| a.name == master)
        .map(|a| Offer {
            version: "master".into(),
            archive: asset(a),
            sums,
        })
        .ok_or_else(|| t("No ffmpeg build for this computer was found.").to_string())
}

fn source() -> Source {
    Source::github(REPOSITORY, None)
}

/// Ask GitHub for the build to download.
pub fn latest(http: &dyn Http, os: Os) -> Result<Offer, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/tags/latest");
    let response = http
        .send(
            &Request::get(&url)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28"),
        )
        .map_err(|e| tf("GitHub could not be reached: {e}", &[("e", &e)]))?;
    if !response.is_success() {
        return Err(tf(
            "GitHub said {status} about the ffmpeg builds.",
            &[("status", &response.status)],
        ));
    }
    pick(&response.body, os)
}

/// Whether ffmpeg at `version` (as `ffmpeg -version` names it: `7.1.1-full_build…`, `n9.0-…`)
/// is older than the release branch `offered` ("9.0"). An unknown version is not older.
pub fn older(version: &str, offered: &str) -> bool {
    let nums = |v: &str| -> Option<(u32, u32)> {
        let v = v.trim_start_matches(['n', 'N']);
        let mut it = v.split(|c: char| !c.is_ascii_digit());
        Some((
            it.next()?.parse().ok()?,
            it.next().unwrap_or("0").parse().unwrap_or(0),
        ))
    };
    match (nums(version), nums(offered)) {
        (Some(have), Some(new)) => have < new,
        _ => false,
    }
}

/// Written next to the copy CargoFrame unpacked, to say where it came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Installed {
    /// The release branch, "9.0".
    pub version: String,
    pub archive: String,
}

/// Download the build, check its fingerprint, and put ffmpeg and ffprobe in `dir`.
pub fn get(
    http: &dyn Http,
    os: Os,
    offer: &Offer,
    dir: &Path,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), String> {
    let work = dir.join("download");
    let available = Available {
        version: offer.version.clone(),
        tag: "latest".into(),
        notes: String::new(),
        published: None,
        page: String::new(),
        installer: offer.archive.clone(),
        sums: offer.sums.clone(),
    };
    let fetched = cargoframe_update::fetch(http, &source(), &available, &work, progress)
        .map_err(|e| e.to_string())?;
    let unpacked = unpack(os, &fetched.path, &work);
    let _ = std::fs::remove_file(&fetched.path);
    let found = unpacked?;
    for (name, from) in found {
        let to = dir.join(&name);
        let part = dir.join(format!("{name}.part"));
        std::fs::copy(&from, &part).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::remove_file(&to);
        std::fs::rename(&part, &to).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_dir_all(&work);
    let kept = Installed {
        version: offer.version.clone(),
        archive: offer.archive.name.clone(),
    };
    std::fs::write(
        dir.join(INSTALLED),
        serde_json::to_vec_pretty(&kept).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())
}

/// ffmpeg and ffprobe out of the archive, by name and where they were unpacked.
fn unpack(os: Os, archive: &Path, work: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let wanted = [os.exe("ffmpeg"), os.exe("ffprobe")];
    let out = work.join("unpacked");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    match os {
        Os::Windows => {
            let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().map_err(|e| e.to_string())?.replace('\\', "/");
                // only bin/ffmpeg.exe and bin/ffprobe.exe, by their own name: nothing in the
                // archive chooses where a file goes
                let Some(file) = wanted.iter().find(|w| name.ends_with(&format!("/bin/{w}")))
                else {
                    continue;
                };
                let mut to = std::fs::File::create(out.join(file)).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut to).map_err(|e| e.to_string())?;
            }
        }
        Os::Linux => {
            let status = std::process::Command::new("tar")
                .arg("-xJf")
                .arg(archive)
                .arg("-C")
                .arg(&out)
                .arg("--no-same-owner")
                .args(["--wildcards", "--no-anchored", "bin/ffmpeg", "bin/ffprobe"])
                .stdin(std::process::Stdio::null())
                .status()
                .map_err(|e| tf("tar could not be started: {e}", &[("e", &e)]))?;
            if !status.success() {
                return Err(t(
                    "The ffmpeg download could not be unpacked (tar and xz are needed).",
                )
                .into());
            }
        }
    }
    let mut found = Vec::new();
    for name in &wanted {
        let path = find_file(&out, name)
            .ok_or_else(|| tf("{name} is not in the download.", &[("name", name)]))?;
        found.push((name.clone(), path));
    }
    Ok(found)
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(f) = find_file(&p, name) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|f| f == name) {
            return Some(p);
        }
    }
    None
}

/// Where the download is, for the System page to draw.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    /// A build is there to download: newer than the ffmpeg in use, or there is none.
    Offered {
        version: String,
        size: u64,
    },
    /// The ffmpeg in use is as new as the builds.
    Current {
        version: String,
    },
    Downloading {
        done: u64,
        total: Option<u64>,
    },
    Unpacking,
    Done {
        version: String,
    },
    Failed {
        message: String,
    },
}

/// Getting ffmpeg, one at a time.
pub struct FfmpegGet {
    http: Arc<dyn Http>,
    dir: PathBuf,
    phase: Mutex<Phase>,
    offer: Mutex<Option<Offer>>,
}

impl FfmpegGet {
    pub fn new(http: Arc<dyn Http>, dir: PathBuf) -> Self {
        Self {
            http,
            dir,
            phase: Mutex::new(Phase::Idle),
            offer: Mutex::new(None),
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set(&self, p: Phase) {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = p;
    }

    fn busy(&self) -> bool {
        matches!(
            self.phase(),
            Phase::Checking | Phase::Downloading { .. } | Phase::Unpacking
        )
    }

    /// Ask for the newest build and compare it with the ffmpeg in use (`have`, its version, or
    /// `None` when there is none).
    pub fn check(&self, have: Option<&str>) -> Phase {
        if self.busy() {
            return self.phase();
        }
        let Some(os) = Os::current() else {
            self.set(Phase::Failed {
                message: t("CargoFrame cannot download ffmpeg for this computer.").into(),
            });
            return self.phase();
        };
        self.set(Phase::Checking);
        let next = match latest(&*self.http, os) {
            Ok(offer) => {
                let phase = match have {
                    Some(v) if !older(v, &offer.version) => Phase::Current {
                        version: v.to_string(),
                    },
                    _ => Phase::Offered {
                        version: offer.version.clone(),
                        size: offer.archive.size,
                    },
                };
                *self.offer.lock().unwrap_or_else(|e| e.into_inner()) = Some(offer);
                phase
            }
            Err(message) => Phase::Failed { message },
        };
        self.set(next);
        self.phase()
    }

    /// Download what the last check offered, on its own thread; `changed` is called as it goes.
    pub fn begin(
        self: &Arc<Self>,
        changed: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), String> {
        if self.busy() {
            return Err(t("ffmpeg is already being downloaded").into());
        }
        let Some(os) = Os::current() else {
            return Err(t("CargoFrame cannot download ffmpeg for this computer.").into());
        };
        let Some(offer) = self.offer.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Err(t("Look for ffmpeg first").into());
        };
        self.set(Phase::Downloading {
            done: 0,
            total: Some(offer.archive.size).filter(|s| *s > 0),
        });
        changed();
        let me = self.clone();
        std::thread::Builder::new()
            .name("ffmpeg-download".into())
            .spawn(move || {
                let mut told = Instant::now();
                let result = get(&*me.http, os, &offer, &me.dir, &mut |p: Progress| {
                    if p.total == Some(p.done) {
                        me.set(Phase::Unpacking);
                        changed();
                    } else if told.elapsed() >= Duration::from_millis(250) {
                        told = Instant::now();
                        me.set(Phase::Downloading {
                            done: p.done,
                            total: p.total,
                        });
                        changed();
                    }
                });
                me.set(match result {
                    Ok(()) => Phase::Done {
                        version: offer.version.clone(),
                    },
                    Err(message) => Phase::Failed { message },
                });
                changed();
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn listing(names: &[&str]) -> Vec<u8> {
        let assets: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                json!({
                    "name": n, "size": 100 + i,
                    "url": format!("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/assets/{i}"),
                })
            })
            .collect();
        serde_json::to_vec(&json!({ "tag_name": "latest", "assets": assets })).unwrap()
    }

    #[test]
    fn the_newest_release_branch_of_this_computer_is_picked() {
        let body = listing(&[
            "checksums.sha256",
            "ffmpeg-master-latest-win64-gpl.zip",
            "ffmpeg-n8.1-latest-win64-gpl-8.1.zip",
            "ffmpeg-n9.0-latest-win64-gpl-9.0.zip",
            "ffmpeg-n9.0-latest-win64-gpl-shared-9.0.zip",
            "ffmpeg-n9.0-latest-win64-lgpl-9.0.zip",
            "ffmpeg-n9.0-latest-linux64-gpl-9.0.tar.xz",
        ]);
        let w = pick(&body, Os::Windows).unwrap();
        assert_eq!(
            (w.version.as_str(), w.archive.name.as_str()),
            ("9.0", "ffmpeg-n9.0-latest-win64-gpl-9.0.zip")
        );
        assert_eq!(w.sums.name, "checksums.sha256");
        let l = pick(&body, Os::Linux).unwrap();
        assert_eq!(l.archive.name, "ffmpeg-n9.0-latest-linux64-gpl-9.0.tar.xz");
        // no release branch: the development one; no fingerprints: nothing
        let only_master = listing(&[
            "checksums.sha256",
            "ffmpeg-master-latest-linux64-gpl.tar.xz",
        ]);
        assert_eq!(pick(&only_master, Os::Linux).unwrap().version, "master");
        assert!(pick(
            &listing(&["ffmpeg-n9.0-latest-win64-gpl-9.0.zip"]),
            Os::Windows
        )
        .is_err());
        assert!(pick(b"nope", Os::Windows).is_err());
    }

    #[test]
    fn a_version_is_older_than_a_newer_release_branch() {
        assert!(older("7.1.1-full_build-www.gyan.dev", "9.0"));
        assert!(older("n8.1-12-gabc", "9.0"));
        assert!(!older("n9.0-3-gdef", "9.0"));
        assert!(!older("9.1", "9.0"));
        // unknown on either side: nothing is offered as newer
        assert!(!older("N-123073-g743df5ded9", "9.0"));
        assert!(!older("7.1", "master"));
    }
}
