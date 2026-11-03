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
use crate::export::{self, locate, ExportError, Plan};
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
        }
    }

    fn fps(&self) -> u32 {
        self.fps.clamp(MIN_FPS, MAX_FPS)
    }
}

/// The whole ffmpeg command line for recording into `ring`.
pub fn args(cfg: &Config, ring: &Ring) -> Vec<OsString> {
    let fps = cfg.fps();
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
            if let Some((w, h)) = size {
                a.extend(["-video_size".into(), format!("{w}x{h}").into()]);
            }
            a.extend(["-i".into(), display.clone().into()]);
        }
        Capture::Gdigrab => {
            a.extend(["-f".into(), "gdigrab".into()]);
            a.extend(["-framerate".into(), fps.to_string().into()]);
            a.extend(["-i".into(), "desktop".into()]);
        }
        Capture::Ddagrab { output } => {
            a.extend(["-f".into(), "lavfi".into()]);
            a.extend([
                "-i".into(),
                format!("ddagrab=output_idx={output}:framerate={fps}").into(),
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
    a.extend(["-vf".into(), filters.join(",").into()]);

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
        })
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
pub fn find_tools(dirs: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    Some((locate("ffmpeg", dirs)?, locate("ffprobe", dirs)?))
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
