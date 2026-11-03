//! Settings, Video: finding ffmpeg, choosing how the screen is captured and compressed, and a
//! short test recording the person can run on their own PC.
//!
//! Which capture and which encoder work best is something only a real machine can say (spike
//! S9), so nothing here guesses beyond a safe default: "Auto" is software compression of the old
//! GDI capture on Windows and x11grab on an X11 desktop, and the test recording tells the person
//! at once whether another choice works on their PC.

use cargoframe_config::{Video, VideoCapture, VideoEncoder};
use cargoframe_video::recorder::{self, Capture, Config, Encoder, Recorder, X264Preset};
use cargoframe_video::{ask, locate_in, probe, Window};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long the test recording runs.
pub const TRIAL_SECONDS: u64 = 5;
/// The name of the test clip, kept in `video-test/` until the next test replaces it.
pub const TRIAL_CLIP: &str = "test-clip.mp4";

/// Only one test recording at a time.
static TRIAL: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    Other,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Other => "other",
        }
    }
}

/// What the desktop session looks like, for the Linux capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub wayland: bool,
    pub display: Option<String>,
}

impl Session {
    pub fn current() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self {
            wayland: var("XDG_SESSION_TYPE").is_some_and(|t| t.eq_ignore_ascii_case("wayland")),
            display: var("DISPLAY"),
        }
    }
}

/// What this ffmpeg can do, read from its own lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub version: String,
    pub encoders: Vec<VideoEncoder>,
    pub captures: Vec<VideoCapture>,
}

/// Where the settings window is told things stand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolsView {
    pub found: bool,
    pub ffmpeg: Option<String>,
    pub version: Option<String>,
    pub encoders: Vec<VideoEncoder>,
    pub captures: Vec<VideoCapture>,
    /// What "Auto" would use, in the same words as the choices.
    pub auto_capture: Option<VideoCapture>,
    pub auto_encoder: Option<VideoEncoder>,
    pub os: &'static str,
    /// Why nothing can be recorded, in words.
    pub problem: Option<String>,
}

/// The folders to look in: the one the person named (a folder, or the ffmpeg program itself),
/// else the one next to CargoFrame, else the search path.
pub fn search_dirs(setting: Option<&str>, beside_app: Option<&Path>) -> (Vec<PathBuf>, bool) {
    if let Some(s) = setting.map(str::trim).filter(|s| !s.is_empty()) {
        let p = PathBuf::from(s);
        let dir = if p.is_file() {
            p.parent().map(Path::to_path_buf).unwrap_or(p)
        } else {
            p
        };
        // a place the person chose is the only place looked in
        return (vec![dir], true);
    }
    (
        beside_app.map(Path::to_path_buf).into_iter().collect(),
        false,
    )
}

fn program_beside_app() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
}

/// Find ffmpeg and ffprobe and ask ffmpeg what it can do.
pub fn find(setting: &Video, beside_app: Option<&Path>) -> Result<Found, String> {
    let (dirs, chosen) = search_dirs(setting.ffmpeg.as_deref(), beside_app);
    let locate = |name: &str| -> Option<PathBuf> {
        if chosen {
            locate_in(name, &dirs)
        } else {
            cargoframe_video::locate(name, &dirs)
        }
    };
    let place = || {
        dirs.first()
            .map(|d| d.display().to_string())
            .unwrap_or_default()
    };
    let Some(ffmpeg) = locate("ffmpeg") else {
        return Err(if chosen {
            format!("ffmpeg was not found in {}.", place())
        } else {
            "ffmpeg was not found. Install it (on Windows: winget install Gyan.FFmpeg) or choose the folder it is in.".into()
        });
    };
    let Some(ffprobe) = locate("ffprobe") else {
        return Err(format!(
            "ffprobe was not found next to ffmpeg ({}). It comes in the same download.",
            ffmpeg
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        ));
    };
    let version = ask(&ffmpeg, &["-hide_banner", "-version"])
        .map_err(|e| format!("ffmpeg would not start: {e}"))?;
    let encoders = ask(&ffmpeg, &["-hide_banner", "-encoders"])
        .map_err(|e| format!("ffmpeg would not list its encoders: {e}"))?;
    // old builds may not know the lists; no devices or filters just means fewer choices
    let devices = ask(&ffmpeg, &["-hide_banner", "-devices"]).unwrap_or_default();
    let filters = ask(&ffmpeg, &["-hide_banner", "-filters"]).unwrap_or_default();
    Ok(Found {
        ffmpeg,
        ffprobe,
        version: parse_version(&version),
        encoders: parse_encoders(&encoders),
        captures: parse_captures(&devices, &filters),
    })
}

