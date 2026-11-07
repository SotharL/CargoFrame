//! Sound that CargoFrame hears itself and hands to ffmpeg over a local connection.
//!
//! ffmpeg cannot record what a Windows computer plays, and it cannot record one program's sound,
//! so CargoFrame listens (see `loopback`) and passes the samples on: a [`Feed`] waits on a port of
//! this computer, ffmpeg connects to it as an input, and the samples are written to it as they
//! come.
//!
//! A computer that plays nothing gives no samples at all, and a recording needs a sound as long as
//! the picture. So the feed keeps time itself and writes silence for the stretches in which the
//! source heard nothing.

use std::io::{ErrorKind, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long the sound system has to open before the feed gives up.
const READY_WITHIN: Duration = Duration::from_secs(10);
/// How long ffmpeg has to connect before the feed gives up.
const CONNECT_WITHIN: Duration = Duration::from_secs(20);
/// Silence is only written for a gap longer than this (shorter ones are the source's own rhythm).
const GAP: Duration = Duration::from_millis(40);
/// Of a gap longer than this (the computer was asleep or stuck) only this much is filled in.
const LONGEST_GAP: Duration = Duration::from_secs(10);
/// The most the feed waits for the source in one go.
const POLL: Duration = Duration::from_millis(20);

/// What a source hands over: 32-bit float samples, little endian, the channels interleaved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmFormat {
    pub rate: u32,
    pub channels: u16,
}

impl PcmFormat {
    pub fn frame_bytes(self) -> usize {
        4 * usize::from(self.channels.max(1))
    }
}

/// Something that hears a sound. It lives on the thread that made it (the Windows sound system
/// does not let its objects travel between threads), so it need not be `Send`.
pub trait PcmSource {
    fn format(&self) -> PcmFormat;
    /// Add what has been heard since the last call to `out`, waiting at most `wait` for something.
    /// Hearing nothing adds nothing. An error ends the feed.
    fn read(&mut self, out: &mut Vec<u8>, wait: Duration) -> Result<(), String>;
}

/// A source being passed to ffmpeg. Dropping it stops it.
pub struct Feed {
    port: u16,
    format: PcmFormat,
    stop: Arc<AtomicBool>,
    said: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl Feed {
    /// Start listening with the source `make` builds, and wait on a port of this computer for
    /// ffmpeg to ask for it. `make` runs on the thread that then feeds ffmpeg; if it cannot make
    /// a source, this says why and nothing is left running.
    pub fn open(
        make: impl FnOnce() -> Result<Box<dyn PcmSource>, String> + Send + 'static,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let said = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<PcmFormat, String>>();
        let thread = {
            let (stop, said) = (Arc::clone(&stop), Arc::clone(&said));
            std::thread::Builder::new()
                .name("sound-feed".into())
                .spawn(move || match make() {
                    Ok(source) => {
                        let _ = ready_tx.send(Ok(source.format()));
                        run(listener, source, &stop, &said);
                    }
                    Err(why) => {
                        let _ = ready_tx.send(Err(why));
                    }
                })
                .map_err(|e| e.to_string())?
        };
        match ready_rx.recv_timeout(READY_WITHIN) {
            Ok(Ok(format)) => Ok(Self {
                port,
                format,
                stop,
                said,
                thread: Some(thread),
            }),
            Ok(Err(why)) => {
                let _ = thread.join();
                Err(why)
            }
            Err(_) => {
                // never ready: let the thread end when it can, and say so
                stop.store(true, Ordering::SeqCst);
                Err("the sound system did not answer".into())
            }
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn format(&self) -> PcmFormat {
        self.format
    }

    /// Why the feed ended by itself, if it did.
    pub fn problem(&self) -> Option<String> {
        self.said.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn tell(said: &Mutex<Option<String>>, why: String) {
    *said.lock().unwrap_or_else(|e| e.into_inner()) = Some(why);
}

fn run(
    listener: TcpListener,
    mut source: Box<dyn PcmSource>,
    stop: &AtomicBool,
    said: &Mutex<Option<String>>,
) {
    let began = Instant::now();
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                if began.elapsed() > CONNECT_WITHIN {
                    tell(said, "ffmpeg did not ask for the sound".into());
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                tell(said, e.to_string());
                return;
            }
        }
    };
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    drop(listener);

    let format = source.format();
    let frame = format.frame_bytes();
    let rate = u64::from(format.rate);
    let start = Instant::now();
    let mut sent: u64 = 0;
    let mut heard = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        heard.clear();
        if let Err(why) = source.read(&mut heard, POLL) {
            tell(said, why);
            return;
        }
        heard.truncate(heard.len() - heard.len() % frame);
        let frames = (heard.len() / frame) as u64;
        // how much sound the time that has passed calls for, against what has been sent
        let due = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX) * rate / 1_000_000;
        let behind = due.saturating_sub(sent + frames);
        let gap = Duration::from_micros(behind * 1_000_000 / rate.max(1));
        // a short gap is the source's own rhythm; a long one is the computer having been asleep
        // or stuck, and only the first part of it is filled in
        let (silence, skipped) = if gap > GAP {
            let kept = behind.min(LONGEST_GAP.as_secs() * rate);
            (kept, behind)
        } else {
            (0, 0)
        };
        // what the source heard came after the silence, so the silence goes first
        let mut chunk = vec![0u8; silence as usize * frame];
        chunk.extend_from_slice(&heard);
        if chunk.is_empty() {
            continue;
        }
        if stream.write_all(&chunk).is_err() {
            // ffmpeg has ended (a recording that has stopped): nothing is left to tell
            return;
        }
        sent += skipped + frames;
    }
}

#[cfg(test)]
#[path = "pcm_tests.rs"]
pub(crate) mod tests;
