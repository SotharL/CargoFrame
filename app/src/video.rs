//! Settings, Video: finding ffmpeg, choosing how the screen is captured and compressed, and a
//! short test recording the person can run on their own PC.
//!
//! Which capture and which encoder work best is something only a real machine can say (spike
//! S9), so nothing here guesses beyond a safe default: "Auto" is software compression of the old
//! GDI capture on Windows and x11grab on an X11 desktop, and the test recording tells the person
//! at once whether another choice works on their PC.

use cargoframe_config::{Video, VideoCapture, VideoEncoder};
use cargoframe_layout::{BlockKind, DesktopSound, Layout, VideoAudio, VideoSubject};
use cargoframe_video::pcm::Feed;
use cargoframe_video::recorder::{
    self, Area, AudioBackend, AudioSource, Capture, Config, Encoder, Recorder, X264Preset,
};
use cargoframe_video::{ask, locate_in, plan, probe, run, Crop, Edit, Overlay, Window};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a recording with sound waits to hear that ffmpeg could not open a sound device. With
/// a sound system running, a device that is not there is refused at once; a sound system that is
/// not running is looked for beforehand (see `audio::pulse_running`), because that takes seconds
/// to be refused.
const SOUND_PATIENCE: Duration = Duration::from_millis(600);

/// How long the test recording runs.
pub const TRIAL_SECONDS: u64 = 5;
/// The name of the test clip, kept in `video-test/` until the next test replaces it.
pub const TRIAL_CLIP: &str = "test-clip.mp4";

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

    pub(crate) fn name(self) -> &'static str {
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
            wayland: crate::desktop_keys::wayland(),
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
    /// GStreamer with its PipeWire plugin, which reads a Wayland desktop's screen.
    pub gstreamer: Option<PathBuf>,
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

pub fn program_beside_app() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
}

/// Find ffmpeg and ffprobe and ask ffmpeg what it can do.
pub fn find(setting: &Video, beside_app: Option<&Path>) -> Result<Found, String> {
    let (mut dirs, chosen) = search_dirs(setting.ffmpeg.as_deref(), beside_app);
    if !chosen {
        // where winget leaves ffmpeg: a program that was already running does not have it on its
        // search path yet
        dirs.extend(crate::setup::winget_links());
    }
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
    let mut captures = parse_captures(&devices, &filters);
    let gstreamer = wayland_reader();
    if gstreamer.is_some() {
        captures.push(VideoCapture::Wayland);
    }
    Ok(Found {
        ffmpeg,
        ffprobe,
        version: parse_version(&version),
        encoders: parse_encoders(&encoders),
        captures,
        gstreamer,
    })
}

/// `gst-launch-1.0`, when GStreamer can read PipeWire here (`pipewiresrc`).
pub fn wayland_reader() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let launch = cargoframe_capture::wayland::find_program("gst-launch-1.0")?;
        let inspect = launch.with_file_name("gst-inspect-1.0");
        let has_plugin = std::process::Command::new(inspect)
            .args(["--exists", "pipewiresrc"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        has_plugin.then_some(launch)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// What to install to record a Wayland desktop.
pub const WAYLAND_RECORDING_NEEDS: &str = "Recording a Wayland desktop needs GStreamer with its PipeWire plugin: install gstreamer1.0-tools and gstreamer1.0-pipewire (Ubuntu, Debian), gstreamer1-plugins-base-tools and pipewire-gstreamer (Fedora), or gstreamer and gst-plugin-pipewire (Arch).";

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
                    if !found.captures.contains(&VideoCapture::Wayland) {
                        return Err(WAYLAND_RECORDING_NEEDS.into());
                    }
                    return Ok(Choice {
                        capture: VideoCapture::Wayland,
                        encoder: choose_encoder(setting, found)?,
                    });
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
                return Err("This desktop runs Wayland: X11 capture would only see old-style windows. Choose Automatic or Wayland screen sharing in Settings, Recording.".into());
            }
            if explicit == VideoCapture::Wayland && !session.wayland {
                return Err("Wayland screen sharing only works in a Wayland session.".into());
            }
            explicit
        }
    };
    Ok(Choice {
        capture,
        encoder: choose_encoder(setting, found)?,
    })
}

fn choose_encoder(setting: &Video, found: &Found) -> Result<VideoEncoder, String> {
    Ok(match setting.encoder {
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
    })
}

