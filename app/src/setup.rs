//! The apps CargoFrame leans on, whether they are on this computer, and the one button that puts
//! the missing ones there.
//!
//! CargoFrame itself needs nothing installed to take a screenshot and share it. A few extras need
//! another program, and each says what it is for:
//!
//! * **ffmpeg** (with ffprobe): recording video, and its sound. On Windows the button installs it
//!   with `winget`, Windows' own package manager: the package is fixed (`Gyan.FFmpeg`), winget
//!   checks the download against the hash in its catalogue, and it installs for this user only
//!   (no administrator prompt). On Linux a program cannot install packages without a password, so
//!   the button copies the one command to run.
//! * **The sound of the computer** (Windows): ffmpeg can only record it through a loopback device
//!   such as *Stereo Mix*. CargoFrame cannot add one by a click, so it opens the Windows sound
//!   settings where it is switched on.
//! * **A sound system** (Linux): PulseAudio or PipeWire, for the sound in videos.
//! * **xdotool** (Linux, X11): lets CargoFrame press Ctrl+V and Enter in a Claude chat.
//!
//! [`report`] is plain data in, plain data out, so every sentence here is tested. Looking at the
//! computer is [`probe`]; installing is [`Installer`].

use crate::video::Os;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The winget package for ffmpeg: a hash-checked, per-user install of the "full" build.
pub const FFMPEG_PACKAGE: &str = "Gyan.FFmpeg";

/// How long winget may take before it is stopped.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// What `winget` is run with: the package is fixed, nothing a person typed goes into it.
pub fn winget_args() -> Vec<&'static str> {
    vec![
        "install",
        "--id",
        FFMPEG_PACKAGE,
        "--exact",
        "--silent",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--disable-interactivity",
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// There.
    Ok,
    /// Not there, so what it is for does not work yet.
    Missing,
    /// Cannot be told yet (it depends on something that is missing).
    Unknown,
}

/// One app and what it is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub id: &'static str,
    pub name: &'static str,
    /// What does not work without it, in a sentence.
    pub what_for: &'static str,
    pub status: Status,
    /// Where it was found, or what is wrong.
    pub detail: String,
    /// A button on this line, if there is something to open (`sound_settings`).
    pub action: Option<&'static str>,
}

/// What the one button does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Install {
    /// Nothing to do.
    None,
    /// The button installs what is missing.
    Run { label: String },
    /// The button copies this command, to run in a terminal.
    Copy { command: String },
    /// Nothing can be done from here; this says what to do.
    Manual { text: String },
    /// An install is in progress.
    Running,
    /// The last install did not work.
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub os: &'static str,
    pub items: Vec<Item>,
    /// How many are missing: the number on the Setup page's tab.
    pub missing: usize,
    pub install: Install,
}

/// How a Linux distribution installs packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packages {
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Unknown,
}

impl Packages {
    /// From the text of `/etc/os-release` (its `ID` and `ID_LIKE`).
    pub fn from_os_release(text: &str) -> Self {
        let mut names = Vec::new();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key == "ID" || key == "ID_LIKE" {
                names.extend(
                    value
                        .trim_matches('"')
                        .split_whitespace()
                        .map(str::to_lowercase),
                );
            }
        }
        let has = |n: &str| names.iter().any(|x| x == n);
        if has("debian") || has("ubuntu") {
            Self::Apt
        } else if has("fedora") || has("rhel") || has("centos") {
            Self::Dnf
        } else if has("arch") {
            Self::Pacman
        } else if has("suse") || has("opensuse") {
            Self::Zypper
        } else {
            Self::Unknown
        }
    }

    /// The command that installs `packages`, or `None` when this system is not known.
    pub fn command(self, packages: &[&str]) -> Option<String> {
        let list = packages.join(" ");
        match self {
            Self::Apt => Some(format!("sudo apt install {list}")),
            Self::Dnf => Some(format!("sudo dnf install {list}")),
            Self::Pacman => Some(format!("sudo pacman -S {list}")),
            Self::Zypper => Some(format!("sudo zypper install {list}")),
            Self::Unknown => None,
        }
    }
}

/// What looking at this computer found.
#[derive(Debug, Clone)]
pub struct Probes {
    /// ffmpeg's place and version, or why it was not found.
    pub ffmpeg: Result<(PathBuf, String), String>,
    /// Windows' package manager is there.
    pub winget: bool,
    /// Windows: a loopback device is listed (`None`: could not be asked).
    pub loopback: Option<bool>,
    /// Linux: PulseAudio or PipeWire is running.
    pub sound_system: bool,
    /// Linux: xdotool is installed.
    pub xdotool: bool,
    pub packages: Packages,
}

fn first_sentence(text: &str) -> String {
    let text = text.trim();
    match text.split_once(". ") {
        Some((first, _)) => format!("{first}."),
        None => text.lines().next().unwrap_or(text).to_string(),
    }
}