/// "ffmpeg version 7.1-full_build ..." -> "7.1-full_build"
pub fn parse_version(text: &str) -> String {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("ffmpeg version "))
        .map(|v| v.split_whitespace().next().unwrap_or(v).to_string())
        .unwrap_or_else(|| "unknown version".into())
}

/// Which H.264 encoders `ffmpeg -encoders` lists. Listed does not mean usable on this PC.
pub fn parse_encoders(text: &str) -> Vec<VideoEncoder> {
    let names = [
        ("libx264", VideoEncoder::Software),
        ("h264_nvenc", VideoEncoder::Nvenc),
        ("h264_qsv", VideoEncoder::Qsv),
        ("h264_amf", VideoEncoder::Amf),
        ("h264_mf", VideoEncoder::MediaFoundation),
        ("h264_vaapi", VideoEncoder::Vaapi),
    ];
    let listed: Vec<&str> = text
        .lines()
        // the flags column ("V....D") comes first, then the name
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    names
        .iter()
        .filter(|(n, _)| listed.contains(n))
        .map(|(_, e)| *e)
        .collect()
}

/// Which screen captures this ffmpeg has: devices come from `-devices`, `ddagrab` is a filter.
pub fn parse_captures(devices: &str, filters: &str) -> Vec<VideoCapture> {
    let second = |text: &str, name: &str| {
        text.lines()
            .any(|l| l.split_whitespace().nth(1) == Some(name))
    };
    let mut found = Vec::new();
    if second(devices, "gdigrab") {
        found.push(VideoCapture::Gdigrab);
    }
    if second(filters, "ddagrab") {
        found.push(VideoCapture::Ddagrab);
    }
    if second(devices, "x11grab") {
        found.push(VideoCapture::X11);
    }
    found
}

/// What will actually be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub capture: VideoCapture,
    pub encoder: VideoEncoder,
}

/// Turn the settings ("Auto" included) into a capture and an encoder this ffmpeg has, or say
/// in words why not.
pub fn choose(setting: &Video, found: &Found, os: Os, session: &Session) -> Result<Choice, String> {
    let capture = match setting.capture {
        VideoCapture::Auto => match os {
            Os::Windows => [VideoCapture::Gdigrab, VideoCapture::Ddagrab]
                .into_iter()
                .find(|c| found.captures.contains(c))
                .ok_or("This ffmpeg cannot capture the Windows screen (it has neither gdigrab nor ddagrab). Use a full build, for example winget install Gyan.FFmpeg.")?,
            Os::Linux => {
                if session.wayland {
                    return Err("This desktop runs Wayland, and recording its screen is not supported yet. Log in with an X11 session to record.".into());
                }
                if !found.captures.contains(&VideoCapture::X11) {
                    return Err("This ffmpeg cannot capture an X11 screen (no x11grab).".into());
                }
                VideoCapture::X11
            }
            Os::Other => return Err("Recording is not supported on this system yet.".into()),
        },
        explicit => {
            if !found.captures.contains(&explicit) {
                return Err(format!(
                    "This ffmpeg does not have the {} capture.",
                    capture_name(explicit)
                ));
            }
            if explicit == VideoCapture::X11 && session.wayland {
                return Err("This desktop runs Wayland: X11 capture would only see old-style windows. Log in with an X11 session to record.".into());
            }
            explicit
        }
    };
    let encoder = match setting.encoder {
        VideoEncoder::Auto => {
            // software works everywhere; a build without it falls back to what it has
            [
                VideoEncoder::Software,
                VideoEncoder::Nvenc,
                VideoEncoder::Qsv,
                VideoEncoder::Amf,
                VideoEncoder::MediaFoundation,
                VideoEncoder::Vaapi,
            ]
            .into_iter()
            .find(|e| found.encoders.contains(e))
            .ok_or("This ffmpeg has no H.264 encoder. Use a full build, for example winget install Gyan.FFmpeg.")?
        }
        explicit => {
            if !found.encoders.contains(&explicit) {
                return Err(format!(
                    "This ffmpeg does not have the {} encoder.",
                    encoder_name(explicit)
                ));
            }
            explicit
        }
    };
    Ok(Choice { capture, encoder })
}

pub fn capture_name(c: VideoCapture) -> &'static str {
    match c {
        VideoCapture::Auto => "automatic",
        VideoCapture::Gdigrab => "GDI",
        VideoCapture::Ddagrab => "Desktop Duplication",
        VideoCapture::X11 => "X11",
    }
}

