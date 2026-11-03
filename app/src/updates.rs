//! Updating CargoFrame from its GitHub releases: what the Settings, General, Updates section
//! shows and does. The network and the installer sit behind small seams, so the whole flow
//! (check, download, verify, hand over to the installer) is tested without a network, a real
//! installer or Tauri.
//!
//! While the repository is private the requests carry a read-only token the owner created. It is
//! kept in the credential store, never read back to a window, and only ever sent to the GitHub
//! API address.

use cargoframe_connectors::SecretStore;
use cargoframe_net::Http;
use cargoframe_update::{
    self as up, Available, Check, Platform, Progress, Source, UpdateError, Version,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where releases are published.
pub const REPOSITORY: &str = "SotharL/CargoFrame";
const TOKEN_KEY: &str = "updates/github-token";
/// After handing over to the installer, how long to wait for it to quit this program before
/// saying it did not take over.
const HANDOVER_WAIT: Duration = Duration::from_secs(90);

type Installer = dyn Fn(Platform, &Path, &Path) -> Result<(), UpdateError> + Send + Sync;

/// Where an update is, for the window to draw.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading {
        done: u64,
        total: Option<u64>,
    },
    Verifying,
    Installing,
    Failed {
        message: String,
        /// The verified installer, kept so the person can run it by hand.
        installer: Option<String>,
        /// The Updates section should ask for a token.
        needs_token: bool,
    },
}