/// Turn what was found into the lines of the Setup page and the one button.
pub fn report(os: Os, p: &Probes, installing: bool, failed: Option<&str>) -> Report {
    let mut items = Vec::new();

    items.push(match &p.ffmpeg {
        Ok((path, version)) => Item {
            id: "ffmpeg",
            name: "ffmpeg",
            what_for:
                "Recording video, and the sound in it. Screenshots and sharing do not need it.",
            status: Status::Ok,
            detail: format!("Version {version}, {}", path.display()),
            action: None,
        },
        Err(why) => Item {
            id: "ffmpeg",
            name: "ffmpeg",
            what_for:
                "Recording video, and the sound in it. Screenshots and sharing do not need it.",
            status: Status::Missing,
            detail: first_sentence(why),
            action: None,
        },
    });

    match os {
        Os::Windows => items.push(match (&p.ffmpeg, p.loopback) {
            (Err(_), _) | (_, None) => Item {
                id: "computer_sound",
                name: "Sound of the computer",
                what_for: "Lets a video include what this PC plays. Needs a loopback device such as Stereo Mix.",
                status: Status::Unknown,
                detail: "Install ffmpeg first; then this can be checked.".into(),
                action: None,
            },
            (Ok(_), Some(true)) => Item {
                id: "computer_sound",
                name: "Sound of the computer",
                what_for: "Lets a video include what this PC plays. Needs a loopback device such as Stereo Mix.",
                status: Status::Ok,
                detail: "A loopback device is available.".into(),
                action: None,
            },
            (Ok(_), Some(false)) => Item {
                id: "computer_sound",
                name: "Sound of the computer",
                what_for: "Lets a video include what this PC plays. Needs a loopback device such as Stereo Mix.",
                status: Status::Missing,
                detail: "No loopback device found. Turn on \"Stereo Mix\" in the Windows sound settings (Recording tab, right-click, Show disabled devices). Without it, videos have no computer sound.".into(),
                action: Some("sound_settings"),
            },
        }),
        Os::Linux => {
            items.push(Item {
                id: "sound_system",
                name: "Sound system (PulseAudio or PipeWire)",
                what_for: "Records the computer's sound and the microphone in videos.",
                status: if p.sound_system { Status::Ok } else { Status::Missing },
                detail: if p.sound_system {
                    "Running.".into()
                } else {
                    "None is running. Install PipeWire (with its PulseAudio layer) or PulseAudio with your package manager, and sign in again.".into()
                },
                action: None,
            });
            items.push(Item {
                id: "xdotool",
                name: "xdotool",
                what_for: "Lets CargoFrame press Ctrl+V and Enter in a Claude chat for you (X11 only).",
                status: if p.xdotool { Status::Ok } else { Status::Missing },
                detail: if p.xdotool {
                    "Installed.".into()
                } else {
                    "Not installed.".into()
                },
                action: None,
            });
        }
        Os::Other => {}
    }

    let missing = items.iter().filter(|i| i.status == Status::Missing).count();
    let ffmpeg_missing = items
        .iter()
        .any(|i| i.id == "ffmpeg" && i.status == Status::Missing);

    let install = if installing {
        Install::Running
    } else if let Some(message) = failed.filter(|_| ffmpeg_missing) {
        Install::Failed {
            message: message.to_string(),
        }
    } else {
        match os {
            Os::Windows if ffmpeg_missing && p.winget => Install::Run {
                label: "Install ffmpeg".into(),
            },
            Os::Windows if ffmpeg_missing => Install::Manual {
                text: "Windows' package manager (winget) was not found. Install \"App Installer\" from the Microsoft Store, then come back; or download ffmpeg yourself and choose its folder under Video.".into(),
            },
            Os::Linux => {
                let mut wanted = Vec::new();
                if ffmpeg_missing {
                    wanted.push("ffmpeg");
                }
                if items
                    .iter()
                    .any(|i| i.id == "xdotool" && i.status == Status::Missing)
                {
                    wanted.push("xdotool");
                }
                match (wanted.is_empty(), p.packages.command(&wanted)) {
                    (true, _) => Install::None,
                    (false, Some(command)) => Install::Copy { command },
                    (false, None) => Install::Manual {
                        text: format!(
                            "Install {} with your package manager.",
                            wanted.join(" and ")
                        ),
                    },
                }
            }
            _ => Install::None,
        }
    };

    Report {
        os: os.name(),
        items,
        missing,
        install,
    }
}

// ----- looking at the computer -----

/// Is `program` a file in one of the folders of the search path?
fn on_path(program: &str) -> bool {
    cargoframe_video::locate(program, &[]).is_some()
}

/// Where winget puts the shortcuts of what it installs (it is not on the search path of a
/// program that was already running when ffmpeg was installed).
pub fn winget_links() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    std::env::var_os("LOCALAPPDATA").map(|d| {
        PathBuf::from(d)
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
    })
}

