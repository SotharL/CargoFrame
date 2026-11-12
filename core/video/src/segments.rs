//! The recording as a ring of small files on disk.
//!
//! The recorder (ffmpeg, started with [`Ring::output_args`]) captures, encodes, and writes the
//! picture in one-second MPEG-TS segments, adding a line to a list file as each one is finished.
//! [`Ring::poll`] reads the new lines and hands the segments to a [`RollingBuffer`], which drops
//! the oldest once they are older than the window; a dropped segment deletes its own file. So a
//! recording can run for an hour and use a few megabytes of disk and none of CargoFrame's memory.
//!
//! When the recording stops, [`Ring::finish`] returns the segments left, and [`join`] says how to
//! stitch them into one MP4 without re-encoding (a stream copy, so it takes well under a second).

use crate::buffer::{RollingBuffer, Window};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// One finished piece of the recording. Its file is deleted when it is dropped.
#[derive(Debug)]
pub struct Segment {
    pub path: PathBuf,
    pub bytes: u64,
    /// How long it is, as ffmpeg reported.
    pub length_ms: u64,
}

impl Drop for Segment {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub struct Ring {
    dir: PathBuf,
    list: PathBuf,
    buffer: RollingBuffer<Segment>,
    /// How many bytes of the list file have been read (only whole lines are).
    read: usize,
}

/// What a line of ffmpeg's segment list says: `seg00003.ts,3.000000,4.001000`.
#[derive(Debug, Clone, PartialEq)]
struct Entry<'a> {
    name: &'a str,
    start: f64,
    end: f64,
}

fn parse_line(line: &str) -> Option<Entry<'_>> {
    let mut parts = line.trim().split(',');
    let name = parts.next()?;
    let (start, end) = (
        parts.next()?.parse::<f64>().ok()?,
        parts.next()?.parse::<f64>().ok()?,
    );
    // A bare file name only: the list is written by ffmpeg, but a name that climbs out of the
    // folder must never be opened or deleted.
    let plain = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
        && name.ends_with(".ts");
    (plain && start.is_finite() && end.is_finite() && end > start && start >= 0.0)
        .then_some(Entry { name, start, end })
}

impl Ring {
    /// `dir` must exist and be used for nothing else. `max_bytes` caps the disk it may fill.
    pub fn new(dir: &Path, window: Window, max_bytes: usize) -> Self {
        Self {
            dir: dir.to_path_buf(),
            list: dir.join("segments.csv"),
            buffer: RollingBuffer::new(window, max_bytes),
            read: 0,
        }
    }

    /// The part of the ffmpeg command line that writes the ring. Put it after the capture and
    /// encoder options (which must produce a keyframe at least every `segment_seconds`, for
    /// example `-g <fps*segment_seconds>`: a segment can only start on a keyframe).
    pub fn output_args(&self, segment_seconds: u32) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "-f".into(),
            "segment".into(),
            "-segment_time".into(),
            segment_seconds.max(1).to_string().into(),
            "-segment_format".into(),
            "mpegts".into(),
            "-reset_timestamps".into(),
            "1".into(),
            "-segment_list_type".into(),
            "csv".into(),
            "-segment_list".into(),
        ];
        args.push(file_arg(&self.list));
        args.push(file_arg(&self.dir.join("seg%05d.ts")));
        args
    }

    /// Read what the recorder has finished since the last call. Returns how many segments
    /// were added.
    pub fn poll(&mut self) -> usize {
        let Ok(text) = fs::read_to_string(&self.list) else {
            return 0;
        };
        let Some(unread) = text.get(self.read..) else {
            return 0;
        };
        // only whole lines: the recorder may be in the middle of writing the last one
        let whole = unread.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let mut added = 0;
        for line in unread[..whole].lines() {
            let Some(e) = parse_line(line) else { continue };
            let path = self.dir.join(e.name);
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            let end_ms = (e.end * 1000.0).round() as u64;
            let seg = Segment {
                path,
                bytes: meta.len(),
                length_ms: ((e.end - e.start) * 1000.0).round() as u64,
            };
            let weight = usize::try_from(seg.bytes).unwrap_or(usize::MAX);
            // a segment out of order is dropped (and its file with it)
            if self.buffer.push(end_ms, weight, seg).is_ok() {
                added += 1;
            }
        }
        self.read += whole;
        added
    }

    /// How much time lies between the end of the oldest segment held and the end of the newest:
    /// the clip is that plus the length of the oldest segment.
    pub fn span_ms(&self) -> u64 {
        self.buffer.span_ms()
    }

    /// Did the disk budget cut the start of the clip short?
    pub fn was_squeezed(&self) -> bool {
        self.buffer.was_squeezed()
    }

    /// The last look at the list, then the segments in order. Their files are deleted when the
    /// returned segments are dropped, so keep them until the clip is joined.
    pub fn finish(&mut self) -> Vec<Segment> {
        self.poll();
        // `read` stays: a segment already handed out must not be read in again
        self.buffer.take().into_iter().map(|(_, s)| s).collect()
    }

    /// A copy of the segments finished so far, in `into` (an empty folder), while the recording
    /// goes on: a continuous recording hands out its last seconds this way. The copies are
    /// deleted when they are dropped, like the segments themselves.
    pub fn snapshot(&mut self, into: &Path) -> std::io::Result<Vec<Segment>> {
        self.poll();
        fs::create_dir_all(into)?;
        let mut out = Vec::new();
        for (i, s) in self.buffer.items().enumerate() {
            let copy = into.join(format!("snap{i:05}.ts"));
            fs::copy(&s.path, &copy)?;
            out.push(Segment {
                path: copy,
                bytes: s.bytes,
                length_ms: s.length_ms,
            });
        }
        Ok(out)
    }

    /// Forget everything and delete the files.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.read = 0;
        let _ = fs::remove_file(&self.list);
    }
}

fn file_arg(p: &Path) -> OsString {
    let mut a = OsString::from("file:");
    a.push(p.as_os_str());
    a
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JoinError {
    #[error("There is nothing recorded")]
    Empty,
    #[error("A recorded file has a name that cannot be joined")]
    BadName,
}

/// How to stitch the segments into one MP4: the text of the list file ffmpeg's concat reader
/// wants, and the arguments (which read that list from `list_file`).
pub fn join(
    segments: &[Segment],
    list_file: &Path,
    output: &Path,
) -> Result<(String, Vec<OsString>), JoinError> {
    if segments.is_empty() {
        return Err(JoinError::Empty);
    }
    let mut list = String::new();
    for s in segments {
        let path = s.path.to_str().ok_or(JoinError::BadName)?;
        if path.contains(['\n', '\r']) {
            return Err(JoinError::BadName);
        }
        // inside single quotes, a quote is written '\''
        let _ = writeln!(list, "file '{}'", path.replace('\'', "'\\''"));
    }
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(file_arg(list_file));
    for a in ["-c", "copy", "-movflags", "+faststart"] {
        args.push(a.into());
    }
    args.push(file_arg(output));
    Ok((list, args))
}

#[cfg(test)]
#[path = "segments_tests.rs"]
mod tests;
