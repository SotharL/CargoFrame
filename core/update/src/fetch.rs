use crate::checksum::{parse_sums, HashingWriter};
use crate::error::UpdateError;
use crate::release::{Available, Source};
use cargoframe_net::{Http, NetError};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Largest installer accepted. Today's are a fraction of this; it only stops a runaway answer.
const MAX_INSTALLER_BYTES: u64 = 400 * 1024 * 1024;

/// How far a download has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: Option<u64>,
}

/// An installer on disk whose fingerprint matches the one the release lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

/// Download the installer of `available` into `dir`, and keep it only when its SHA-256 is the one
/// in the release's `SHA256SUMS.txt`. The file is written under a temporary name and renamed once
/// it checks out, so a half download or a wrong file is never left looking like an installer.
pub fn fetch(
    http: &dyn Http,
    source: &Source,
    available: &Available,
    dir: &Path,
    progress: &mut dyn FnMut(Progress),
) -> Result<Fetched, UpdateError> {
    for asset in [&available.installer, &available.sums] {
        if !source.owns_asset_url(&asset.url) {
            return Err(UpdateError::UntrustedAddress(asset.url.clone()));
        }
    }

    // The list of fingerprints first: with no fingerprint there is nothing to download for.
    let listed = http.send(&source.request(&available.sums.url, "application/octet-stream"))?;
    if !listed.is_success() {
        return Err(UpdateError::DownloadRefused {
            status: listed.status,
            message: short(&listed.text()),
        });
    }
    let sums = parse_sums(&listed.text());
    let expected = sums
        .of(&available.installer.name)
        .ok_or_else(|| UpdateError::NotListed(available.installer.name.clone()))?
        .to_string();

    fs::create_dir_all(dir).map_err(|e| UpdateError::Disk(e.to_string()))?;
    let final_path = dir.join(&available.installer.name);
    let part_path = dir.join(format!("{}.part", available.installer.name));
    let file = File::create(&part_path).map_err(|e| UpdateError::Disk(e.to_string()))?;
    let mut writer = HashingWriter::new(BufWriter::new(file));

    let request = source
        .request(&available.installer.url, "application/octet-stream")
        .timeout(Duration::from_secs(15 * 60));
    let result = http.download(
        &request,
        &mut writer,
        MAX_INSTALLER_BYTES,
        &mut |done, total| progress(Progress { done, total }),
    );
    let outcome = match result {
        Ok(done) if done.is_success() => Ok(done.bytes),
        Ok(done) => Err(UpdateError::DownloadRefused {
            status: done.status,
            message: short(&done.message),
        }),
        Err(e) => Err(map_net(e)),
    };
    let bytes = match outcome {
        Ok(b) => b,
        Err(e) => {
            let _ = fs::remove_file(&part_path);
            return Err(e);
        }
    };
    let (_, actual) = match writer.finish() {
        Ok(v) => v,
        Err(e) => {
            let _ = fs::remove_file(&part_path);
            return Err(e);
        }
    };
    if actual != expected {
        let _ = fs::remove_file(&part_path);
        return Err(UpdateError::ChecksumMismatch);
    }
    let _ = fs::remove_file(&final_path);
    fs::rename(&part_path, &final_path).map_err(|e| {
        let _ = fs::remove_file(&part_path);
        UpdateError::Disk(e.to_string())
    })?;
    Ok(Fetched {
        path: final_path,
        sha256: actual,
        bytes,
    })
}

fn map_net(e: NetError) -> UpdateError {
    match e {
        NetError::Other(m) if m.starts_with("cannot write the download") => UpdateError::Disk(m),
        other => UpdateError::Net(other),
    }
}

/// A server's explanation, cut to one line.
fn short(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > 160 {
        let cut: String = line.chars().take(160).collect();
        format!("{cut}…")
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests;