/// Look at this computer.
pub fn probe(os: Os, video: &cargoframe_config::Video) -> Probes {
    let ffmpeg = crate::video::find(video, crate::video::program_beside_app().as_deref())
        .map(|f| (f.ffmpeg, f.version));
    let loopback = match (&ffmpeg, os) {
        (Ok((program, _)), Os::Windows) => {
            let devices = crate::audio::list(program, os);
            devices.listed.then_some(!devices.desktop.is_empty())
        }
        _ => None,
    };
    Probes {
        winget: os == Os::Windows && on_path("winget"),
        loopback,
        sound_system: os == Os::Linux && crate::audio::pulse_running(),
        xdotool: os == Os::Linux && on_path("xdotool"),
        packages: if os == Os::Linux {
            std::fs::read_to_string("/etc/os-release")
                .map(|t| Packages::from_os_release(&t))
                .unwrap_or(Packages::Unknown)
        } else {
            Packages::Unknown
        },
        ffmpeg,
    }
}

/// Open the Windows sound settings on the Recording tab, where Stereo Mix is switched on.
pub fn open_sound_settings() -> Result<(), String> {
    if !cfg!(windows) {
        return Err("This is a Windows setting.".into());
    }
    let mut command = Command::new("control.exe");
    command.args(["mmsys.cpl,,1"]);
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the sound settings: {e}"))
}

// ----- installing -----

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Idle,
    Running,
    Failed(String),
}

/// Runs one install at a time, in the background, and remembers how it went.
#[derive(Debug, Default, Clone)]
pub struct Installer {
    phase: Arc<Mutex<Phase>>,
}

impl Installer {
    fn lock(&self) -> std::sync::MutexGuard<'_, Phase> {
        self.phase.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn running(&self) -> bool {
        *self.lock() == Phase::Running
    }

    /// The last failure, until a new install starts or the thing is found.
    pub fn failure(&self) -> Option<String> {
        match &*self.lock() {
            Phase::Failed(m) => Some(m.clone()),
            _ => None,
        }
    }

    /// Forget a failure (the thing was found after all).
    pub fn clear_failure(&self) {
        let mut phase = self.lock();
        if matches!(*phase, Phase::Failed(_)) {
            *phase = Phase::Idle;
        }
    }

    /// Run `job` in the background. Refused while another is running.
    pub fn start(
        &self,
        job: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        {
            let mut phase = self.lock();
            if *phase == Phase::Running {
                return Err("An install is already running.".into());
            }
            *phase = Phase::Running;
        }
        let phase = Arc::clone(&self.phase);
        std::thread::spawn(move || {
            let result = job();
            *phase.lock().unwrap_or_else(|e| e.into_inner()) = match result {
                Ok(()) => Phase::Idle,
                Err(message) => Phase::Failed(message),
            };
        });
        Ok(())
    }
}

/// What winget said at the end, as a short sentence for a person.
pub fn winget_failure(output: &str, code: Option<i32>) -> String {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        // progress bars and spinners are not what it said
        .filter(|l| {
            !l.is_empty()
                && l.chars().any(|c| c.is_alphabetic())
                && !l.contains('█')
                && !l.contains('▒')
        })
        .collect();
    let said = lines
        .iter()
        .rev()
        .take(2)
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    let said: String = said.chars().take(300).collect();
    match (said.is_empty(), code) {
        (true, Some(c)) => {
            format!("winget stopped with code {c}. Try again, or install ffmpeg by hand.")
        }
        (true, None) => "winget stopped before it finished.".into(),
        (false, _) => format!("winget said: {said}"),
    }
}

/// The real install: ffmpeg by winget. Blocks until it is done (call it from a background job).
pub fn install_ffmpeg_with_winget() -> Result<(), String> {
    let mut command = Command::new("winget");
    command
        .args(winget_args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cargoframe_video::quiet(&mut command)
        .spawn()
        .map_err(|e| format!("winget could not be started: {e}"))?;
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut p) = pipe {
                let _ = std::io::Read::read_to_end(&mut p, &mut bytes);
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if started.elapsed() > INSTALL_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("winget took more than 15 minutes and was stopped.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let said = format!(
        "{}\n{}",
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default()
    );
    // "already installed" and similar end with a code that is not 0: whether it worked is
    // decided by looking for ffmpeg afterwards, not by the code
    if status.success() {
        Ok(())
    } else {
        Err(winget_failure(&said, status.code()))
    }
}

/// Has this person seen the Setup page at least once? The first run opens it; this remembers.
/// A folder that cannot be written to says "yes", so the page does not open at every start.
pub fn first_run(config: &Path) -> bool {
    let marker = config.join("setup-seen");
    if marker.exists() {
        return false;
    }
    std::fs::create_dir_all(config)
        .and_then(|()| std::fs::write(&marker, b"1"))
        .is_ok()
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
