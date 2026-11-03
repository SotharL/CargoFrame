//! Settings, Video: finding ffmpeg, choosing how the screen is captured and compressed, and a
//! short test recording the person can run on their own PC.
//!
//! Which capture and which encoder work best is something only a real machine can say (spike
//! S9), so nothing here guesses beyond a safe default: "Auto" is software compression of the old
//! GDI capture on Windows and x11grab on an X11 desktop, and the test recording tells the person
//! at once whether another choice works on their PC.

use cargoframe_config::{Video, VideoCapture, VideoEncoder};
use cargoframe_video::recorder::{self, Capture, Config, Encoder, Recorder, X264Preset};
use cargoframe_video::{ask, locate_in, plan, probe, run, Crop, Edit, Overlay, Window};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long the test recording runs.
pub const TRIAL_SECONDS: u64 = 5;
/// The name of the test clip, kept in `video-test/` until the next test replaces it.
pub const TRIAL_CLIP: &str = "test-clip.mp4";
/// The name of the copy made by "Export a copy".
pub const EXPORTED_CLIP: &str = "exported.mp4";

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

/// How long a recording started from Settings may run before it is stopped for the person.
const MANUAL_LIMIT: Duration = Duration::from_secs(600);

/// A recording that is running.
struct Active {
    recorder: Recorder,
    found: Found,
    choice: Choice,
    work: PathBuf,
    /// How many seconds the clip keeps; the rest of a long recording is thrown away as it goes.
    keep_secs: u32,
    began: Instant,
}

enum Slot {
    Idle {
        /// Why the last recording ended by itself, until the person has been told.
        failure: Option<String>,
    },
    Running(Box<Active>),
    /// Stopped; the clip is being joined.
    Finishing,
}

/// What the settings window shows of a recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RecordingView {
    Idle,
    Recording {
        elapsed_ms: u64,
        /// How much of the end of the recording is kept right now.
        buffered_ms: u64,
        /// The disk budget, not the window, cut the start.
        squeezed: bool,
        keep_secs: u32,
    },
    Finishing,
    /// It ended by itself.
    Failed {
        message: String,
    },
}

/// The one recording that can run at a time: started, watched and stopped by commands.
pub struct Recording {
    slot: Mutex<Slot>,
}

impl Default for Recording {
    fn default() -> Self {
        Self::new()
    }
}

