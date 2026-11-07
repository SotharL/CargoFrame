//! Starting and stopping the recorder.
//!
//! The recorder is ffmpeg, run as a child process: it captures the screen, encodes it and writes
//! the ring of segments ([`crate::segments`]). Nothing of the picture passes through CargoFrame.
//! This module builds its command line from the settings, watches it, stops it politely (so the
//! last second is saved) and joins what is left into a clip.
//!
//! Which capture and which encoder suit each machine is what spike S9 decides; the choices here
//! are the ones that spike measures, and `Encoder::Auto`/`Capture::default_for_this_system` are
//! only proposals until it is run.

use crate::buffer::Window;
use crate::export::{self, ExportError, Plan};
use crate::segments::{join, Ring, Segment};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MIN_FPS: u32 = 5;
pub const MAX_FPS: u32 = 60;
pub const DEFAULT_FPS: u32 = 30;
/// One hour.
pub const DEFAULT_MAX_RUN_SECS: u32 = 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capture {
    /// X11 (`:0`, or the display of a virtual screen). `size` is the part of the screen taken;
    /// `None` takes the whole screen, whatever its size.
    X11 {
        display: String,
        size: Option<(u32, u32)>,
    },
    /// Windows, the old way: the desktop through GDI.
    Gdigrab,
    /// Windows, Desktop Duplication: the picture stays on the graphics card until downloaded.
    Ddagrab { output: u32 },
}

/// A part of the desktop, in pixels (`x` and `y` may be negative on Windows, where a screen can
/// lie left of or above the first one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Area {
    /// The smallest recording worth making: anything tinier is a tray stub, not a window.
    pub const MIN_SIDE: u32 = 16;

    /// The same area with even sides (the video format needs them), or `None` if it is too small.
    pub fn even(self) -> Option<Self> {
        let (w, h) = (self.w & !1, self.h & !1);
        (w >= Self::MIN_SIDE && h >= Self::MIN_SIDE).then_some(Self { w, h, ..self })
    }
}

/// How a sound reaches ffmpeg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    /// PulseAudio, or PipeWire speaking its language: what most Linux desktops have.
    Pulse,
    /// DirectShow: the Windows sound devices ffmpeg can list.
    Dshow,
    /// What the computer plays, to be listened to by CargoFrame itself (`pcm::Feed`), because
    /// ffmpeg cannot on Windows. `program` limits it to the sound of one program. The one who
    /// starts the recording turns this into [`AudioBackend::Pcm`] before the recorder starts; a
    /// recorder given this as it is leaves it out.
    Loopback { program: Option<u32> },
    /// Raw samples from a `pcm::Feed` waiting on `port` of this computer.
    Pcm { port: u16, rate: u32, channels: u16 },
}

