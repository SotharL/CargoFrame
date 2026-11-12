//! Writing the edited clip with ffmpeg.
//!
//! [`plan`] turns an [`Edit`] into the exact arguments; [`run`] starts ffmpeg with them and can
//! be cancelled or timed out; [`probe`] asks ffprobe how big and how long a recording is.
//!
//! * Every path is given to ffmpeg as `file:<path>`. Without the prefix ffmpeg would take a path
//!   such as `concat:…` or `http://…` (a mark picture that came from a window) for a protocol,
//!   and one that starts with `-` for an option.
//! * Marks are laid on the uncropped picture and the crop comes last, so changing the crop never
//!   moves a mark. Their times are on the output's own timeline once the start is cut.
//! * A crop that follows a moving window slides from place to place: its corner is a sum of
//!   ramps, one per move, which ffmpeg works out for each frame.
//! * Texts come last, on the picture as it is sent (the keys pressed, the time): one input of
//!   pictures each, each shown for its time (ffmpeg's concat reader, see [`keys_list`]).
//! * The result is H.264 in an MP4 with the index at the front (so it plays while it downloads).
//!   The sound of the recording, if it has one, is kept as AAC and cut with the picture.

use crate::edit::{resolve, Clip, Edit, Placed, Problem, Resolved};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub args: Vec<OsString>,
    /// How long the exported clip is.
    pub length_ms: u64,
    /// The picture size of the exported clip.
    pub size: (u32, u32),
    /// Marks left out because they only show in the part that was cut.
    pub dropped_overlays: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    #[error("A video tool (ffmpeg or ffprobe) was not found ({0}). Install ffmpeg, or set its location in the video settings")]
    NoEncoder(String),
    #[error("ffmpeg could not write the clip: {0}")]
    Failed(String),
    #[error("The export was cancelled")]
    Cancelled,
    #[error("The export took too long and was stopped")]
    TimedOut,
    #[error("The recording could not be read: {0}")]
    Unreadable(String),
}