pub fn capture_name(c: VideoCapture) -> &'static str {
    match c {
        VideoCapture::Auto => "automatic",
        VideoCapture::Gdigrab => "GDI",
        VideoCapture::Ddagrab => "Desktop Duplication",
        VideoCapture::X11 => "X11",
        VideoCapture::Wayland => "Wayland screen sharing",
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

/// What a layout's video block asks of one recording, as it is when the shortcut is pressed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wish {
    /// Record only this part of the desktop (the window in focus). `None` is the whole screen.
    pub area: Option<Area>,
    /// The sounds, in the layout's words. `None` records no sound (a test recording has none).
    pub audio: Option<VideoAudio>,
    /// The program in focus (its process id), when the layout asks for that program's sound.
    pub program: Option<u32>,
}

/// The window that has the focus when a shortcut is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focus {
    pub area: Area,
    /// The program it belongs to, when the system says.
    pub program: Option<u32>,
}

/// What the layout a video trigger opens asks the recording to be. `focused` says which window
/// is in focus, and is only asked when the layout needs it (the window to record, or the program
/// whose sound to record). The second part is what the person should be told about anything that
/// could not be done.
pub fn wish_for(
    layout: Option<&Layout>,
    focused: &dyn Fn() -> Option<Focus>,
) -> (Wish, Vec<String>) {
    let block = layout
        .and_then(|l| {
            l.blocks.iter().find_map(|b| match &b.kind {
                BlockKind::Video(v) => Some(v.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let mut notes = Vec::new();
    let wants_program = block.audio.desktop == cargoframe_layout::AudioChoice::Default
        && block.audio.desktop_from == DesktopSound::FocusedProgram;
    let wants_window = block.subject == VideoSubject::FocusedWindow;
    let focus = (wants_window || wants_program).then(focused).flatten();
    let area = match block.subject {
        VideoSubject::Screen => None,
        VideoSubject::FocusedWindow => {
            match focus.map(|f| f.area) {
                Some(a) if a.even().is_some() => Some(a),
                Some(_) => {
                    notes.push("The window in focus is too small to record, so the whole screen is recorded.".into());
                    None
                }
                None => {
                    notes.push("CargoFrame could not tell which window has the focus, so the whole screen is recorded.".into());
                    None
                }
            }
        }
    };
    (
        Wish {
            area,
            audio: Some(block.audio),
            program: focus.and_then(|f| f.program).filter(|_| wants_program),
        },
        notes,
    )
}

/// A wish after the computer has been asked what it has.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub area: Option<Area>,
    pub sources: Vec<AudioSource>,
    /// How long to wait to hear that ffmpeg could not open a sound device.
    pub patience: Duration,
    /// How a sound CargoFrame listens to itself is opened. `None` is the real one (the Windows
    /// sound system); a test gives a stand-in.
    pub listen: Option<Listen>,
    /// The most it may run, in seconds; `None` is the recorder's own limit (an hour).
    pub max_run_secs: Option<u32>,
}

/// Open a sound CargoFrame listens to itself: the computer's (`None`), or one program's.
pub type Listen = fn(Option<u32>) -> Result<Feed, String>;

fn listen_to_the_computer(program: Option<u32>) -> Result<Feed, String> {
    Feed::open(move || cargoframe_video::loopback::open(program))
}

/// Make ffmpeg inputs of the sounds. The ones CargoFrame listens to itself are opened and come
/// back as their feeds, to be kept as long as the recording runs; one that cannot be opened is
/// left out and said, so the picture (and any other sound) still gets recorded.
fn open_sounds(
    sources: &[AudioSource],
    listen: Listen,
    notes: &mut Vec<String>,
) -> (Vec<AudioSource>, Vec<Feed>) {
    let mut inputs = Vec::new();
    let mut feeds = Vec::new();
    for source in sources {
        let AudioBackend::Loopback { program } = source.backend else {
            inputs.push(source.clone());
            continue;
        };
        match listen(program) {
            Ok(feed) => {
                let format = feed.format();
                inputs.push(AudioSource {
                    backend: AudioBackend::Pcm {
                        port: feed.port(),
                        rate: format.rate,
                        channels: format.channels,
                    },
                    device: source.device.clone(),
                });
                feeds.push(feed);
            }
            Err(why) => notes.push(format!(
                "The sound of {} could not be recorded ({why}), so this recording has none of it.",
                if program.is_some() {
                    "the program in focus"
                } else {
                    "the computer"
                }
            )),
        }
    }
    (inputs, feeds)
}

/// Where the desktop's "remember this screen" answer is kept, beside the recording's folder.
#[cfg(target_os = "linux")]
fn screen_sharing_token(work: &Path) -> PathBuf {
    work.parent().unwrap_or(work).join("screen-sharing.token")
}

/// Ask the desktop to share a screen (it may ask the person which, the first time) and start
/// GStreamer reading it. The window to record, if any, becomes a part of that screen.
#[cfg(target_os = "linux")]
fn open_wayland_picture(
    found: &Found,
    fps: u32,
    area: Option<Area>,
    work: &Path,
    notes: &mut Vec<String>,
) -> Result<
    (
        cargoframe_video::screenfeed::PictureFeed,
        Option<cargoframe_video::recorder::Part>,
    ),
    String,
> {
    use cargoframe_capture::screencast::ScreenCast;
    use cargoframe_capture::CaptureError;
    let gst = found
        .gstreamer
        .clone()
        .ok_or_else(|| WAYLAND_RECORDING_NEEDS.to_string())?;
    let token_file = screen_sharing_token(work);
    let token = std::fs::read_to_string(&token_file).ok();
    let cast = ScreenCast::start(token.as_deref().map(str::trim)).map_err(|e| match e {
        CaptureError::Refused => "The screen was not shared, so nothing was recorded.".to_string(),
        other => format!("The desktop did not share the screen: {other}"),
    })?;
    if let Some(t) = &cast.restore_token {
        let _ = std::fs::write(&token_file, t);
    }
    let part = match (area, cast.stream.position, cast.stream.size) {
        (Some(a), Some((x, y)), Some((w, h))) => {
            let part = cargoframe_video::recorder::Part::of(a, Area { x, y, w, h });
            if part.is_none() {
                notes.push("The window in focus is not on the shared screen, so the whole screen is recorded.".into());
            }
            part
        }
        (Some(_), _, _) => {
            notes.push("The desktop did not say where the shared screen is, so the whole screen is recorded.".into());
            None
        }
        _ => None,
    };
    let args = cargoframe_video::screenfeed::pipewire_args(cast.stream.node, fps);
    let feed = cargoframe_video::screenfeed::PictureFeed::open(gst, args, Box::new(cast))?;
    Ok((feed, part))
}

#[cfg(not(target_os = "linux"))]
fn open_wayland_picture(
    _found: &Found,
    _fps: u32,
    _area: Option<Area>,
    _work: &Path,
    _notes: &mut Vec<String>,
) -> Result<
    (
        cargoframe_video::screenfeed::PictureFeed,
        Option<cargoframe_video::recorder::Part>,
    ),
    String,
> {
    Err("Wayland screen sharing only works on Linux.".into())
}

/// How a recording began: whether it has sound, and what the person should be told about what
/// was left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Started {
    pub sound: bool,
    pub notes: Vec<String>,
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
        // the port is known once the screen-sharing stream is open (see `start_with_extras`)
        VideoCapture::Wayland => Capture::Feed {
            port: 0,
            part: None,
        },
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
    /// The clip has a sound track.
    pub clip_audio: bool,
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
            clip_audio: false,
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

/// The folder (inside the app's data folder) a recording started by a trigger works in. Settings,
/// Video has its own (`video-test`), so a test never touches a real recording.
pub fn work_dir(store: &cargoframe_config::Store) -> PathBuf {
    store.dirs().data().join("video-recording")
}

/// What a press of a video trigger does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing is recording: start.
    Start,
    /// A recording this trigger family started is running: stop it and open the window.
    Stop,
    /// A continuous recording is running: open the window with its last seconds, and go on.
    Snapshot,
    /// Something else is going on; this is what to tell the person.
    Refuse(&'static str),
    /// The recording ended by itself: say why (and the next press starts a new one).
    Report(String),
}

/// Decide what a press of a video trigger does, from where the recording stands.
pub fn step(view: &RecordingView, started_by_trigger: bool, continuous: bool) -> Step {
    match view {
        RecordingView::Idle => Step::Start,
        RecordingView::Failed { message } if started_by_trigger => Step::Report(message.clone()),
        RecordingView::Failed { .. } => Step::Start,
        RecordingView::Recording { .. } if started_by_trigger && continuous => Step::Snapshot,
        RecordingView::Recording { .. } if started_by_trigger => Step::Stop,
        RecordingView::Recording { .. } => {
            Step::Refuse("A test recording started from Settings is running. Stop it there first.")
        }
        RecordingView::Finishing => Step::Refuse("The last recording is still being finished."),
    }
}

/// The one recording that can run at a time: started, watched and stopped by commands.
pub struct Recording {
    slot: Mutex<Slot>,
    /// The running recording was started by a trigger: it is not stopped after ten minutes (the
    /// person stops it with the shortcut, and only the last seconds are kept anyway).
    by_trigger: std::sync::atomic::AtomicBool,
    /// The running recording goes on until a shortcut is held again: a press hands out its last
    /// seconds instead of stopping it.
    continuous: std::sync::atomic::AtomicBool,
    /// Counts the copies handed out, so each has a folder of its own.
    snapshots: std::sync::atomic::AtomicU64,
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
            by_trigger: std::sync::atomic::AtomicBool::new(false),
            continuous: std::sync::atomic::AtomicBool::new(false),
            snapshots: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Is a continuous recording running (one that a press does not stop)?
    pub fn is_continuous(&self) -> bool {
        self.continuous.load(std::sync::atomic::Ordering::SeqCst)
            && matches!(&*self.lock(), Slot::Running(_))
    }

    /// Like `start_for_trigger`, for a recording that goes on until a shortcut is held again.
    pub fn start_continuous(
        &self,
        video: &Video,
        os: Os,
        session: &Session,
        work: &Path,
        wish: &Wish,
    ) -> Result<Started, String> {
        let started = self.start_wanting_for(
            video,
            os,
            session,
            work,
            video.window_secs,
            wish,
            Some(Video::MAX_CONTINUOUS_SECS),
        )?;
        self.by_trigger
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.continuous
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(started)
    }

    /// The last seconds of the running recording as a clip of their own, while it goes on.
    pub fn snapshot(&self) -> Trial {
        let n = self
            .snapshots
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // copy the pieces under the lock (a few files), join them without it
        let taken = {
            let mut slot = self.lock();
            let Slot::Running(a) = &mut *slot else {
                return Trial::failed("No recording is running.", None);
            };
            let dir = a.work.join(format!("snapshot-{n}"));
            let _ = std::fs::remove_dir_all(&dir);
            a.recorder.snapshot(&dir).map(|segments| {
                (
                    segments,
                    dir,
                    a.found.clone(),
                    a.choice.clone(),
                    a.keep_secs,
                )
            })
        };
        let (segments, dir, found, choice, _keep) = match taken {
            Ok(t) => t,
            Err(e) => return Trial::failed(e.to_string(), None),
        };
        let clip = dir.join(TRIAL_CLIP);
        let joined = recorder::assemble(&found.ffmpeg, &segments, &dir, &clip, &|| false);
        drop(segments);
        if let Err(e) = joined {
            let _ = std::fs::remove_dir_all(&dir);
            return Trial::failed(e.to_string(), Some(&choice));
        }
        let info = match probe(&found.ffprobe, &clip) {
            Ok(i) => i,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Trial::failed(
                    format!("The clip could not be read back: {e}"),
                    Some(&choice),
                );
            }
        };
        Trial {
            ok: true,
            message: format!(
                "The last {:.1} s of the recording.",
                info.duration_ms as f64 / 1000.0
            ),
            capture: Some(choice.capture),
            encoder: Some(choice.encoder),
            width: info.width,
            height: info.height,
            clip_ms: info.duration_ms,
            clip_bytes: std::fs::metadata(&clip).map(|m| m.len()).unwrap_or(0),
            clip_audio: info.has_audio,
            took_ms: 0,
            clip: Some(clip.display().to_string()),
        }
    }

    /// Was the running recording started by a video trigger (and not from Settings)?
    pub fn started_by_trigger(&self) -> bool {
        self.by_trigger.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Like `start`, for a video trigger.
    pub fn start_for_trigger(
        &self,
        video: &Video,
        os: Os,
        session: &Session,
        work: &Path,
        keep_secs: u32,
        wish: &Wish,
    ) -> Result<Started, String> {
        let started = self.start_wanting(video, os, session, work, keep_secs, wish)?;
        self.by_trigger
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(started)
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
        self.start_wanting(video, os, session, work, keep_secs, &Wish::default())
            .map(|_| ())
    }

    /// `start`, with what a layout's video block asks for.
    pub fn start_wanting(
        &self,
        video: &Video,
        os: Os,
        session: &Session,
        work: &Path,
        keep_secs: u32,
        wish: &Wish,
    ) -> Result<Started, String> {
        self.start_wanting_for(video, os, session, work, keep_secs, wish, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn start_wanting_for(
        &self,
        video: &Video,
        os: Os,
        session: &Session,
        work: &Path,
        keep_secs: u32,
        wish: &Wish,
        max_run_secs: Option<u32>,
    ) -> Result<Started, String> {
        validate(video)?;
        // a recording started by hand has the ten-minute limit (see `view`)
        let idle = matches!(&*self.lock(), Slot::Idle { .. });
        if idle {
            self.by_trigger
                .store(false, std::sync::atomic::Ordering::SeqCst);
            self.continuous
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        // refuse early, before looking for ffmpeg, so the message is the right one
        match &*self.lock() {
            Slot::Idle { .. } => {}
            Slot::Running(_) => return Err("A recording is already running.".into()),
            Slot::Finishing => return Err("The last recording is still being finished.".into()),
        }
        let found = find(video, program_beside_app().as_deref())?;
        let choice = choose(video, &found, os, session)?;
        let mut notes = Vec::new();
        let sources = match &wish.audio {
            Some(audio) => {
                let plan = crate::audio::plan(
                    audio,
                    os,
                    crate::audio::pulse_running(),
                    wish.program,
                    &|| crate::audio::list(&found.ffmpeg, os),
                );
                notes.extend(plan.notes);
                plan.sources
            }
            None => Vec::new(),
        };
        let extras = Extras {
            area: wish.area,
            sources,
            patience: SOUND_PATIENCE,
            listen: None,
            max_run_secs,
        };
        let mut started =
            self.start_with_extras(found, choice, video, session, work, keep_secs, &extras)?;
        notes.append(&mut started.notes);
        started.notes = notes;
        Ok(started)
    }

    #[cfg(test)]
    fn start_with(
        &self,
        found: Found,
        choice: Choice,
        video: &Video,
        session: &Session,
        work: &Path,
        keep_secs: u32,
    ) -> Result<(), String> {
        self.start_with_extras(
            found,
            choice,
            video,
            session,
            work,
            keep_secs,
            &Extras::default(),
        )
        .map(|_| ())
    }

    #[allow(clippy::too_many_arguments)]
    fn start_with_extras(
        &self,
        found: Found,
        choice: Choice,
        video: &Video,
        session: &Session,
        work: &Path,
        keep_secs: u32,
        extras: &Extras,
    ) -> Result<Started, String> {
        let mut choice = choice;
        // A part of the desktop is cut out by the capture itself. GDI takes it from anywhere on
        // the desktop, on any screen; the graphics-card capture only sees one screen.
        if extras.area.is_some()
            && choice.capture == VideoCapture::Ddagrab
            && found.captures.contains(&VideoCapture::Gdigrab)
        {
            choice.capture = VideoCapture::Gdigrab;
        }
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
        cfg.area = extras.area;
        if let Some(secs) = extras.max_run_secs {
            cfg.max_run_secs = secs;
        }
        let mut notes = Vec::new();
        let (inputs, feeds) = open_sounds(
            &extras.sources,
            extras.listen.unwrap_or(listen_to_the_computer),
            &mut notes,
        );
        cfg.audio = inputs;
        // Wayland: the desktop shares the screen as a stream, which GStreamer reads for ffmpeg.
        let picture = if choice.capture == VideoCapture::Wayland {
            let (feed, part) =
                open_wayland_picture(&found, cfg.fps, extras.area, work, &mut notes)?;
            cfg.capture = Capture::Feed {
                port: feed.port(),
                part,
            };
            cfg.area = None;
            Some(feed)
        } else {
            None
        };
        let mut recorder = Recorder::start(&cfg).map_err(|e| e.to_string())?;
        for feed in feeds {
            recorder.keep(Box::new(feed));
        }
        if let Some(p) = picture {
            recorder.keep(Box::new(p));
        }
        if !cfg.audio.is_empty() {
            // A sound device that is not there ends ffmpeg at once. The picture matters more than
            // the sound, so go on without it, and say so.
            if let Some(why) = recorder.early_failure(extras.patience) {
                recorder.cancel();
                let _ = std::fs::remove_dir_all(&ring);
                let _ = std::fs::create_dir_all(&ring);
                notes.push(format!(
                    "The sound could not be recorded, so this recording has none ({why})."
                ));
                cfg.audio.clear();
                recorder = Recorder::start(&cfg).map_err(|e| e.to_string())?;
            }
        }
        let sound = !cfg.audio.is_empty();
        *slot = Slot::Running(Box::new(Active {
            recorder,
            found,
            choice,
            work: work.to_path_buf(),
            keep_secs: cfg.window.as_secs(),
            began: Instant::now(),
        }));
        Ok(Started { sound, notes })
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
            Ok(p) if elapsed <= MANUAL_LIMIT || self.started_by_trigger() => {
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
        clip_audio: info.has_audio,
        took_ms,
        clip: Some(clip.display().to_string()),
    }
}

/// What the person did to a recording in the CargoFrame window: kept from `start_ms` to `end_ms`
/// (`None` is to the end of the recording), and perhaps cropped to an area. Times are on the
/// recording's own timeline, the crop is in the recording's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ClipEdit {
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    #[serde(default)]
    pub crop: Option<Crop>,
    /// A mark was drawn: lay the picture the window uploaded (see `set_clip_mark`) over the
    /// kept part, on the uncropped picture.
    #[serde(default)]
    pub mark: bool,
}

impl ClipEdit {
    /// Nothing is cut and nothing is cropped: the recording is sent as it was recorded.
    pub fn changes_nothing(&self, duration_ms: u64) -> bool {
        self.crop.is_none()
            && !self.mark
            && self.start_ms == 0
            && match self.end_ms {
                None => true,
                Some(end) => end >= duration_ms,
            }
    }
}

/// Make `input` into what `edit` keeps and write it to `output` (replacing what is there).
/// Returns the new clip's length, size and weight. Used when a video capture is sent or saved.
pub fn edit_clip(
    video: &Video,
    input: &Path,
    output: &Path,
    edit: ClipEdit,
    mark: Option<&Path>,
) -> Result<Trial, String> {
    let found = find(video, program_beside_app().as_deref())?;
    let info = probe(&found.ffprobe, input)
        .map_err(|e| format!("The recording could not be read: {e}"))?;
    // The encoder wants an even picture: round the crop to even numbers, inside the picture.
    let crop = edit.crop.map(|c| {
        let even = |n: u32| n & !1;
        let (x, y) = (even(c.x), even(c.y));
        Crop {
            x,
            y,
            w: even(c.w).min(info.width.saturating_sub(x) & !1),
            h: even(c.h).min(info.height.saturating_sub(y) & !1),
        }
    });
    // the mark is drawn the size of the picture, so it goes on the corner and shows all along
    let overlays = mark
        .map(|image| Overlay {
            image: image.to_path_buf(),
            x: 0,
            y: 0,
            from_ms: edit.start_ms,
            to_ms: edit.end_ms.unwrap_or(info.duration_ms),
        })
        .into_iter()
        .collect();
    let edit = Edit {
        start_ms: edit.start_ms,
        end_ms: edit.end_ms,
        crop,
        overlays,
    };
    let planned = plan(input, output, &edit, &info).map_err(|problems| {
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(". ")
    })?;
    let began = Instant::now();
    let _ = std::fs::remove_file(output);
    run(&found.ffmpeg, &planned, &|| false, Duration::from_secs(120))
        .map_err(|e| format!("The recording could not be edited: {e}"))?;
    let out = probe(&found.ffprobe, output)
        .map_err(|e| format!("The edited recording could not be read back: {e}"))?;
    let bytes = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    Ok(Trial {
        ok: true,
        message: String::new(),
        capture: None,
        encoder: None,
        width: out.width,
        height: out.height,
        clip_ms: out.duration_ms,
        clip_bytes: bytes,
        clip_audio: out.has_audio,
        took_ms: u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX),
        clip: Some(output.display().to_string()),
    })
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
