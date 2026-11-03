//! Updating CargoFrame from its GitHub releases.
//!
//! 1. [`check`] asks the repository for its newest release and compares it with the running
//!    version. It never installs anything.
//! 2. [`fetch`] downloads the installer for this computer and the release's `SHA256SUMS.txt`,
//!    and keeps the installer only if its fingerprint is the one listed. No fingerprint, no
//!    install.
//! 3. [`install`] hands the verified installer to the platform's own installer, which quits the
//!    running copy, swaps the files, keeps the settings and starts CargoFrame again.
//!
//! While the repository is private the requests carry a read-only token the owner created; the
//! token only ever goes to the GitHub API address the check was started with, never to the
//! storage address a download is redirected to.

mod checksum;
mod error;
mod fetch;
mod install;
mod release;

pub use checksum::{parse_sums, Sums};
pub use error::UpdateError;
pub use fetch::{fetch, Fetched, Progress};
pub use install::{install, plan, InstallPlan};
pub use release::{check, Asset, Available, Check, Platform, Source};
pub use semver::Version;
