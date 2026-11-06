//! The sounds of a recording: which devices the computer has, and which of them a layout's video
//! block asks for.
//!
//! * Linux: ffmpeg speaks to PulseAudio (PipeWire answers the same way). What the computer plays
//!   is the *monitor* of an output; `@DEFAULT_MONITOR@` is the one of whatever is the default.
//! * Windows: ffmpeg speaks DirectShow, which has microphones and, on some PCs, a loopback
//!   device ("Stereo Mix") that carries what the computer plays. ffmpeg has no way of its own to
//!   record the sound of the computer, so without such a device the recording is silent and says
//!   why. (A native loopback capture is a later step.)
//!
//! Nothing here starts a recording; [`plan`] only turns the layout's words into inputs.

use crate::video::Os;
use cargoframe_layout::{AudioChoice, VideoAudio};
use cargoframe_video::recorder::{AudioBackend, AudioSource};
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};

/// What the Pulse monitor of the default output is called to ffmpeg.
pub const DEFAULT_MONITOR: &str = "@DEFAULT_MONITOR@";
/// What the default microphone is called to ffmpeg (Pulse).
pub const DEFAULT_SOURCE: &str = "default";

/// A sound device the person can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    /// What ffmpeg is given, and what a layout stores.
    pub name: String,
    /// What the person reads.
    pub label: String,
    /// The system's own choice.
    pub default: bool,
}

/// The devices of this computer, split by what they are for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Devices {
    /// Which operating system's rules apply (`windows`, `linux` or `other`).
    pub os: String,
    /// Carry what the computer plays.
    pub desktop: Vec<Device>,
    /// Carry what a microphone hears.
    pub microphone: Vec<Device>,
    /// ffmpeg could be asked, and answered. `false` for no ffmpeg, or no sound system.
    pub listed: bool,
    /// What the person should know about their choices, in words.
    pub note: Option<String>,
}

/// Names that mean "what the computer plays" on Windows, in the languages Windows names it in.
const LOOPBACKS: &[&str] = &[
    "stereo mix",
    "stereomix",
    "what u hear",
    "what you hear",
    "wave out",
    "waveout",
    "loopback",
    "virtual-audio-capturer",
    "mixage stéréo",
    "mixage stereo",
    "mezcla estéreo",
    "mezcla estereo",
    "missaggio stereo",
];

fn is_loopback(name: &str) -> bool {
    let name = name.to_lowercase();
    LOOPBACKS.iter().any(|l| name.contains(l))
}

/// The answer of `ffmpeg -sources pulse`:
///
/// ```text
/// Auto-detected sources for pulse:
/// * alsa_output.pci-0000_00_1f.3.analog-stereo.monitor [Monitor of Built-in Audio Analog Stereo]
///   alsa_input.pci-0000_00_1f.3.analog-stereo [Built-in Audio Analog Stereo]
/// ```
pub fn parse_pulse(text: &str) -> Devices {
    let mut found = Devices {
        os: "linux".into(),
        ..Devices::default()
    };
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.ends_with(':') || line.trim_start().starts_with('[') {
            continue;
        }
        let default = line.trim_start().starts_with('*');
        let rest = line.trim_start().trim_start_matches('*').trim();
        let (name, label) = match rest.split_once(" [") {
            Some((n, l)) => (n.trim(), l.trim_end_matches(']').trim()),
            None => (rest, rest),
        };
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        let device = Device {
            name: name.to_string(),
            label: label.to_string(),
            default,
        };
        if name.ends_with(".monitor") {
            found.desktop.push(device);
        } else {
            found.microphone.push(device);
        }
    }
    found.listed = true;
    found
}

/// The answer of `ffmpeg -list_devices true -f dshow -i dummy` (it is printed on stderr). Two
/// forms exist; both are read:
///
/// ```text
/// [dshow @ 000001] DirectShow audio devices
/// [dshow @ 000001]  "Microphone (Realtek)"
/// [dshow @ 000001]     Alternative name "@device_cm_…"
///
/// [dshow @ 000001] "Microphone (Realtek)" (audio)
/// [dshow @ 000001]   Alternative name "@device_cm_…"
/// ```
pub fn parse_dshow(text: &str) -> Devices {
    #[derive(PartialEq, Clone, Copy)]
    enum Section {
        None,
        Video,
        Audio,
    }
    let mut found = Devices {
        os: "windows".into(),
        ..Devices::default()
    };
    let mut section = Section::None;
    for line in text.lines() {
        // the "[dshow @ 0000] " the line starts with
        let rest = match line.find("] ") {
            Some(i) if line.trim_start().starts_with("[dshow") => &line[i + 2..],
            _ => continue,
        };
        let rest = rest.trim();
        if rest.contains("DirectShow video devices") {
            section = Section::Video;
            continue;
        }
        if rest.contains("DirectShow audio devices") {
            section = Section::Audio;
            continue;
        }
        if rest.starts_with("Alternative name") || !rest.starts_with('"') {
            continue;
        }
        let Some(close) = rest[1..].find('"') else {
            continue;
        };
        let name = &rest[1..1 + close];
        let after = rest[close + 2..].trim();
        let audio = match after {
            "(audio)" => true,
            "(video)" | "(none)" => false,
            // the old form: the section says
            _ => section == Section::Audio,
        };
        if !audio || name.is_empty() {
            continue;
        }
        let device = Device {
            name: name.to_string(),
            label: name.to_string(),
            default: false,
        };
        if is_loopback(name) {
            found.desktop.push(device);
        } else {
            found.microphone.push(device);
        }
    }
    found.listed = true;
    found
}