pub fn encoder_name(e: VideoEncoder) -> &'static str {
    match e {
        VideoEncoder::Auto => "automatic",
        VideoEncoder::Software => "software (x264)",
        VideoEncoder::Nvenc => "NVIDIA (NVENC)",
        VideoEncoder::Qsv => "Intel (Quick Sync)",
        VideoEncoder::Amf => "AMD (AMF)",
        VideoEncoder::MediaFoundation => "Windows built-in",
        VideoEncoder::Vaapi => "VAAPI",
    }
}

/// The settings must be sensible before they are stored or used.
pub fn validate(video: &Video) -> Result<(), String> {
    if !(Video::MIN_FPS..=Video::MAX_FPS).contains(&video.fps) {
        return Err(format!(
            "Frames per second must be between {} and {}",
            Video::MIN_FPS,
            Video::MAX_FPS
        ));
    }
    if !(1..=Video::MAX_WINDOW_SECS).contains(&video.window_secs) {
        return Err(format!(
            "The length kept must be between 1 and {} seconds",
            Video::MAX_WINDOW_SECS
        ));
    }
    if video.max_height != 0 && !(240..=4320).contains(&video.max_height) {
        return Err("The largest height must be 0 (no limit) or between 240 and 4320".into());
    }
    Ok(())
}

/// Tidy what the person typed.
pub fn normalized(mut video: Video) -> Video {
    video.ffmpeg = video
        .ffmpeg
        .map(|f| f.trim().trim_matches('"').trim().to_string())
        .filter(|f| !f.is_empty());
    video
}

/// The recorder's own settings for a choice.
pub fn recorder_config(
    found: &Found,
    choice: &Choice,
    video: &Video,
    session: &Session,
    dir: &Path,
) -> Config {
    let capture = match choice.capture {
        VideoCapture::Gdigrab => Capture::Gdigrab,
        VideoCapture::Ddagrab => Capture::Ddagrab { output: 0 },
        // Auto never reaches here; X11 takes the whole screen
        _ => Capture::X11 {
            display: session.display.clone().unwrap_or_else(|| ":0".into()),
            size: None,
        },
    };
    let mut cfg = Config::new(found.ffmpeg.clone(), capture, dir.to_path_buf());
    cfg.encoder = match choice.encoder {
        VideoEncoder::Nvenc => Encoder::Nvenc,
        VideoEncoder::Qsv => Encoder::Qsv,
        VideoEncoder::Amf => Encoder::Amf,
        VideoEncoder::MediaFoundation => Encoder::MediaFoundation,
        VideoEncoder::Vaapi => Encoder::Vaapi {
            device: PathBuf::from("/dev/dri/renderD128"),
        },
        _ => Encoder::X264 {
            preset: X264Preset::Ultrafast,
        },
    };
    cfg.fps = video.fps.clamp(recorder::MIN_FPS, recorder::MAX_FPS);
    cfg.window = Window::seconds(video.window_secs);
    cfg.max_height = (video.max_height != 0).then_some(video.max_height);
    cfg
}

/// What the settings window shows after "Test ffmpeg".
pub fn tools_view(video: &Video, os: Os, session: &Session) -> ToolsView {
    let found = match find(video, program_beside_app().as_deref()) {
        Ok(f) => f,
        Err(problem) => {
            return ToolsView {
                found: false,
                ffmpeg: None,
                version: None,
                encoders: vec![],
                captures: vec![],
                auto_capture: None,
                auto_encoder: None,
                os: os.name(),
                problem: Some(problem),
            }
        }
    };
    let auto = choose(
        &Video {
            capture: VideoCapture::Auto,
            encoder: VideoEncoder::Auto,
            ..video.clone()
        },
        &found,
        os,
        session,
    );
    let (auto_capture, auto_encoder, problem) = match auto {
        Ok(c) => (Some(c.capture), Some(c.encoder), None),
        Err(p) => (None, None, Some(p)),
    };
    ToolsView {
        found: true,
        ffmpeg: Some(found.ffmpeg.display().to_string()),
        version: Some(found.version),
        encoders: found.encoders,
        captures: found.captures,
        auto_capture,
        auto_encoder,
        os: os.name(),
        problem,
    }
}

