use std::path::{Path, PathBuf};

const APP_DIR: &str = "CargoFrame";

/// Where CargoFrame keeps its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    config: PathBuf,
    data: PathBuf,
    /// The organisation's read-only settings, when there may be some (see [`Dirs::managed`]).
    managed: Option<PathBuf>,
}

impl Dirs {
    /// Standard locations. `CARGOFRAME_HOME` (a folder) overrides both, which is how tests and
    /// portable setups keep everything in one place: `<home>/config` and `<home>/data`.
    pub fn from_env() -> Option<Self> {
        // `CARGOFRAME_MANAGED` names the organisation's folder anywhere; with `CARGOFRAME_HOME`
        // (tests, portable copies) only it does, so nothing outside the home folder is read
        let named = std::env::var_os("CARGOFRAME_MANAGED")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        if let Some(home) = std::env::var_os("CARGOFRAME_HOME").filter(|v| !v.is_empty()) {
            let mut d = Self::under(Path::new(&home));
            d.managed = named;
            return Some(d);
        }
        Some(Self {
            config: dirs::config_dir()?.join(APP_DIR),
            data: dirs::data_local_dir()?.join(APP_DIR).join("data"),
            managed: named.or_else(system_managed),
        })
    }

    pub fn under(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            data: root.join("data"),
            managed: None,
        }
    }

    /// The same folders, with the organisation's settings read from `managed`.
    pub fn with_managed(mut self, managed: impl Into<PathBuf>) -> Self {
        self.managed = Some(managed.into());
        self
    }

    /// The organisation's settings: `%ProgramData%\CargoFrame\managed` on Windows,
    /// `/etc/cargoframe/managed` on Linux, or `CARGOFRAME_MANAGED`. Read-only: the same item
    /// folders as the person's own, plus `policy.json`. Written by an administrator (Intune, a
    /// login script, a package), never by CargoFrame.
    pub fn managed(&self) -> Option<&Path> {
        self.managed.as_deref()
    }

    /// Settings: `%AppData%\CargoFrame` or `~/.config/CargoFrame`.
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// Local data: `%LocalAppData%\CargoFrame\data` or `~/.local/share/CargoFrame/data`. The
    /// program itself lives one level up, so removing the data folder never touches it.
    pub fn data(&self) -> &Path {
        &self.data
    }

    /// Where zips are kept when the safe folder cannot be used.
    pub fn kept_zips(&self) -> PathBuf {
        self.data.join("kept")
    }

    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }
}

/// Where an organisation puts its settings on this system.
fn system_managed() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("ProgramData")
            .filter(|v| !v.is_empty())
            .map(|d| PathBuf::from(d).join(APP_DIR).join("managed"))
    } else {
        Some(PathBuf::from("/etc/cargoframe/managed"))
    }
}