/// One sound that goes into the recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSource {
    pub backend: AudioBackend,
    /// The device, as the backend names it. For Pulse, `@DEFAULT_MONITOR@` is whatever the
    /// computer is playing and `default` is the default microphone.
    pub device: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encoder {
    /// Software H.264: works everywhere, costs the most CPU.
    X264 {
        preset: X264Preset,
    },
    Nvenc,
    Qsv,
    Amf,
    /// Windows' own encoder.
    MediaFoundation,
    Vaapi {
        device: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X264Preset {
    Ultrafast,
    Veryfast,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub ffmpeg: PathBuf,
    pub capture: Capture,
    pub encoder: Encoder,
    pub fps: u32,
    pub window: Window,
    /// An empty folder used for nothing else.
    pub dir: PathBuf,
    /// The most disk the ring may fill.
    pub max_bytes: usize,
    /// Larger pictures are scaled down to this height before encoding.
    pub max_height: Option<u32>,
    /// ffmpeg ends the recording by itself after this long. A backstop: if CargoFrame is killed
    /// in the middle of a recording nobody is left to delete old pieces, and this keeps the
    /// leftovers from growing for ever.
    pub max_run_secs: u32,
    /// Record only this part of the desktop (a window), not all of what the capture sees.
    pub area: Option<Area>,
    /// The sounds to record: none, one, or two (mixed together).
    pub audio: Vec<AudioSource>,
}

impl Config {
    /// The sounds the recorder can take as inputs now: a loopback still to be set up is not one.
    fn sounds(&self) -> impl Iterator<Item = &AudioSource> {
        self.audio
            .iter()
            .filter(|s| !matches!(s.backend, AudioBackend::Loopback { .. }))
            .take(2)
    }
}

impl Config {
    pub fn new(ffmpeg: PathBuf, capture: Capture, dir: PathBuf) -> Self {
        Self {
            ffmpeg,
            capture,
            encoder: Encoder::X264 {
                preset: X264Preset::Ultrafast,
            },
            fps: DEFAULT_FPS,
            window: Window::default(),
            dir,
            max_bytes: 400 * 1024 * 1024,
            max_height: Some(1080),
            max_run_secs: DEFAULT_MAX_RUN_SECS,
            area: None,
            audio: Vec::new(),
        }
    }

    fn fps(&self) -> u32 {
        self.fps.clamp(MIN_FPS, MAX_FPS)
    }
}

/// The whole ffmpeg command line for recording into `ring`.
pub fn args(cfg: &Config, ring: &Ring) -> Vec<OsString> {
    let fps = cfg.fps();
    // a part too small to be a window is not a part: the whole picture is recorded
    let area = cfg.area.and_then(Area::even);
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-y"]
        .iter()
        .map(OsString::from)
        .collect();

    // what the encoder needs before the input
    if let Encoder::Vaapi { device } = &cfg.encoder {
        a.extend(["-vaapi_device".into(), device.clone().into_os_string()]);
    }

    // the capture, and the filters that turn it into a picture the encoder takes
    let mut filters: Vec<String> = Vec::new();
    match &cfg.capture {
        Capture::X11 { display, size } => {
            a.extend(["-f".into(), "x11grab".into()]);
            a.extend(["-framerate".into(), fps.to_string().into()]);
            // the root window has no negative places: a window half off the screen starts at 0
            let mut input = display.clone();
            if let Some(r) = &area {
                a.extend(["-video_size".into(), format!("{}x{}", r.w, r.h).into()]);
                input = format!("{display}+{},{}", r.x.max(0), r.y.max(0));
            } else if let Some((w, h)) = size {
                a.extend(["-video_size".into(), format!("{w}x{h}").into()]);
            }
            a.extend(["-i".into(), input.into()]);
        }
        Capture::Gdigrab => {
            a.extend(["-f".into(), "gdigrab".into()]);
            a.extend(["-framerate".into(), fps.to_string().into()]);
            if let Some(r) = &area {
                a.extend(["-offset_x".into(), r.x.to_string().into()]);
                a.extend(["-offset_y".into(), r.y.to_string().into()]);
                a.extend(["-video_size".into(), format!("{}x{}", r.w, r.h).into()]);
            }
            a.extend(["-i".into(), "desktop".into()]);
        }
        Capture::Ddagrab { output } => {
            a.extend(["-f".into(), "lavfi".into()]);
            // offsets are inside the screen the picture is taken from
            let part = area
                .map(|r| {
                    format!(
                        ":offset_x={}:offset_y={}:video_size={}x{}",
                        r.x.max(0),
                        r.y.max(0),
                        r.w,
                        r.h
                    )
                })
                .unwrap_or_default();
            a.extend([
                "-i".into(),
                format!("ddagrab=output_idx={output}:framerate={fps}{part}").into(),
            ]);
            filters.push("hwdownload".into());
            filters.push("format=bgra".into());
        }
    }
    if let Some(h) = cfg.max_height {
        // never enlarge; keep the width even (-2) and the aspect ratio
        filters.push(format!("scale=-2:min(ih\\,{h})"));
    }
    filters.push(
        if matches!(cfg.encoder, Encoder::Vaapi { .. }) {
            "format=nv12,hwupload"
        } else {
            "format=yuv420p"
        }
        .into(),
    );
    // the sounds: each is one more input, after the picture
    let sounds = cfg.sounds().count();
    for source in cfg.sounds() {
        a.extend(["-thread_queue_size".into(), "1024".into()]);
        match source.backend {
            AudioBackend::Loopback { .. } => {}
            AudioBackend::Pcm {
                port,
                rate,
                channels,
            } => {
                a.extend([
                    "-f".into(),
                    "f32le".into(),
                    "-ar".into(),
                    rate.to_string().into(),
                    "-ac".into(),
                    channels.to_string().into(),
                    "-i".into(),
                    format!("tcp://127.0.0.1:{port}").into(),
                ]);
            }
            AudioBackend::Pulse => {
                a.extend([
                    "-f".into(),
                    "pulse".into(),
                    "-i".into(),
                    source.device.clone().into(),
                ]);
            }
            AudioBackend::Dshow => {
                a.extend([
                    "-f".into(),
                    "dshow".into(),
                    "-i".into(),
                    format!("audio={}", source.device).into(),
                ]);
            }
        }
    }
    if sounds == 2 {
        // two sounds are mixed into one (the mix is half as loud as each, so it is raised)
        let graph = format!(
            "[0:v]{}[v];[1:a][2:a]amix=inputs=2:duration=longest,volume=2[a]",
            filters.join(",")
        );
        a.extend(["-filter_complex".into(), graph.into()]);
        a.extend(["-map".into(), "[v]".into(), "-map".into(), "[a]".into()]);
    } else {
        a.extend(["-vf".into(), filters.join(",").into()]);
        if sounds == 1 {
            a.extend(["-map".into(), "0:v".into(), "-map".into(), "1:a".into()]);
        }
    }

    // a keyframe every second, so every segment can start on one
    a.extend(["-g".into(), fps.to_string().into()]);
    let enc: &[&str] = match &cfg.encoder {
        Encoder::X264 { preset } => match preset {
            X264Preset::Ultrafast => &["-c:v", "libx264", "-preset", "ultrafast", "-crf", "28"],
            X264Preset::Veryfast => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "23"],
        },
        Encoder::Nvenc => &["-c:v", "h264_nvenc", "-preset", "p1", "-cq", "28"],
        Encoder::Qsv => &["-c:v", "h264_qsv", "-global_quality", "28"],
        Encoder::Amf => &["-c:v", "h264_amf", "-quality", "speed"],
        Encoder::MediaFoundation => &["-c:v", "h264_mf"],
        Encoder::Vaapi { .. } => &["-c:v", "h264_vaapi", "-qp", "24"],
    };
    a.extend(enc.iter().map(OsString::from));
    if sounds > 0 {
        for x in ["-c:a", "aac", "-b:a", "128k", "-ac", "2", "-ar", "48000"] {
            a.push(x.into());
        }
    }
    a.extend(["-t".into(), cfg.max_run_secs.max(1).to_string().into()]);
    a.extend(ring.output_args(1));
    a
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    #[error("{0}")]
    Export(#[from] ExportError),
    #[error("The recorder stopped by itself: {0}")]
    Died(String),
    #[error("Nothing was recorded")]
    NothingRecorded,
    #[error("The recording could not be joined: {0}")]
    Join(String),
}

/// What a running recording looks like from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// How much of the end of the recording is kept right now.
    pub buffered_ms: u64,
    /// Did the disk budget, not the window, cut the start?
    pub squeezed: bool,
}

pub struct Recorder {
    child: Child,
    stdin: Option<ChildStdin>,
    ring: Ring,
    said: Arc<Mutex<String>>,
    started: Instant,
    /// Things that feed ffmpeg and are stopped with it (the sounds CargoFrame listens to).
    feeds: Vec<Box<dyn Send>>,
}

impl Recorder {
    pub fn start(cfg: &Config) -> Result<Self, RecordError> {
        let ring = Ring::new(&cfg.dir, cfg.window, cfg.max_bytes);
        let mut command = Command::new(&cfg.ffmpeg);
        command
            .args(args(cfg, &ring))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = export::quiet(&mut command)
            .spawn()
            .map_err(|e| ExportError::NoEncoder(format!("{}: {e}", cfg.ffmpeg.display())))?;
        let said = Arc::new(Mutex::new(String::new()));
        if let Some(mut err) = child.stderr.take() {
            let said = Arc::clone(&said);
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = err.read_to_end(&mut buf);
                *said.lock().unwrap_or_else(|e| e.into_inner()) =
                    String::from_utf8_lossy(&buf).into_owned();
            });
        }
        let stdin = child.stdin.take();
        Ok(Self {
            child,
            stdin,
            ring,
            said,
            started: Instant::now(),
            feeds: Vec::new(),
        })
    }

    /// Keep `feed` for as long as the recorder runs. It is let go when the recording stops, is
    /// cancelled, or the recorder is dropped.
    pub fn keep(&mut self, feed: Box<dyn Send>) {
        self.feeds.push(feed);
    }

    /// Wait up to `wait` to see whether ffmpeg gives up at once (a sound device that is not
    /// there, a screen that cannot be opened). `Some(what it said)` if it did; `None` if it is
    /// still running, which is the usual case.
    pub fn early_failure(&mut self, wait: Duration) -> Option<String> {
        let until = Instant::now() + wait;
        loop {
            if let Ok(Some(_)) = self.child.try_wait() {
                // give the reader of its messages a moment to finish
                std::thread::sleep(Duration::from_millis(50));
                return Some(self.last_message());
            }
            if Instant::now() >= until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Take in the segments finished so far. An error means the recorder is gone.
    pub fn poll(&mut self) -> Result<Progress, RecordError> {
        if let Ok(Some(status)) = self.child.try_wait() {
            if !status.success() {
                // give the reader of its messages a moment to finish
                std::thread::sleep(Duration::from_millis(50));
                return Err(RecordError::Died(self.last_message()));
            }
        }
        self.ring.poll();
        Ok(Progress {
            buffered_ms: self.ring.span_ms(),
            squeezed: self.ring.was_squeezed(),
        })
    }

    fn last_message(&self) -> String {
        let said = self.said.lock().unwrap_or_else(|e| e.into_inner());
        said.lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("it gave no reason")
            .chars()
            .take(300)
            .collect()
    }

    /// How long the recorder has been running.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Stop, saving the last second, and hand back the segments of the clip. ffmpeg is asked to
    /// finish (it reads `q`); if it has not by `patience`, it is stopped.
    pub fn stop(mut self, patience: Duration) -> Result<Vec<Segment>, RecordError> {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }
        let waited = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if waited.elapsed() < patience => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
            }
        }
        let segments = self.ring.finish();
        if segments.is_empty() {
            Err(RecordError::NothingRecorded)
        } else {
            Ok(segments)
        }
    }

    /// Stop and throw the recording away.
    pub fn cancel(mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.ring.clear();
    }
}

