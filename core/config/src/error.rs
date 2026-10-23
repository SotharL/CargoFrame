use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid id {0:?}: use lowercase letters, digits, '-' and '_' (1 to 64 characters)")]
    InvalidId(String),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}: not valid JSON ({message})", path.display())]
    Parse { path: PathBuf, message: String },
    #[error(
        "{} was written by a newer CargoFrame (schema {found}; this version understands up to {supported})",
        path.display()
    )]
    Newer {
        path: PathBuf,
        found: u32,
        supported: u32,
    },
    #[error("{0}")]
    Invalid(String),
}

impl ConfigError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