/// How the test recording went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trial {
    pub ok: bool,
    /// One or two sentences for the person.
    pub message: String,
    pub capture: Option<VideoCapture>,
    pub encoder: Option<VideoEncoder>,
    pub width: u32,
    pub height: u32,
    pub clip_ms: u64,
    pub clip_bytes: u64,
    /// Wall time from start to finished clip.
    pub took_ms: u64,
    /// Where the clip is, to open and watch.
    pub clip: Option<String>,
}

impl Trial {
    fn failed(message: impl Into<String>, choice: Option<&Choice>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            capture: choice.map(|c| c.capture),
            encoder: choice.map(|c| c.encoder),
            width: 0,
            height: 0,
            clip_ms: 0,
            clip_bytes: 0,
            took_ms: 0,
            clip: None,
        }
    }
}

/// Words to add to a failure, from what was being tried.
fn hint(choice: &Choice) -> &'static str {
    match (choice.capture, choice.encoder) {
        (_, e) if e != VideoEncoder::Software && e != VideoEncoder::Auto => {
            " Hardware encoders need the matching graphics card and driver: choose Software to carry on."
        }
        (VideoCapture::Ddagrab, _) => {
            " Desktop Duplication does not work on every PC (remote sessions and some virtual machines): try GDI."
        }
        _ => "",
    }
}

/// Record the screen for a few seconds with the chosen settings and report what came out.
/// `work` is a folder for this test; the clip stays in it as [`TRIAL_CLIP`].
pub fn trial(video: &Video, os: Os, session: &Session, work: &Path, seconds: u64) -> Trial {
    let Ok(_one_at_a_time) = TRIAL.try_lock() else {
        return Trial::failed("A test recording is already running.", None);
    };
    if let Err(e) = validate(video) {
        return Trial::failed(e, None);
    }
    let found = match find(video, program_beside_app().as_deref()) {
        Ok(f) => f,
        Err(e) => return Trial::failed(e, None),
    };
    let choice = match choose(video, &found, os, session) {
        Ok(c) => c,
        Err(e) => return Trial::failed(e, None),
    };
    run_trial(&found, &choice, video, session, work, seconds)
}

fn run_trial(
    found: &Found,
    choice: &Choice,
    video: &Video,
    session: &Session,
    work: &Path,
    seconds: u64,
) -> Trial {
    let fail = |m: String| Trial::failed(format!("{m}{}", hint(choice)), Some(choice));
    let _ = std::fs::remove_dir_all(work);
    let ring = work.join("ring");
    if let Err(e) = std::fs::create_dir_all(&ring) {
        return fail(format!(
            "CargoFrame could not make a folder for the test: {e}"
        ));
    }
    let mut cfg = recorder_config(found, choice, video, session, &ring);
    // the clip is the whole test, not just the last seconds of a long recording
    cfg.window = Window::seconds(u32::try_from(seconds).unwrap_or(5).max(2) + 5);
    let began = Instant::now();
    let mut rec = match Recorder::start(&cfg) {
        Ok(r) => r,
        Err(e) => return fail(e.to_string()),
    };
    while rec.elapsed() < Duration::from_secs(seconds) {
        if let Err(e) = rec.poll() {
            rec.cancel();
            let _ = std::fs::remove_dir_all(&ring);
            return fail(e.to_string());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let segments = match rec.stop(Duration::from_secs(10)) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&ring);
            return fail(e.to_string());
        }
    };
    let clip = work.join(TRIAL_CLIP);
    let joined = recorder::assemble(&found.ffmpeg, &segments, work, &clip, &|| false);
    drop(segments);
    let _ = std::fs::remove_dir_all(&ring);
    if let Err(e) = joined {
        return fail(e.to_string());
    }
    let info = match probe(&found.ffprobe, &clip) {
        Ok(i) => i,
        Err(e) => return fail(format!("The clip could not be read back: {e}")),
    };
    let bytes = std::fs::metadata(&clip).map(|m| m.len()).unwrap_or(0);
    let took_ms = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
    Trial {
        ok: true,
        message: format!(
            "Recorded {:.1} s at {}×{} with {} and {}. Open the clip to check that it shows your screen.",
            info.duration_ms as f64 / 1000.0,
            info.width,
            info.height,
            capture_name(choice.capture),
            encoder_name(choice.encoder),
        ),
        capture: Some(choice.capture),
        encoder: Some(choice.encoder),
        width: info.width,
        height: info.height,
        clip_ms: info.duration_ms,
        clip_bytes: bytes,
        took_ms,
        clip: Some(clip.display().to_string()),
    }
}

#[cfg(test)]
#[path = "video_tests.rs"]
mod tests;