impl Drop for Recorder {
    /// A recorder that is let go while running is stopped: ffmpeg must never be left capturing
    /// the screen with nobody to look after the pieces it writes.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.ring.clear();
        }
    }
}

/// Join the segments into one MP4 at `output`, using `work` for the list file. The segments are
/// deleted when they are dropped, so keep them until this returns.
pub fn assemble(
    ffmpeg: &Path,
    segments: &[Segment],
    work: &Path,
    output: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), RecordError> {
    let list_file = work.join("join.txt");
    let (list, args) =
        join(segments, &list_file, output).map_err(|e| RecordError::Join(e.to_string()))?;
    std::fs::write(&list_file, list).map_err(|e| RecordError::Join(e.to_string()))?;
    let plan = Plan {
        args,
        length_ms: 0,
        size: (0, 0),
        dropped_overlays: 0,
    };
    let result = export::run(ffmpeg, &plan, cancelled, Duration::from_secs(60));
    let _ = std::fs::remove_file(&list_file);
    result.map_err(RecordError::from)
}

/// ffmpeg and ffprobe, looked for in `dirs` (a copy next to the app) and then the search path.
/// (The program itself finds them through the Video settings; this is for the tests.)
#[cfg(test)]
pub fn find_tools(dirs: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    use crate::export::locate;
    Some((locate("ffmpeg", dirs)?, locate("ffprobe", dirs)?))
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