/// The outcome of the last check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Last {
    UpToDate {
        latest: String,
    },
    Available {
        version: String,
        notes: String,
        page: String,
        installer: String,
        size: u64,
        published: Option<String>,
    },
    Failed {
        message: String,
        needs_token: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateView {
    pub current: String,
    pub repository: String,
    /// There is an installer for this kind of computer.
    pub supported: bool,
    pub token_set: bool,
    /// The credential store could not be read, so a token cannot be kept or used.
    pub token_problem: Option<String>,
    pub last: Option<Last>,
    pub phase: Phase,
}

#[derive(Default)]
struct State {
    last: Option<Last>,
    phase: Phase,
    available: Option<Available>,
    /// The newest version a notification was already shown for.
    notified: Option<String>,
}

pub struct Updates {
    http: Arc<dyn Http>,
    secrets: Arc<dyn SecretStore>,
    api_base: String,
    repository: String,
    current: Version,
    platform: Option<Platform>,
    install: Arc<Installer>,
    handover_wait: Duration,
    state: Mutex<State>,
}

impl Updates {
    pub fn new(http: Arc<dyn Http>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            http,
            secrets,
            api_base: "https://api.github.com".into(),
            repository: REPOSITORY.into(),
            current: Version::parse(env!("CARGO_PKG_VERSION"))
                .unwrap_or_else(|_| Version::new(0, 0, 0)),
            platform: Platform::current(),
            install: Arc::new(up::install),
            handover_wait: HANDOVER_WAIT,
            state: Mutex::new(State::default()),
        }
    }

    #[cfg(test)]
    fn for_tests(
        http: Arc<dyn Http>,
        secrets: Arc<dyn SecretStore>,
        api_base: &str,
        current: &str,
        install: Arc<Installer>,
    ) -> Self {
        let mut u = Self::new(http, secrets);
        u.api_base = api_base.trim_end_matches('/').to_string();
        u.repository = "me/CargoFrame".into();
        u.current = Version::parse(current).unwrap();
        u.platform = Some(Platform::Windows);
        u.install = install;
        u.handover_wait = Duration::from_millis(50);
        u
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The token, if one is stored; or why the store could not be read.
    fn token(&self) -> (Option<String>, Option<String>) {
        match self.secrets.get(TOKEN_KEY) {
            Ok(t) => (t.filter(|t| !t.trim().is_empty()), None),
            Err(e) => (None, Some(e.to_string())),
        }
    }

    fn source(&self) -> Source {
        Source {
            api_base: self.api_base.clone(),
            repository: self.repository.clone(),
            token: self.token().0,
        }
    }

    pub fn view(&self) -> UpdateView {
        let (token, problem) = self.token();
        let s = self.lock();
        UpdateView {
            current: self.current.to_string(),
            repository: self.repository.clone(),
            supported: self.platform.is_some(),
            token_set: token.is_some(),
            token_problem: problem,
            last: s.last.clone(),
            phase: s.phase.clone(),
        }
    }

    /// Store a token the person pasted; an empty one removes it. Never read back.
    pub fn set_token(&self, value: &str) -> Result<(), String> {
        let value = value.trim();
        if value.is_empty() {
            self.secrets.delete(TOKEN_KEY).map_err(|e| e.to_string())
        } else {
            self.secrets
                .set(TOKEN_KEY, value)
                .map_err(|e| e.to_string())
        }
    }

    fn busy(&self) -> bool {
        matches!(
            self.lock().phase,
            Phase::Checking | Phase::Downloading { .. } | Phase::Verifying | Phase::Installing
        )
    }

    /// Ask GitHub for the newest release. Does nothing while a download or install is going.
    pub fn check(&self) -> UpdateView {
        if self.busy() {
            return self.view();
        }
        self.lock().phase = Phase::Checking;
        let source = self.source();
        let result = up::check(&*self.http, &source, &self.current, self.platform);
        {
            let mut s = self.lock();
            s.phase = Phase::Idle;
            match result {
                Ok(Check::UpToDate { latest, .. }) => {
                    s.available = None;
                    s.last = Some(Last::UpToDate { latest });
                }
                Ok(Check::Available(a)) => {
                    s.last = Some(Last::Available {
                        version: a.version.clone(),
                        notes: a.notes.clone(),
                        page: a.page.clone(),
                        installer: a.installer.name.clone(),
                        size: a.installer.size,
                        published: a.published.clone(),
                    });
                    s.available = Some(a);
                }
                Err(e) => {
                    s.available = None;
                    s.last = Some(Last::Failed {
                        needs_token: e.needs_token(),
                        message: e.to_string(),
                    });
                }
            }
        }
        self.view()
    }

    /// A version that is newer and that no notification was shown for yet; marks it as shown.
    pub fn news(&self) -> Option<String> {
        let mut s = self.lock();
        let version = s.available.as_ref()?.version.clone();
        if s.notified.as_deref() == Some(version.as_str()) {
            return None;
        }
        s.notified = Some(version.clone());
        Some(version)
    }

    /// Start downloading and installing the release the last check found. Returns at once; the
    /// work runs on its own thread and `changed` is called whenever the view changed.
    pub fn begin_install(
        self: &Arc<Self>,
        dir: PathBuf,
        changed: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), String> {
        if self.platform.is_none() {
            return Err(UpdateError::Unsupported.to_string());
        }
        if self.busy() {
            return Err("An update is already in progress".into());
        }
        let Some(available) = self.lock().available.clone() else {
            return Err("Check for updates first".into());
        };
        self.lock().phase = Phase::Downloading {
            done: 0,
            total: Some(available.installer.size).filter(|s| *s > 0),
        };
        changed();
        let me = self.clone();
        std::thread::Builder::new()
            .name("update".into())
            .spawn(move || me.run_install(&available, &dir, &changed))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// The whole update, on the calling thread: download, verify, hand over to the installer.
    pub fn run_install(&self, available: &Available, dir: &Path, changed: &dyn Fn()) {
        let source = self.source();
        let mut last_told = Instant::now();
        let mut told_bytes = 0u64;
        let fetched = up::fetch(&*self.http, &source, available, dir, &mut |p: Progress| {
            let finished = p.total == Some(p.done);
            if finished
                || p.done - told_bytes >= 1024 * 1024
                || last_told.elapsed() >= Duration::from_millis(250)
            {
                told_bytes = p.done;
                last_told = Instant::now();
                self.lock().phase = Phase::Downloading {
                    done: p.done,
                    total: p.total,
                };
                changed();
            }
        });
        let fetched = match fetched {
            Ok(f) => f,
            Err(e) => return self.fail(&e, None, changed),
        };
        self.lock().phase = Phase::Verifying;
        changed();
        let Some(platform) = self.platform else {
            return self.fail(&UpdateError::Unsupported, None, changed);
        };
        self.lock().phase = Phase::Installing;
        changed();
        let shown = fetched.path.to_string_lossy().into_owned();
        if let Err(e) = (self.install)(platform, &fetched.path, dir) {
            return self.fail(&e, Some(shown), changed);
        }
        // The installer quits this program. If it has not within a while, say so.
        let still = self.lock().phase.clone();
        if still == Phase::Installing {
            std::thread::sleep(self.handover_wait);
            let mut s = self.lock();
            if s.phase == Phase::Installing {
                s.phase = Phase::Failed {
                    message:
                        "The installer did not take over. Run the downloaded installer yourself"
                            .into(),
                    installer: Some(shown),
                    needs_token: false,
                };
                drop(s);
                changed();
            }
        }
    }

    fn fail(&self, e: &UpdateError, installer: Option<String>, changed: &dyn Fn()) {
        self.lock().phase = Phase::Failed {
            message: e.to_string(),
            installer,
            needs_token: e.needs_token(),
        };
        changed();
    }
}

/// Downloads from earlier runs are of no use once the program has started: either that update
/// was installed, or it is older than what the next check finds.
pub fn clear_downloads(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(test)]
mod tests;