/// Ask ffmpeg which sound devices there are.
pub fn list(ffmpeg: &Path, os: Os) -> Devices {
    let (args, backend): (&[&str], _) = match os {
        Os::Linux => (&["-hide_banner", "-sources", "pulse"], AudioBackend::Pulse),
        Os::Windows => (
            &[
                "-hide_banner",
                "-list_devices",
                "true",
                "-f",
                "dshow",
                "-i",
                "dummy",
            ],
            AudioBackend::Dshow,
        ),
        Os::Other => {
            return Devices {
                os: "other".into(),
                note: Some("Recording sound is not supported on this system yet.".into()),
                ..Devices::default()
            }
        }
    };
    let mut command = Command::new(ffmpeg);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // this listing ends with an error on Windows: the text is what counts, not the exit code
    let output = cargoframe_video::quiet(&mut command).output();
    let mut found = match output {
        Ok(o) => {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            match backend {
                AudioBackend::Pulse if o.status.success() && !text.contains("Cannot list") => {
                    parse_pulse(&text)
                }
                AudioBackend::Dshow => parse_dshow(&text),
                AudioBackend::Pulse => Devices {
                    os: "linux".into(),
                    note: Some(
                        "No sound system answered (PulseAudio or PipeWire). Recordings will have no sound."
                            .into(),
                    ),
                    ..Devices::default()
                },
            }
        }
        Err(e) => Devices {
            os: if os == Os::Windows {
                "windows"
            } else {
                "linux"
            }
            .into(),
            note: Some(format!("ffmpeg would not start: {e}")),
            ..Devices::default()
        },
    };
    if found.note.is_none() && os == Os::Windows && found.desktop.is_empty() {
        found.note = Some(LOOPBACK_MISSING.into());
    }
    found
}

/// Said when Linux has no PulseAudio or PipeWire to record from.
pub const NO_SOUND_SYSTEM: &str =
    "No sound system (PulseAudio or PipeWire) is running, so this recording has no sound.";

/// Is a PulseAudio server (or PipeWire speaking its language) there to talk to? Looked for as
/// the files and variables it leaves, because asking ffmpeg takes seconds when it is not.
pub fn pulse_running() -> bool {
    if std::env::var_os("PULSE_SERVER").is_some_and(|v| !v.is_empty()) {
        return true;
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|d| std::path::Path::new(&d).join("pulse").join("native"))
        .is_some_and(|p| p.exists())
}

/// Said when Windows has nothing that carries what the computer plays.
pub const LOOPBACK_MISSING: &str = "Windows does not let ffmpeg record the sound of the computer by itself. Turn on \"Stereo Mix\" in the Windows sound settings (Recording tab, right-click, Show disabled devices) or install a loopback device such as virtual-audio-capturer; until then the sound of the computer is left out.";

/// The inputs a layout's video block asks for, and what to tell the person about the ones left
/// out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub sources: Vec<AudioSource>,
    pub notes: Vec<String>,
}

/// Turn the layout's choices into inputs. `devices` is only looked at for what the system must be
/// asked for (Windows, where nothing is "default"); a named device is trusted as it is.
///
/// `sound_system` is whether Linux has a sound system to ask (see [`pulse_running`]); ffmpeg
/// takes seconds to find out that it has not, so it is not asked.
pub fn plan(audio: &VideoAudio, os: Os, sound_system: bool, devices: &dyn Fn() -> Devices) -> Plan {
    let mut plan = Plan::default();
    let backend = match os {
        Os::Linux => AudioBackend::Pulse,
        Os::Windows => AudioBackend::Dshow,
        Os::Other => {
            if audio.desktop != AudioChoice::Off || audio.microphone != AudioChoice::Off {
                plan.notes
                    .push("Recording sound is not supported on this system yet.".into());
            }
            return plan;
        }
    };
    if os == Os::Linux
        && !sound_system
        && (audio.desktop != AudioChoice::Off || audio.microphone != AudioChoice::Off)
    {
        plan.notes.push(NO_SOUND_SYSTEM.into());
        return plan;
    }
    let asked_for_default =
        audio.desktop == AudioChoice::Default || audio.microphone == AudioChoice::Default;
    // Linux names its defaults itself; Windows has to be asked
    let listing = (os == Os::Windows && asked_for_default).then(devices);

    let push = |plan: &mut Plan, device: String| {
        if !plan.sources.iter().any(|s| s.device == device) {
            plan.sources.push(AudioSource { backend, device });
        }
    };

    match &audio.desktop {
        AudioChoice::Off => {}
        AudioChoice::Device(name) => push(&mut plan, name.clone()),
        AudioChoice::Default => match os {
            Os::Linux => push(&mut plan, DEFAULT_MONITOR.into()),
            _ => match listing.as_ref().and_then(|l| l.desktop.first()) {
                Some(d) => push(&mut plan, d.name.clone()),
                None => plan.notes.push(LOOPBACK_MISSING.into()),
            },
        },
    }
    match &audio.microphone {
        AudioChoice::Off => {}
        AudioChoice::Device(name) => push(&mut plan, name.clone()),
        AudioChoice::Default => match os {
            Os::Linux => push(&mut plan, DEFAULT_SOURCE.into()),
            _ => match listing.as_ref().and_then(|l| l.microphone.first()) {
                Some(d) => push(&mut plan, d.name.clone()),
                None => plan.notes.push("No microphone was found.".into()),
            },
        },
    }
    plan
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
