use super::*;
use std::io::Read;
use std::net::TcpStream;

pub(crate) const FORMAT: PcmFormat = PcmFormat {
    rate: 48_000,
    channels: 2,
};

/// A source that hears a tone in real time, or nothing at all.
pub(crate) struct Tone {
    last: Instant,
    owed: f64,
    phase: f64,
    pub quiet: bool,
}

impl Tone {
    pub(crate) fn new(quiet: bool) -> Self {
        Self {
            last: Instant::now(),
            owed: 0.0,
            phase: 0.0,
            quiet,
        }
    }
}

impl PcmSource for Tone {
    fn format(&self) -> PcmFormat {
        FORMAT
    }

    fn read(&mut self, out: &mut Vec<u8>, wait: Duration) -> Result<(), String> {
        std::thread::sleep(wait);
        self.owed += self.last.elapsed().as_secs_f64() * f64::from(FORMAT.rate);
        self.last = Instant::now();
        let frames = self.owed as usize;
        self.owed -= frames as f64;
        if self.quiet {
            return Ok(());
        }
        for _ in 0..frames {
            self.phase += 2.0 * std::f64::consts::PI * 440.0 / f64::from(FORMAT.rate);
            let sample = (self.phase.sin() * 0.4) as f32;
            for _ in 0..FORMAT.channels {
                out.extend_from_slice(&sample.to_le_bytes());
            }
        }
        Ok(())
    }
}

/// Read from the feed for `for_how_long`, as ffmpeg would.
fn listen(feed: &Feed, for_how_long: Duration) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", feed.port())).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let began = Instant::now();
    let mut all = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    while began.elapsed() < for_how_long {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => all.extend_from_slice(&buf[..n]),
            Err(_) => {}
        }
    }
    all
}

fn one_second_of_bytes() -> usize {
    FORMAT.rate as usize * FORMAT.frame_bytes()
}

#[test]
fn a_computer_that_plays_nothing_still_gives_sound_as_long_as_the_time_that_passed() {
    let feed = Feed::open(|| Ok(Box::new(Tone::new(true)) as Box<dyn PcmSource>)).unwrap();
    assert_eq!(feed.format(), FORMAT);
    let got = listen(&feed, Duration::from_millis(1500));
    let seconds = got.len() as f64 / one_second_of_bytes() as f64;
    assert!(
        (1.2..=1.7).contains(&seconds),
        "1.5 s of silence came as {seconds:.2} s"
    );
    assert!(got.iter().all(|b| *b == 0), "silence must be zeros");
}

#[test]
fn what_is_heard_is_passed_on_in_step_with_the_clock() {
    let feed = Feed::open(|| Ok(Box::new(Tone::new(false)) as Box<dyn PcmSource>)).unwrap();
    let got = listen(&feed, Duration::from_millis(1500));
    let seconds = got.len() as f64 / one_second_of_bytes() as f64;
    assert!(
        (1.2..=1.7).contains(&seconds),
        "1.5 s of sound came as {seconds:.2} s"
    );
    let loud = got
        .chunks_exact(4)
        .filter(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]).abs() > 0.05)
        .count();
    assert!(loud > got.len() / 4 / 2, "the tone was lost");
}

struct Breaks {
    reads: u32,
}

impl PcmSource for Breaks {
    fn format(&self) -> PcmFormat {
        FORMAT
    }
    fn read(&mut self, _: &mut Vec<u8>, wait: Duration) -> Result<(), String> {
        std::thread::sleep(wait);
        self.reads += 1;
        if self.reads > 3 {
            return Err("the device was unplugged".into());
        }
        Ok(())
    }
}

#[test]
fn a_source_that_fails_ends_the_feed_and_says_why() {
    let feed = Feed::open(|| Ok(Box::new(Breaks { reads: 0 }) as Box<dyn PcmSource>)).unwrap();
    let _ = listen(&feed, Duration::from_millis(600));
    assert_eq!(feed.problem().as_deref(), Some("the device was unplugged"));
}

#[test]
fn a_source_that_cannot_be_made_says_why_and_leaves_nothing_running() {
    let err = Feed::open(|| Err("there is no sound device".into()))
        .err()
        .expect("must fail");
    assert_eq!(err, "there is no sound device");
}

#[test]
fn a_feed_that_is_dropped_before_ffmpeg_comes_ends_at_once() {
    let feed = Feed::open(|| Ok(Box::new(Tone::new(true)) as Box<dyn PcmSource>)).unwrap();
    let began = Instant::now();
    drop(feed);
    assert!(began.elapsed() < Duration::from_secs(2));
}