impl Recording {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(Slot::Idle { failure: None }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Find ffmpeg, choose how to record, and start. `keep_secs` is how much of the end the
    /// clip keeps.
    pub fn start(
        &self,
        video: &Video,
        os: Os,
        session: &Session,
        work: &Path,
        keep_secs: u32,
    ) -> Result<(), String> {
        validate(video)?;
        // refuse early, before looking for ffmpeg, so the message is the right one
        match &*self.lock() {
            Slot::Idle { .. } => {}
            Slot::Running(_) => return Err("A recording is already running.".into()),
            Slot::Finishing => return Err("The last recording is still being finished.".into()),
        }
        let found = find(video, program_beside_app().as_deref())?;
        let choice = choose(video, &found, os, session)?;
        self.start_with(found, choice, video, session, work, keep_secs)
    }

    fn start_with(
        &self,
        found: Found,
        choice: Choice,
        video: &Video,
        session: &Session,
        work: &Path,
        keep_secs: u32,
    ) -> Result<(), String> {
        let mut slot = self.lock();
        match &*slot {
            Slot::Idle { .. } => {}
            Slot::Running(_) => return Err("A recording is already running.".into()),
            Slot::Finishing => return Err("The last recording is still being finished.".into()),
        }
        let _ = std::fs::remove_dir_all(work);
        let ring = work.join("ring");
        std::fs::create_dir_all(&ring)
            .map_err(|e| format!("CargoFrame could not make a folder for the recording: {e}"))?;
        let mut cfg = recorder_config(&found, &choice, video, session, &ring);
        cfg.window = Window::seconds(keep_secs);
        let recorder = Recorder::start(&cfg).map_err(|e| e.to_string())?;
        *slot = Slot::Running(Box::new(Active {
            recorder,
            found,
            choice,
            work: work.to_path_buf(),
            keep_secs: cfg.window.as_secs(),
            began: Instant::now(),
        }));
        Ok(())
    }

    /// Where the recording stands. Also notices a recorder that has stopped by itself, and stops
    /// one that has run past the limit for a recording started by hand.
    pub fn view(&self) -> RecordingView {
        let mut slot = self.lock();
        let (progress, elapsed) = match &mut *slot {
            Slot::Idle { failure: None } => return RecordingView::Idle,
            Slot::Idle {
                failure: Some(message),
            } => {
                return RecordingView::Failed {
                    message: message.clone(),
                }
            }
            Slot::Finishing => return RecordingView::Finishing,
            Slot::Running(a) => (a.recorder.poll(), a.recorder.elapsed()),
        };
        let message = match progress {
            Ok(p) if elapsed <= MANUAL_LIMIT => {
                let Slot::Running(a) = &*slot else {
                    return RecordingView::Idle;
                };
                return RecordingView::Recording {
                    elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                    buffered_ms: p.buffered_ms,
                    squeezed: p.squeezed,
                    keep_secs: a.keep_secs,
                };
            }
            Ok(_) => {
                "The recording was stopped after 10 minutes. Start it again to go on.".to_string()
            }
            Err(e) => e.to_string(),
        };
        // it is over: throw it away and remember why
        let advice = match std::mem::replace(&mut *slot, Slot::Idle { failure: None }) {
            Slot::Running(a) => {
                let advice = hint(&a.choice);
                a.recorder.cancel();
                let _ = std::fs::remove_dir_all(a.work.join("ring"));
                advice
            }
            _ => "",
        };
        let message = format!("{message}{advice}");
        *slot = Slot::Idle {
            failure: Some(message.clone()),
        };
        RecordingView::Failed { message }
    }

    /// Stop, join the pieces into a clip and read it back. If the recording had ended by itself,
    /// this says why.
    pub fn stop(&self) -> Trial {
        let active = {
            let mut slot = self.lock();
            match std::mem::replace(&mut *slot, Slot::Finishing) {
                Slot::Running(a) => a,
                Slot::Idle { failure } => {
                    *slot = Slot::Idle { failure: None };
                    return Trial::failed(
                        failure.unwrap_or_else(|| "No recording is running.".into()),
                        None,
                    );
                }
                Slot::Finishing => {
                    return Trial::failed("The recording is already being finished.", None)
                }
            }
        };
        let trial = finish(*active);
        *self.lock() = Slot::Idle { failure: None };
        trial
    }

    /// Stop and throw away whatever is running (CargoFrame is closing).
    pub fn cancel(&self) {
        if let Slot::Running(a) = std::mem::replace(&mut *self.lock(), Slot::Idle { failure: None })
        {
            let work = a.work.clone();
            a.recorder.cancel();
            let _ = std::fs::remove_dir_all(work.join("ring"));
        }
    }

    /// Record for `seconds` and report. The whole recording is kept (the window is a little
    /// longer than the test), so the clip is as long as the test.
    pub fn trial(
        &self,
        video: &Video,
        os: Os,
        session: &Session,
        work: &Path,
        seconds: u64,
    ) -> Trial {
        match self.start(video, os, session, work, trial_window(seconds)) {
            Ok(()) => self.run_for(seconds),
            Err(e) => Trial::failed(e, None),
        }
    }

    fn run_for(&self, seconds: u64) -> Trial {
        let began = Instant::now();
        while began.elapsed() < Duration::from_secs(seconds) {
            if matches!(self.view(), RecordingView::Failed { .. }) {
                // say why, once
                return self.stop();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.stop()
    }
}

fn trial_window(seconds: u64) -> u32 {
    u32::try_from(seconds).unwrap_or(5).max(2) + 5
}

/// Stop the recorder, join the pieces and read the clip back.
fn finish(active: Active) -> Trial {
    let Active {
        recorder,
        found,
        choice,
        work,
        keep_secs,
        began,
    } = active;
    let fail = |m: String| Trial::failed(format!("{m}{}", hint(&choice)), Some(&choice));
    let ring = work.join("ring");
    let ran = recorder.elapsed();
    let segments = match recorder.stop(Duration::from_secs(10)) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&ring);
            return fail(e.to_string());
        }
    };
    let clip = work.join(TRIAL_CLIP);
    let joined = recorder::assemble(&found.ffmpeg, &segments, &work, &clip, &|| false);
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
    let mut message = format!(
        "Recorded {:.1} s at {}×{} with {} and {}.",
        info.duration_ms as f64 / 1000.0,
        info.width,
        info.height,
        capture_name(choice.capture),
        encoder_name(choice.encoder),
    );
    if u64::try_from(ran.as_millis()).unwrap_or(u64::MAX) > info.duration_ms + 2_500 {
        message.push_str(&format!(
            " The recording ran for {:.0} s and only the last {} s are kept, as they will be in a real recording.",
            ran.as_secs_f64(),
            keep_secs
        ));
    }
    message.push_str(" Open the clip to check that it shows your screen.");
    Trial {
        ok: true,
        message,
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

/// What "Export a copy" is asked to do with the test clip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TestEdit {
    pub start_ms: u64,
    /// `None` keeps the clip to its end.
    pub end_ms: Option<u64>,
    pub crop: Option<Crop>,
    /// Lay a red square over the whole clip, to check that marks are drawn.
    pub mark: bool,
}

impl Recording {
    fn is_idle(&self) -> bool {
        matches!(&*self.lock(), Slot::Idle { .. })
    }
}

/// Cut, crop and mark the test clip into a copy, the way an edited video will be exported.
pub fn export_test(video: &Video, edit: &TestEdit, recording: &Recording, work: &Path) -> Trial {
    if !recording.is_idle() {
        return Trial::failed("Stop the recording first.", None);
    }
    let input = work.join(TRIAL_CLIP);
    if !input.is_file() {
        return Trial::failed(
            "There is no test clip yet: run a test recording first",
            None,
        );
    }
    let found = match find(video, program_beside_app().as_deref()) {
        Ok(f) => f,
        Err(e) => return Trial::failed(e, None),
    };
    let info = match probe(&found.ffprobe, &input) {
        Ok(i) => i,
        Err(e) => return Trial::failed(format!("The test clip could not be read: {e}"), None),
    };
    let mut overlays = Vec::new();
    if edit.mark {
        let image = work.join("mark.png");
        let target = format!("file:{}", image.display());
        // ffmpeg draws the test mark itself: a translucent red square
        let made = ask(
            &found.ffmpeg,
            &[
                "-hide_banner",
                "-nostdin",
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red@0.6:s=120x120,format=rgba",
                "-frames:v",
                "1",
                target.as_str(),
            ],
        );
        if let Err(e) = made {
            return Trial::failed(format!("The test mark could not be made: {e}"), None);
        }
        overlays.push(Overlay {
            image,
            x: 20,
            y: 20,
            from_ms: edit.start_ms,
            to_ms: edit.end_ms.unwrap_or(info.duration_ms),
        });
    }
    let output = work.join(EXPORTED_CLIP);
    let began = Instant::now();
    let edit = Edit {
        start_ms: edit.start_ms,
        end_ms: edit.end_ms,
        crop: edit.crop,
        overlays,
    };
    let planned = match plan(&input, &output, &edit, &info) {
        Ok(p) => p,
        Err(problems) => {
            let said: Vec<String> = problems.iter().map(ToString::to_string).collect();
            return Trial::failed(said.join(". "), None);
        }
    };
    let _ = std::fs::remove_file(&output);
    if let Err(e) = run(&found.ffmpeg, &planned, &|| false, Duration::from_secs(120)) {
        return Trial::failed(format!("The copy could not be made: {e}"), None);
    }
    let out = match probe(&found.ffprobe, &output) {
        Ok(i) => i,
        Err(e) => return Trial::failed(format!("The copy could not be read back: {e}"), None),
    };
    let bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    Trial {
        ok: true,
        message: format!(
            "Made a copy of {:.1} s at {}×{}. Open it to check the cut, the crop and the mark.",
            out.duration_ms as f64 / 1000.0,
            out.width,
            out.height
        ),
        capture: None,
        encoder: None,
        width: out.width,
        height: out.height,
        clip_ms: out.duration_ms,
        clip_bytes: bytes,
        took_ms: u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX),
        clip: Some(output.display().to_string()),
    }
}

/// The most of a clip the Settings window is sent to play (a 30 s clip is a few MB).
pub const PREVIEW_LIMIT: u64 = 100 * 1024 * 1024;

/// A clip's bytes, to play in the window. Refuses anything big, so a wrong file cannot flood it.
pub fn read_clip(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path)
        .map_err(|_| "There is no test clip yet: run a test recording first".to_string())?;
    if !meta.is_file() {
        return Err("There is no test clip yet: run a test recording first".into());
    }
    if meta.len() > limit {
        return Err(format!(
            "The clip is {:.0} MB, too large to play in this window. Open it in your video player instead.",
            meta.len() as f64 / (1024.0 * 1024.0)
        ));
    }
    std::fs::read(path).map_err(|e| format!("The clip could not be read: {e}"))
}

#[cfg(test)]
#[path = "video_tests.rs"]
mod tests;
