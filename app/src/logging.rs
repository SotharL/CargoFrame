//! A small log file in the local data folder, kept under 1 MiB (one rotated copy).

use cargoframe_config::Dirs;
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::sync::Mutex;
use tracing_subscriber::EnvFilter;

const MAX_BYTES: u64 = 1024 * 1024;

pub fn init(dirs: &Dirs) {
    let filter =
        EnvFilter::try_from_env("CARGOFRAME_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let to_stderr = cfg!(debug_assertions) || std::env::var_os("CARGOFRAME_LOG_STDERR").is_some();

    let file = open_log(&dirs.logs().join("cargoframe.log"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false);
    match (file, to_stderr) {
        (Some(f), false) => builder.with_writer(Mutex::new(f)).init(),
        _ => builder.with_writer(std::io::stderr).init(),
    }

    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));
}

fn open_log(path: &Path) -> Option<fs::File> {
    fs::create_dir_all(path.parent()?).ok()?;
    if fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false)
    {
        let _ = fs::rename(path, path.with_extension("log.old"));
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_log_is_rotated_once() {
        let dir = tempdir();
        let log = dir.join("cargoframe.log");
        fs::write(&log, vec![b'x'; (MAX_BYTES + 1) as usize]).unwrap();
        open_log(&log).unwrap();
        assert!(dir.join("cargoframe.log.old").exists());
        assert_eq!(fs::metadata(&log).unwrap().len(), 0);
    }

    #[test]
    fn a_small_log_is_appended_to() {
        let dir = tempdir();
        let log = dir.join("cargoframe.log");
        fs::write(&log, "hello").unwrap();
        open_log(&log).unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap(), "hello");
        assert!(!dir.join("cargoframe.log.old").exists());
    }

    fn tempdir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "cf-log-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
}
