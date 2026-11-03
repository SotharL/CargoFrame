use crate::error::UpdateError;
use crate::release::Platform;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A program to start, described so tests can check it without starting anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// What starts the installer. Windows runs the setup program silently as an update and asks it
/// to start CargoFrame again when it is done (`/S /UPDATE /R`): it quits the running copy, swaps
/// the files, and keeps settings and the choice to run at sign-in.
pub fn plan(
    platform: Platform,
    installer: &Path,
    install_script: Option<&Path>,
) -> Option<InstallPlan> {
    match platform {
        Platform::Windows => Some(InstallPlan {
            program: installer.to_path_buf(),
            args: vec!["/S".into(), "/UPDATE".into(), "/R".into()],
        }),
        // `setsid --fork` puts the script in a session of its own, so it keeps running when it
        // quits the copy that started it.
        Platform::Linux => install_script.map(|script| InstallPlan {
            program: PathBuf::from("setsid"),
            args: vec![
                "--fork".into(),
                "bash".into(),
                script.to_string_lossy().into_owned(),
                "--restart".into(),
            ],
        }),
    }
}

/// Hand a verified installer to the platform. On success the installer is running on its own and
/// will quit this program; the caller only reports that and waits.
///
/// `work` is a folder this may fill (the Linux package is unpacked in it).
pub fn install(platform: Platform, installer: &Path, work: &Path) -> Result<(), UpdateError> {
    if Platform::current() != Some(platform) {
        return Err(UpdateError::Unsupported);
    }
    let script = match platform {
        Platform::Windows => None,
        Platform::Linux => Some(unpack(installer, work)?),
    };
    let p = plan(platform, installer, script.as_deref()).ok_or(UpdateError::Unsupported)?;
    start_detached(&p)
}

/// Unpack the Linux package and find the `install.sh` that travels in it.
fn unpack(package: &Path, work: &Path) -> Result<PathBuf, UpdateError> {
    let out = work.join("package");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).map_err(|e| UpdateError::Install(e.to_string()))?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(package)
        .arg("-C")
        .arg(&out)
        .arg("--no-same-owner")
        .stdin(Stdio::null())
        .status()
        .map_err(|e| UpdateError::Install(format!("cannot run tar: {e}")))?;
    if !status.success() {
        return Err(UpdateError::Install(
            "the package could not be unpacked".into(),
        ));
    }
    find_script(&out).ok_or_else(|| UpdateError::Install("the package has no install.sh".into()))
}

/// `install.sh` directly inside `dir` or inside one folder of it (the package's own folder).
fn find_script(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("install.sh");
    if direct.is_file() {
        return Some(direct);
    }
    let mut folders: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();
    folders
        .into_iter()
        .map(|f| f.join("install.sh"))
        .find(|s| s.is_file())
}

#[cfg(windows)]
fn start_detached(plan: &InstallPlan) -> Result<(), UpdateError> {
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: the installer outlives this program.
    const FLAGS: u32 = 0x0000_0008 | 0x0000_0200;
    Command::new(&plan.program)
        .args(&plan.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(FLAGS)
        .spawn()
        .map(|_| ())
        .map_err(|e| UpdateError::Install(e.to_string()))
}

#[cfg(not(windows))]
fn start_detached(plan: &InstallPlan) -> Result<(), UpdateError> {
    let run = |program: &Path, args: &[String]| {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    };
    match run(&plan.program, &plan.args) {
        Ok(mut child) => {
            // `setsid --fork` returns at once; reap it so no zombie is left.
            let _ = child.wait();
            Ok(())
        }
        // No `setsid` on this system: run the script directly, still without waiting for it.
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && plan.args.first().map(String::as_str) == Some("--fork") =>
        {
            run(Path::new(&plan.args[1]), &plan.args[2..])
                .map(|_| ())
                .map_err(|e| UpdateError::Install(e.to_string()))
        }
        Err(e) => Err(UpdateError::Install(e.to_string())),
    }
}

#[cfg(test)]
mod tests;
