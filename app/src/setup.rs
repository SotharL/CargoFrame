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
//! * **A sound output**: the speakers or headphones the computer plays on, which is where a video's
//!   computer sound comes from. It is looked up fresh at every check (Windows: the default
//!   playback device; Linux: PulseAudio's or PipeWire's default sink), and when there is none the
//!   page says so. CargoFrame listens to what that output plays itself (Windows: the sound system's
//!   loopback; Linux: the monitor of the default sink), so no extra device is needed. On Windows
//!   the line has a button that opens the sound settings, on the Playback tab, to change it.
//! * **The Claude desktop app** (Windows): where a Claude connection opens a chat when it is set
//!   to use the app. It is listed only while such a connection exists, and the same button
//!   installs it with `winget` (the catalogue's package `Anthropic.Claude`, hash-checked).
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

/// The winget package for the Claude desktop app.
pub const CLAUDE_PACKAGE: &str = "Anthropic.Claude";

/// How long winget may take before it is stopped.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// What plays the computer's sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// The default one, by the name the system gives it.
    Found(String),
    /// There is none, and why (no device, no sound system).
    None(String),
    /// The computer could not be asked.
    Unknown,
}

/// What `winget` is run with: the package is one of ours, nothing a person typed goes into it.
pub fn winget_args(package: &'static str) -> Vec<&'static str> {
    vec![
        "install",
        "--id",
        package,
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
    /// The default sound output.
    pub output: Output,
    /// Windows: whether the Claude desktop app is installed. `None`: no Claude connection opens
    /// it, or the computer could not be asked, so the page says nothing about it.
    pub claude_app: Option<bool>,
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

const SOUND_WHAT_FOR: &str =
    "The speakers or headphones this computer plays on. A video records what it plays.";

/// The line about the sound output: which device is the default, and what that means for videos.
fn sound_output_item(os: Os, p: &Probes) -> Item {
    let line = |status, detail: String, action| Item {
        id: "sound_output",
        name: "Sound output",
        what_for: SOUND_WHAT_FOR,
        status,
        detail,
        action,
    };
    match &p.output {
        Output::Found(name) => line(
            Status::Ok,
            format!("{name}. Videos record this sound."),
            // Windows keeps the choice of output in its own settings
            (os == Os::Windows).then_some("sound_settings"),
        ),
        Output::None(why) => line(
            Status::Missing,
            why.clone(),
            (os == Os::Windows).then_some("sound_settings"),
        ),
        Output::Unknown => line(
            Status::Unknown,
            "This computer could not be asked. Press Check again.".into(),
            None,
        ),
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

    if os != Os::Other {
        items.push(sound_output_item(os, p));
    }
    if os == Os::Linux {
        items.push(Item {
            id: "xdotool",
            name: "xdotool",
            what_for: "Lets CargoFrame press Ctrl+V and Enter in a Claude chat for you (X11 only).",
            status: if p.xdotool {
                Status::Ok
            } else {
                Status::Missing
            },
            detail: if p.xdotool {
                "Installed.".into()
            } else {
                "Not installed.".into()
            },
            action: None,
        });
    }

    if let (Os::Windows, Some(found)) = (os, p.claude_app) {
        items.push(Item {
            id: "claude_app",
            name: "Claude desktop app",
            what_for: "Where a Claude connection opens your chat. Without it, set that connection to open in the browser.",
            status: if found { Status::Ok } else { Status::Missing },
            detail: if found {
                "Installed.".into()
            } else {
                "Not installed.".into()
            },
            action: None,
        });
    }

    let missing = items.iter().filter(|i| i.status == Status::Missing).count();
    let is_missing = |id: &str| {
        items
            .iter()
            .any(|i| i.id == id && i.status == Status::Missing)
    };
    let ffmpeg_missing = is_missing("ffmpeg");
    let claude_missing = is_missing("claude_app");
    let windows_wants = ffmpeg_missing || claude_missing;

    let install = if installing {
        Install::Running
    } else if let Some(message) = failed.filter(|_| match os {
        Os::Windows => windows_wants,
        _ => ffmpeg_missing,
    }) {
        Install::Failed {
            message: message.to_string(),
        }
    } else {
        match os {
            Os::Windows if windows_wants && p.winget => Install::Run {
                label: match (ffmpeg_missing, claude_missing) {
                    (true, true) => "Install what is missing",
                    (true, false) => "Install ffmpeg",
                    _ => "Install the Claude app",
                }
                .into(),
            },
            Os::Windows if windows_wants => Install::Manual {
                text: format!(
                    "Windows' package manager (winget) was not found. Install \"App Installer\" from the Microsoft Store, then come back{}{}",
                    if ffmpeg_missing {
                        "; or download ffmpeg yourself and choose its folder under Video"
                    } else {
                        ""
                    },
                    if claude_missing {
                        "; or get the Claude app at claude.ai/download"
                    } else {
                        ""
                    },
                ) + ".",
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

/// Asked of PowerShell: the id of Windows' default playback device, and its name from the registry
/// (`PKEY_Device_FriendlyName`, which is what the sound settings show). The text is fixed; nothing a
/// person typed goes into it. It answers one line: `OK|name`, `NONE` or `ERROR|why`.
const WINDOWS_OUTPUT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
try {
  [void][Windows.Media.Devices.MediaDevice, Windows.Media.Devices, ContentType = WindowsRuntime]
  $id = [Windows.Media.Devices.MediaDevice]::GetDefaultAudioRenderId(0)
} catch { 'ERROR|' + $_.Exception.Message; exit 0 }
if ([string]::IsNullOrEmpty($id)) { 'NONE'; exit 0 }
$guid = [regex]::Match($id, '\{0\.0\.0\.00000000\}\.(\{[0-9a-fA-F-]+\})').Groups[1].Value
$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\' + $guid
$p = Get-ItemProperty -Path ($key + '\Properties') -ErrorAction SilentlyContinue
$name = $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},14'
if (-not $name) { $name = $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2' }
if (-not $name) { $name = 'the default playback device' }
'OK|' + $name
"#;

/// Read PowerShell's one-line answer about the default playback device.
pub fn parse_windows_output(text: &str) -> Output {
    let line = text
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("");
    if let Some(name) = line.strip_prefix("OK|") {
        return Output::Found(name.trim().to_string());
    }
    if line == "NONE" {
        return Output::None(
            "Windows has no default playback device: no speakers or headphones are set, or none is plugged in. Plug one in, or choose one in the Windows sound settings (the button).".into(),
        );
    }
    Output::Unknown
}

/// Read what `pactl` says: `info` names the default sink, `sinks` (`pactl list sinks`) describes
/// each one.
pub fn parse_pactl_output(info: &str, sinks: &str) -> Output {
    let default = info
        .lines()
        .find_map(|l| l.trim().strip_prefix("Default Sink:"))
        .map(str::trim)
        .unwrap_or("");
    if default.is_empty() || default == "auto_null" {
        return Output::None(
            "The sound system has no output: no speakers or headphones are set, or none is plugged in. Choose one in your system's sound settings.".into(),
        );
    }
    // the description of that sink, if the listing has one
    let mut current = "";
    let mut description = None;
    for l in sinks.lines().map(str::trim) {
        if let Some(n) = l.strip_prefix("Name:") {
            current = n.trim();
        } else if let Some(d) = l.strip_prefix("Description:") {
            if current == default {
                description = Some(d.trim().to_string());
                break;
            }
        }
    }
    Output::Found(description.unwrap_or_else(|| default.to_string()))
}

/// PowerShell's raw answer about the default playback device.
fn ask_windows_output() -> Result<String, cargoframe_video::ExportError> {
    cargoframe_video::ask(
        Path::new("powershell.exe"),
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            WINDOWS_OUTPUT_SCRIPT,
        ],
    )
}

/// Asked of PowerShell: is an app called Claude in the Start menu (however it was installed)? The
/// text is fixed. It answers `yes` or `no`.
const CLAUDE_APP_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
if (Get-StartApps | Where-Object { $_.Name -like 'Claude*' }) { 'yes' } else { 'no' }
"#;

/// Read PowerShell's `yes` or `no`; anything else is "could not be told".
pub fn parse_yes_no(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// Is the Claude desktop app installed? Asked fresh each time; `None` when it cannot be told.
pub fn claude_app_installed() -> Option<bool> {
    if !cfg!(windows) {
        return None;
    }
    cargoframe_video::ask(
        Path::new("powershell.exe"),
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            CLAUDE_APP_SCRIPT,
        ],
    )
    .ok()
    .and_then(|text| parse_yes_no(&text))
}

/// Ask this computer for the default sound output, fresh each time.
fn sound_output(os: Os) -> Output {
    match os {
        Os::Windows => match ask_windows_output() {
            Ok(text) => parse_windows_output(&text),
            Err(_) => Output::Unknown,
        },
        Os::Linux => {
            if !crate::audio::pulse_running() {
                return Output::None(
                    "No sound system is running. Install PipeWire (with its PulseAudio layer) or PulseAudio with your package manager, and sign in again.".into(),
                );
            }
            let Some(pactl) = cargoframe_video::locate("pactl", &[]) else {
                return Output::Unknown;
            };
            match cargoframe_video::ask(&pactl, &["info"]) {
                Ok(info) => {
                    let sinks =
                        cargoframe_video::ask(&pactl, &["list", "sinks"]).unwrap_or_default();
                    parse_pactl_output(&info, &sinks)
                }
                Err(_) => Output::Unknown,
            }
        }
        Os::Other => Output::Unknown,
    }
}

/// Look at this computer.
pub fn probe(os: Os, video: &cargoframe_config::Video, wants_claude_app: bool) -> Probes {
    let ffmpeg = crate::video::find(video, crate::video::program_beside_app().as_deref())
        .map(|f| (f.ffmpeg, f.version));
    Probes {
        winget: os == Os::Windows && on_path("winget"),
        output: sound_output(os),
        claude_app: if os == Os::Windows && wants_claude_app {
            claude_app_installed()
        } else {
            None
        },
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

/// Open the Windows sound settings on the Playback tab, where the output the computer plays on is
/// chosen.
pub fn open_sound_settings() -> Result<(), String> {
    if !cfg!(windows) {
        return Err("This is a Windows setting.".into());
    }
    let mut command = Command::new("control.exe");
    command.args(["mmsys.cpl,,0"]);
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
            format!("winget stopped with code {c}. Try again, or install the app by hand.")
        }
        (true, None) => "winget stopped before it finished.".into(),
        (false, _) => format!("winget said: {said}"),
    }
}

/// The real install: one of our packages by winget. Blocks until it is done (call it from a
/// background job).
pub fn install_with_winget(package: &'static str) -> Result<(), String> {
    let mut command = Command::new("winget");
    command
        .args(winget_args(package))
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
    // decided by looking for the app afterwards, not by the code
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