fn secs(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

fn file_arg(p: &Path) -> OsString {
    let mut a = OsString::from("file:");
    a.push(p.as_os_str());
    a
}

/// The arguments that write `output` from `input` with the edit applied.
pub fn plan(input: &Path, output: &Path, edit: &Edit, clip: &Clip) -> Result<Plan, Vec<Problem>> {
    let r = resolve(edit, clip)?;
    Ok(plan_resolved(input, output, &r, clip))
}

fn plan_resolved(input: &Path, output: &Path, r: &Resolved, clip: &Clip) -> Plan {
    let mut args: Vec<OsString> = ["-hide_banner", "-nostdin", "-y", "-loglevel", "error"]
        .iter()
        .map(OsString::from)
        .collect();
    // Cutting before the input is frame-accurate because the clip is re-encoded anyway.
    args.extend(["-ss".into(), secs(r.start_ms).into()]);
    args.extend(["-t".into(), secs(r.length_ms()).into()]);
    args.extend(["-i".into(), file_arg(input)]);
    for o in &r.overlays {
        args.extend(["-i".into(), file_arg(&o.image)]);
    }
    for k in &r.layers {
        for a in ["-f", "concat", "-safe", "0", "-i"] {
            args.push(a.into());
        }
        args.push(file_arg(&k.list));
    }

    let mut graph = String::new();
    let mut last = String::from("0:v");
    for (i, o) in r.overlays.iter().enumerate() {
        let out = format!("o{i}");
        graph.push_str(&format!(
            "[{last}][{n}:v]overlay=x={x}:y={y}:format=auto:enable='between(t,{from},{to})'[{out}];",
            n = i + 1,
            x = o.x,
            y = o.y,
            from = secs(o.from_ms - r.start_ms),
            to = secs(o.to_ms - r.start_ms),
        ));
        last = out;
    }
    let cropped = if r.layers.is_empty() { "out" } else { "t0" };
    match r.crop {
        Some(c) if !r.follow.is_empty() => graph.push_str(&format!(
            "[{last}]crop=w={}:h={}:x='{}':y='{}'[{cropped}]",
            c.w,
            c.h,
            moving(&r.follow, r.start_ms, |p| p.x),
            moving(&r.follow, r.start_ms, |p| p.y),
        )),
        Some(c) => graph.push_str(&format!(
            "[{last}]crop={}:{}:{}:{}[{cropped}]",
            c.w, c.h, c.x, c.y
        )),
        // even sides, as yuv420 needs
        None => graph.push_str(&format!(
            "[{last}]crop=trunc(iw/2)*2:trunc(ih/2)*2:0:0[{cropped}]"
        )),
    }
    for (i, k) in r.layers.iter().enumerate() {
        let to = if i + 1 == r.layers.len() {
            "out".to_string()
        } else {
            format!("t{}", i + 1)
        };
        graph.push_str(&format!(
            ";[t{i}][{n}:v]overlay=x={x}:y={y}:format=auto:eof_action=pass[{to}]",
            n = r.overlays.len() + 1 + i,
            x = k.x,
            y = k.y,
        ));
    }
    args.extend(["-filter_complex".into(), graph.into()]);
    args.extend(["-map".into(), "[out]".into()]);
    if clip.has_audio {
        args.extend(["-map".into(), "0:a:0".into()]);
    } else {
        args.push("-an".into());
    }
    args.extend(["-t".into(), secs(r.length_ms()).into()]);
    for a in [
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "23",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
    ] {
        args.push(a.into());
    }
    if clip.has_audio {
        for a in ["-c:a", "aac", "-b:a", "128k"] {
            args.push(a.into());
        }
    }
    args.push(file_arg(output));

    Plan {
        args,
        length_ms: r.length_ms(),
        size: r.size(clip),
        dropped_overlays: r.dropped_overlays,
    }
}

/// One side of a moving crop's corner as an ffmpeg expression of the time `t` (seconds on the
/// sent clip, which starts at `start_ms`): the first place, plus a ramp for each move, so it
/// slides from each place to the next. ffmpeg keeps the crop inside the picture.
fn moving(places: &[Placed], start_ms: u64, side: impl Fn(&Placed) -> u32) -> String {
    let at = |p: &Placed| (p.at_ms as f64 - start_ms as f64) / 1000.0;
    let mut expr = side(&places[0]).to_string();
    for w in places.windows(2) {
        let step = i64::from(side(&w[1])) - i64::from(side(&w[0]));
        if step == 0 {
            continue;
        }
        let (a, b) = (at(&w[0]), at(&w[1]));
        if b - a > 0.0005 {
            expr.push_str(&format!("+({step})*clip((t-({a:.3}))/{:.3},0,1)", b - a));
        } else {
            expr.push_str(&format!("+({step})*gte(t,{b:.3})"));
        }
    }
    expr
}

/// The list ffmpeg's concat reader plays for the keys layer: from the start of the kept part to
/// its end, each caption picture for its time (`from_ms`, `to_ms` on the uncut clip), and the
/// `blank` one (the same size, see-through) between them.
pub fn keys_list(
    blank: &Path,
    shown: &[(PathBuf, u64, u64)],
    start_ms: u64,
    end_ms: u64,
) -> String {
    fn entry(out: &mut String, image: &Path, ms: u64) {
        // inside quotes only a quote needs care: close, write it escaped, open again
        let name = format!("file:{}", image.display()).replace('\'', "'\\''");
        out.push_str(&format!(
            "file '{name}'\nduration {:.3}\n",
            ms.max(1) as f64 / 1000.0
        ));
    }
    let mut out = String::from("ffconcat version 1.0\n");
    let mut now = start_ms;
    let mut last = blank.to_path_buf();
    for (image, from, to) in shown {
        let (from, to) = ((*from).max(now), (*to).min(end_ms));
        if to <= from {
            continue;
        }
        if from > now {
            entry(&mut out, blank, from - now);
        }
        entry(&mut out, image, to - from);
        now = to;
        last = image.clone();
    }
    if end_ms > now {
        entry(&mut out, blank, end_ms - now);
        last = blank.to_path_buf();
    }
    // the reader needs the last picture once more to give the one before its length
    let name = format!("file:{}", last.display()).replace('\'', "'\\''");
    out.push_str(&format!("file '{name}'\n"));
    out
}

/// Run ffmpeg. Stops (and kills it) when `cancelled` turns true or `timeout` passes.
pub fn run(
    ffmpeg: &Path,
    plan: &Plan,
    cancelled: &dyn Fn() -> bool,
    timeout: Duration,
) -> Result<(), ExportError> {
    run_command(ffmpeg, &plan.args, cancelled, timeout).map(|_| ())
}

fn run_command(
    program: &Path,
    args: &[OsString],
    cancelled: &dyn Fn() -> bool,
    timeout: Duration,
) -> Result<String, ExportError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = quiet(&mut command)
        .spawn()
        .map_err(|e| ExportError::NoEncoder(format!("{}: {e}", program.display())))?;
    // Both pipes are drained as the program runs, so a chatty one cannot fill them and stop.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut p) = pipe {
                let mut bytes = Vec::new();
                let _ = p.read_to_end(&mut bytes);
                text = String::from_utf8_lossy(&bytes).into_owned();
            }
            text
        })
    };
    let out = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(ExportError::Failed(e.to_string())),
        }
        if cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ExportError::Cancelled);
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ExportError::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if status.success() {
        Ok(stdout)
    } else {
        Err(ExportError::Failed(last_line(&stderr)))
    }
}

