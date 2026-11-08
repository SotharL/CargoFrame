//! A picture that ffmpeg cannot capture itself, handed to it over a local connection.
//!
//! On Wayland the screen comes from the desktop portal as a PipeWire stream, which ffmpeg cannot
//! read; GStreamer can. A [`PictureFeed`] waits on a port of this computer; when ffmpeg connects
//! (as its video input), it starts the given program (GStreamer writing raw video as YUV4MPEG on
//! its standard output) and passes the bytes on. Nothing of the picture is kept by CargoFrame.
//!
//! The recorder may start ffmpeg again (when a sound device fails), so the feed serves one
//! connection after another until it is dropped.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const POLL: Duration = Duration::from_millis(50);

/// The GStreamer pipeline that reads a PipeWire node and writes YUV4MPEG at `fps` frames a
/// second. The portal sends a frame only when the screen changes; `keepalive-time` repeats the
/// last one so a still screen still makes a steady video.
pub fn pipewire_args(node: u32, fps: u32) -> Vec<String> {
    let keepalive = (1000 / fps.max(1)).max(10);
    [
        "-q",
        "pipewiresrc",
        &format!("path={node}"),
        "do-timestamp=true",
        &format!("keepalive-time={keepalive}"),
        "always-copy=true",
        "!",
        "videoconvert",
        "!",
        "videorate",
        "!",
        &format!("video/x-raw,format=I420,framerate={fps}/1"),
        "!",
        "y4menc",
        "!",
        "fdsink",
        "fd=1",
        "sync=false",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// A program's output passed to ffmpeg. Dropping it stops the program and whatever `keep` holds
/// (the screen-sharing session).
pub struct PictureFeed {
    port: u16,
    stop: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    said: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
    _keep: Box<dyn Send>,
}

impl PictureFeed {
    pub fn open(program: PathBuf, args: Vec<String>, keep: Box<dyn Send>) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let said = Arc::new(Mutex::new(None));
        let thread = {
            let (stop, child, said) = (stop.clone(), child.clone(), said.clone());
            std::thread::Builder::new()
                .name("picture-feed".into())
                .spawn(move || serve(listener, &program, &args, &stop, &child, &said))
                .map_err(|e| e.to_string())?
        };
        Ok(Self {
            port,
            stop,
            child,
            said,
            thread: Some(thread),
            _keep: keep,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Why the program stopped by itself, if it did.
    pub fn problem(&self) -> Option<String> {
        self.said.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for PictureFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mut c) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(
    listener: TcpListener,
    program: &PathBuf,
    args: &[String],
    stop: &AtomicBool,
    child: &Mutex<Option<Child>>,
    said: &Mutex<Option<String>>,
) {
    while !stop.load(Ordering::SeqCst) {
        let socket = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(POLL);
                continue;
            }
            Err(e) => {
                *said.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
                return;
            }
        };
        if let Err(why) = pass_on(socket, program, args, stop, child) {
            *said.lock().unwrap_or_else(|p| p.into_inner()) = Some(why);
        }
    }
}

/// Run the program and copy its output to `socket` until either ends.
fn pass_on(
    mut socket: TcpStream,
    program: &PathBuf,
    args: &[String],
    stop: &AtomicBool,
    child: &Mutex<Option<Child>>,
) -> Result<(), String> {
    let _ = socket.set_nonblocking(false);
    let mut started = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{} could not start: {e}", program.display()))?;
    let mut out = started.stdout.take().ok_or("no output")?;
    let mut err = started.stderr.take();
    *child.lock().unwrap_or_else(|p| p.into_inner()) = Some(started);
    let mut buf = vec![0u8; 1 << 20];
    let mut sent = 0u64;
    let ended = loop {
        if stop.load(Ordering::SeqCst) {
            break Ok(());
        }
        match out.read(&mut buf) {
            Ok(0) => {
                // the program ended: say why when it wrote something
                let mut text = String::new();
                if let Some(e) = err.as_mut() {
                    let _ = e.take(4096).read_to_string(&mut text);
                }
                break Err(if sent == 0 {
                    format!("the screen stream gave nothing: {}", text.trim())
                } else {
                    format!("the screen stream ended: {}", text.trim())
                });
            }
            Ok(n) => {
                if socket.write_all(&buf[..n]).is_err() {
                    // ffmpeg has stopped reading: the recording is over
                    break Ok(());
                }
                sent += n as u64;
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => break Err(e.to_string()),
        }
    };
    if let Some(mut c) = child.lock().unwrap_or_else(|p| p.into_inner()).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    ended
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pipeline_reads_the_node_and_repeats_still_frames() {
        let a = pipewire_args(57, 30).join(" ");
        assert!(a.contains("pipewiresrc path=57"), "{a}");
        assert!(a.contains("keepalive-time=33"), "{a}");
        assert!(
            a.contains("videoconvert ! videorate ! video/x-raw,format=I420,framerate=30/1"),
            "{a}"
        );
        assert!(a.ends_with("y4menc ! fdsink fd=1 sync=false"), "{a}");
    }

    #[cfg(unix)]
    #[test]
    fn what_the_program_writes_reaches_the_connection_and_a_second_one_starts_it_again() {
        let feed = PictureFeed::open(
            "/bin/sh".into(),
            vec!["-c".into(), "printf 'YUV4MPEG2 W2 H2 F1:1\\n'".into()],
            Box::new(()),
        )
        .unwrap();
        for _ in 0..2 {
            let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, feed.port())).unwrap();
            let mut got = String::new();
            s.read_to_string(&mut got).unwrap();
            assert_eq!(got, "YUV4MPEG2 W2 H2 F1:1\n");
        }
        drop(feed);
    }

    #[cfg(unix)]
    #[test]
    fn a_program_that_fails_says_why() {
        let feed = PictureFeed::open(
            "/bin/sh".into(),
            vec!["-c".into(), "echo 'no such node' >&2; exit 1".into()],
            Box::new(()),
        )
        .unwrap();
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, feed.port())).unwrap();
        let mut got = Vec::new();
        let _ = s.read_to_end(&mut got);
        for _ in 0..50 {
            if feed.problem().is_some() {
                break;
            }
            std::thread::sleep(POLL);
        }
        assert!(feed.problem().unwrap().contains("no such node"));
    }

    #[cfg(unix)]
    #[test]
    fn dropping_the_feed_stops_a_program_that_never_ends() {
        let feed = PictureFeed::open(
            "/bin/sh".into(),
            vec!["-c".into(), "while :; do printf x; sleep 0.01; done".into()],
            Box::new(()),
        )
        .unwrap();
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, feed.port())).unwrap();
        let mut one = [0u8; 1];
        s.read_exact(&mut one).unwrap();
        let started = std::time::Instant::now();
        drop(feed);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
