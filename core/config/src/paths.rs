use std::path::{Path, PathBuf};

const APP_DIR: &str = "CargoFrame";

/// Where CargoFrame keeps its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    config: PathBuf,
    data: PathBuf,
}

impl Dirs {
    /// Standard locations. `CARGOFRAME_HOME` (a folder) overrides both, which is how tests and
    /// portable setups keep everything in one place: `<home>/config` and `<home>/data`.
    pub fn from_env() -> Option<Self> {
        if let Some(home) = std::env::var_os("CARGOFRAME_HOME").filter(|v| !v.is_empty()) {
            return Some(Self::under(Path::new(&home)));
        }
        Some(Self {
            config: dirs::config_dir()?.join(APP_DIR),
            data: dirs::data_local_dir()?.join(APP_DIR),
        })
    }

    pub fn under(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            data: root.join("data"),
        }
    }

    /// Settings: `%AppData%\CargoFrame` or `~/.config/CargoFrame`.
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// Local data: `%LocalAppData%\CargoFrame` or `~/.local/share/CargoFrame`.
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