/// The last useful line of what ffmpeg said, cut short.
fn last_line(stderr: &str) -> String {
    let line = stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("it stopped without saying why");
    if line.chars().count() > 300 {
        format!("{}…", line.chars().take(300).collect::<String>())
    } else {
        line.to_string()
    }
}

/// Keep a console window from flashing up when CargoFrame (which has no console of its own)
/// starts ffmpeg on Windows. Nothing on other systems.
pub fn quiet(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Find a program in `dirs` only.
pub fn locate_in(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let candidates = [name.to_string(), format!("{name}.exe")];
    dirs.iter()
        .flat_map(|d| candidates.iter().map(move |c| d.join(c)))
        .find(|p| p.is_file())
}

/// Find a program in `dirs` first (a copy shipped next to CargoFrame), then in the search path.
pub fn locate(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let all: Vec<PathBuf> = dirs.iter().cloned().chain(path_dirs).collect();
    locate_in(name, &all)
}

/// Run a tool for the text it prints (for example `ffmpeg -encoders`). Gives up after 20 s.
pub fn ask(program: &Path, args: &[&str]) -> Result<String, ExportError> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    run_command(program, &args, &|| false, Duration::from_secs(20))
}

/// Read size and length out of ffprobe's JSON.
pub fn parse_probe(json: &str) -> Result<Clip, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "ffprobe answered with something unreadable")?;
    let streams = v
        .get("streams")
        .and_then(serde_json::Value::as_array)
        .ok_or("the file has no picture")?;
    fn kind(s: &serde_json::Value) -> Option<&str> {
        s.get("codec_type").and_then(serde_json::Value::as_str)
    }
    // the picture is the first video stream (or, from a probe that does not say, the first)
    let stream = streams
        .iter()
        .find(|s| kind(s) == Some("video"))
        .or_else(|| streams.iter().find(|s| kind(s).is_none()))
        .ok_or("the file has no picture")?;
    let has_audio = streams.iter().any(|s| kind(s) == Some("audio"));
    let dim = |k: &str| {
        stream
            .get(k)
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
    };
    let (width, height) = (
        dim("width").ok_or("the picture has no width")?,
        dim("height").ok_or("the picture has no height")?,
    );
    let seconds = v
        .pointer("/format/duration")
        .and_then(|d| {
            d.as_str()
                .map(str::to_string)
                .or_else(|| d.as_f64().map(|f| f.to_string()))
        })
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .ok_or("the file has no length")?;
    Ok(Clip {
        width,
        height,
        duration_ms: (seconds * 1000.0).round() as u64,
        has_audio,
    })
}

/// How big and how long a recording is.
pub fn probe(ffprobe: &Path, file: &Path) -> Result<Clip, ExportError> {
    let args: Vec<OsString> = [
        "-v",
        "error",
        "-show_entries",
        "stream=codec_type,width,height:format=duration",
        "-of",
        "json",
    ]
    .iter()
    .map(OsString::from)
    .chain([file_arg(file)])
    .collect();
    let json = run_command(ffprobe, &args, &|| false, Duration::from_secs(30))?;
    parse_probe(&json).map_err(ExportError::Unreadable)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
