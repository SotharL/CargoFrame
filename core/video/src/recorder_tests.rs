use super::*;
use crate::export::probe;
use std::fs;

fn strings(a: &[OsString]) -> Vec<String> {
    a.iter().map(|x| x.to_string_lossy().into_owned()).collect()
}

fn after(a: &[String], flag: &str) -> String {
    let i = a
        .iter()
        .position(|x| x == flag)
        .unwrap_or_else(|| panic!("no {flag}"));
    a[i + 1].clone()
}

fn config(capture: Capture, dir: &Path) -> Config {
    Config::new("ffmpeg".into(), capture, dir.to_path_buf())
}

#[test]
fn an_x11_recording_is_captured_scaled_encoded_and_cut_into_segments() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(
        Capture::X11 {
            display: ":0.0".into(),
            size: Some((2560, 1440)),
        },
        dir.path(),
    );
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    assert_eq!(after(&a, "-f"), "x11grab");
    assert_eq!(after(&a, "-framerate"), "30");
    assert_eq!(after(&a, "-video_size"), "2560x1440");
    assert_eq!(after(&a, "-i"), ":0.0");
    // larger pictures are scaled down, never up
    assert_eq!(after(&a, "-vf"), "scale=-2:min(ih\\,1080),format=yuv420p");
    // a keyframe every second, because a segment starts on one
    assert_eq!(after(&a, "-g"), "30");
    // a backstop: ffmpeg ends by itself after an hour
    assert_eq!(after(&a, "-t"), "3600");
    assert_eq!(after(&a, "-c:v"), "libx264");
    assert_eq!(after(&a, "-preset"), "ultrafast");
    // the ring's own part comes last
    let ring_args = strings(&ring.output_args(1));
    assert_eq!(a[a.len() - ring_args.len()..], ring_args[..]);
}

#[test]
fn a_windows_recording_downloads_the_picture_from_the_graphics_card_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(Capture::Ddagrab { output: 1 }, dir.path());
    cfg.max_height = None;
    cfg.encoder = Encoder::Nvenc;
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    assert_eq!(after(&a, "-f"), "lavfi");
    assert_eq!(after(&a, "-i"), "ddagrab=output_idx=1:framerate=30");
    assert_eq!(after(&a, "-vf"), "hwdownload,format=bgra,format=yuv420p");
    assert_eq!(after(&a, "-c:v"), "h264_nvenc");

    cfg.capture = Capture::Gdigrab;
    cfg.encoder = Encoder::MediaFoundation;
    let a = strings(&args(&cfg, &ring));
    assert_eq!(after(&a, "-f"), "gdigrab");
    assert_eq!(after(&a, "-i"), "desktop");
    assert_eq!(after(&a, "-c:v"), "h264_mf");
}

#[test]
fn without_a_size_x11_takes_the_whole_screen() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(
        Capture::X11 {
            display: ":1".into(),
            size: None,
        },
        dir.path(),
    );
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    assert!(!a.iter().any(|x| x == "-video_size"));
    assert_eq!(after(&a, "-i"), ":1");
}

#[test]
fn a_window_is_recorded_by_asking_the_capture_for_just_its_part() {
    let dir = tempfile::tempdir().unwrap();
    let ring = Ring::new(dir.path(), Window::default(), 1);
    // odd sides are made even: the video format has no half pixels
    let area = Some(Area {
        x: 120,
        y: 80,
        w: 801,
        h: 603,
    });

    let mut cfg = config(
        Capture::X11 {
            display: ":0.0".into(),
            size: Some((2560, 1440)),
        },
        dir.path(),
    );
    cfg.area = area;
    let a = strings(&args(&cfg, &ring));
    assert_eq!(after(&a, "-video_size"), "800x602");
    assert_eq!(after(&a, "-i"), ":0.0+120,80");

    cfg.capture = Capture::Gdigrab;
    let a = strings(&args(&cfg, &ring));
    assert_eq!(after(&a, "-offset_x"), "120");
    assert_eq!(after(&a, "-offset_y"), "80");
    assert_eq!(after(&a, "-video_size"), "800x602");
    assert_eq!(after(&a, "-i"), "desktop");

    // gdigrab can start left of the first screen; x11grab and ddagrab cannot
    cfg.area = Some(Area {
        x: -300,
        y: -20,
        w: 640,
        h: 480,
    });
    let a = strings(&args(&cfg, &ring));
    assert_eq!(after(&a, "-offset_x"), "-300");
    cfg.capture = Capture::X11 {
        display: ":0".into(),
        size: None,
    };
    assert_eq!(after(&strings(&args(&cfg, &ring)), "-i"), ":0+0,0");
    cfg.capture = Capture::Ddagrab { output: 0 };
    let a = strings(&args(&cfg, &ring));
    assert_eq!(
        after(&a, "-i"),
        "ddagrab=output_idx=0:framerate=30:offset_x=0:offset_y=0:video_size=640x480"
    );
}

#[test]
fn a_part_too_small_to_be_a_window_records_the_whole_picture() {
    let dir = tempfile::tempdir().unwrap();
    let ring = Ring::new(dir.path(), Window::default(), 1);
    let mut cfg = config(Capture::Gdigrab, dir.path());
    for (w, h) in [(8, 600), (800, 15), (0, 0), (1, 1)] {
        cfg.area = Some(Area { x: 5, y: 5, w, h });
        let a = strings(&args(&cfg, &ring));
        assert!(!a.iter().any(|x| x == "-offset_x"), "{w}x{h}");
        assert!(!a.iter().any(|x| x == "-video_size"), "{w}x{h}");
    }
}

fn pulse(device: &str) -> AudioSource {
    AudioSource {
        backend: AudioBackend::Pulse,
        device: device.into(),
    }
}

#[test]
fn without_sound_nothing_audio_is_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(Capture::Gdigrab, dir.path());
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    for flag in ["-c:a", "-map", "-filter_complex", "pulse", "dshow"] {
        assert!(!a.iter().any(|x| x == flag), "{flag}");
    }
}

#[test]
fn one_sound_is_a_second_input_that_is_encoded_with_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(
        Capture::X11 {
            display: ":0".into(),
            size: None,
        },
        dir.path(),
    );
    cfg.audio = vec![pulse("@DEFAULT_MONITOR@")];
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    // the picture is input 0 and the sound input 1, and the sound follows the picture's own
    let inputs: Vec<usize> = (0..a.len()).filter(|i| a[*i] == "-i").collect();
    assert_eq!(inputs.len(), 2);
    assert_eq!(a[inputs[1] + 1], "@DEFAULT_MONITOR@");
    assert_eq!(a[inputs[1] - 2], "-f");
    assert_eq!(a[inputs[1] - 1], "pulse");
    assert_eq!(a[inputs[1] - 3], "1024");
    assert_eq!(a[inputs[1] - 4], "-thread_queue_size");
    // both are mapped, the filters still apply to the picture only
    assert_eq!(
        a.iter().filter(|x| *x == "-map").count(),
        2,
        "one -map per stream"
    );
    assert!(a.windows(2).any(|w| w == ["-map", "0:v"]));
    assert!(a.windows(2).any(|w| w == ["-map", "1:a"]));
    assert_eq!(after(&a, "-c:a"), "aac");
    assert_eq!(after(&a, "-ar"), "48000");
    // the sound is encoded before the output options of the ring
    let ring_args = strings(&ring.output_args(1));
    assert_eq!(a[a.len() - ring_args.len()..], ring_args[..]);
}

#[test]
fn a_windows_sound_goes_in_by_directshow_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(Capture::Gdigrab, dir.path());
    cfg.audio = vec![AudioSource {
        backend: AudioBackend::Dshow,
        device: "Stereo Mix (Realtek(R) Audio)".into(),
    }];
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    assert!(a.windows(2).any(|w| w == ["-f", "dshow"]));
    assert!(a.iter().any(|x| x == "audio=Stereo Mix (Realtek(R) Audio)"));
}

#[test]
fn a_sound_that_cargoframe_listens_to_comes_in_as_raw_samples_over_a_local_connection() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(Capture::Gdigrab, dir.path());
    cfg.audio = vec![AudioSource {
        backend: AudioBackend::Pcm {
            port: 41234,
            rate: 48_000,
            channels: 2,
        },
        device: String::new(),
    }];
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    assert!(a.windows(2).any(|w| w == ["-f", "f32le"]));
    assert_eq!(after(&a, "-ar"), "48000");
    assert!(a.windows(2).any(|w| w == ["-ac", "2"]));
    assert!(a.iter().any(|x| x == "tcp://127.0.0.1:41234"));
    assert!(a.windows(2).any(|w| w == ["-map", "1:a"]));
}

#[test]
fn a_loopback_that_has_not_been_set_up_is_left_out_of_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(Capture::Gdigrab, dir.path());
    cfg.audio = vec![AudioSource {
        backend: AudioBackend::Loopback { program: None },
        device: String::new(),
    }];
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    for flag in ["-c:a", "-map", "-filter_complex"] {
        assert!(!a.iter().any(|x| x == flag), "{flag}");
    }
    // and it does not use up one of the two places for sounds
    cfg.audio.push(pulse("a"));
    cfg.audio.push(pulse("b"));
    let a = strings(&args(&cfg, &ring));
    assert!(
        a.iter().any(|x| x == "amix=inputs=2") || a.iter().any(|x| x.contains("amix=inputs=2"))
    );
}

#[test]
fn two_sounds_are_mixed_into_one_track() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(
        Capture::X11 {
            display: ":0".into(),
            size: None,
        },
        dir.path(),
    );
    cfg.audio = vec![pulse("@DEFAULT_MONITOR@"), pulse("default"), pulse("third")];
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    // a third is ignored: a recording has the screen's sound and the voice, no more
    assert_eq!(a.iter().filter(|x| *x == "-i").count(), 3);
    assert!(!a.iter().any(|x| x == "third"));
    let graph = after(&a, "-filter_complex");
    assert!(graph.starts_with("[0:v]scale="), "{graph}");
    assert!(graph.contains("[v];[1:a][2:a]amix=inputs=2"), "{graph}");
    assert!(!a.iter().any(|x| x == "-vf"));
    assert!(a.windows(2).any(|w| w == ["-map", "[v]"]));
    assert!(a.windows(2).any(|w| w == ["-map", "[a]"]));
}

#[test]
fn vaapi_names_its_device_first_and_uploads_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(
        Capture::X11 {
            display: ":0".into(),
            size: Some((1280, 720)),
        },
        dir.path(),
    );
    cfg.encoder = Encoder::Vaapi {
        device: "/dev/dri/renderD128".into(),
    };
    let ring = Ring::new(dir.path(), cfg.window, cfg.max_bytes);
    let a = strings(&args(&cfg, &ring));
    let device = a.iter().position(|x| x == "-vaapi_device").unwrap();
    assert!(device < a.iter().position(|x| x == "-i").unwrap());
    assert!(after(&a, "-vf").ends_with("format=nv12,hwupload"));
    assert_eq!(after(&a, "-c:v"), "h264_vaapi");
}

#[test]
fn the_frame_rate_is_kept_sensible() {
    let dir = tempfile::tempdir().unwrap();
    let ring = Ring::new(dir.path(), Window::default(), 1);
    for (asked, used) in [(0, "5"), (1, "5"), (24, "24"), (500, "60")] {
        let mut cfg = config(Capture::Gdigrab, dir.path());
        cfg.fps = asked;
        let a = strings(&args(&cfg, &ring));
        assert_eq!(after(&a, "-framerate"), used);
        assert_eq!(after(&a, "-g"), used);
    }
}

#[test]
fn a_missing_ffmpeg_is_said_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(Capture::Gdigrab, dir.path());
    cfg.ffmpeg = "/no/such/ffmpeg".into();
    let err = Recorder::start(&cfg).err().expect("must fail");
    assert!(
        matches!(err, RecordError::Export(ExportError::NoEncoder(_))),
        "{err}"
    );
}

// ----- with a real ffmpeg and a virtual screen (skipped where there is none) -----

/// A virtual X screen that goes away with the guard.
/// The tests that record a real screen share the computer, and each promises the recorder a
/// little time: one at a time, so a busy computer does not starve one of them.
fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

struct Screen {
    server: Child,
    display: String,
}

impl Screen {
    fn start(number: u32) -> Option<Self> {
        let server = Command::new("Xvfb")
            .args([
                format!(":{number}"),
                "-screen".into(),
                "0".into(),
                "640x480x24".into(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let screen = Self {
            server,
            display: format!(":{number}"),
        };
        let socket = format!("/tmp/.X11-unix/X{number}");
        for _ in 0..60 {
            if Path::new(&socket).exists() {
                return Some(screen);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}

#[test]
fn recording_the_screen_for_a_while_gives_a_clip_of_the_last_seconds() {
    let Some((ffmpeg, ffprobe)) = find_tools(&[]) else {
        eprintln!("skipped: ffmpeg and ffprobe are not installed");
        return;
    };
    let _one = one_at_a_time();
    let Some(screen) = Screen::start(97) else {
        eprintln!("skipped: no virtual X screen (Xvfb) to record");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let ring_dir = dir.path().join("ring");
    fs::create_dir(&ring_dir).unwrap();
    let mut cfg = Config::new(
        ffmpeg.clone(),
        Capture::X11 {
            display: screen.display.clone(),
            size: Some((640, 480)),
        },
        ring_dir.clone(),
    );
    cfg.window = Window::seconds(2);
    cfg.fps = 15;

    let mut rec = Recorder::start(&cfg).unwrap();
    let began = Instant::now();
    let mut last = Progress {
        buffered_ms: 0,
        squeezed: false,
    };
    while began.elapsed() < Duration::from_secs(6) {
        last = rec.poll().unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(rec.elapsed() >= Duration::from_secs(6));
    // six seconds in, only about two are kept
    assert!(last.buffered_ms <= 3_000, "{last:?}");
    assert!(!last.squeezed);

    let segments = rec.stop(Duration::from_secs(10)).unwrap();
    assert!(
        !segments.is_empty() && segments.len() <= 5,
        "{} segments",
        segments.len()
    );
    let clip = dir.path().join("clip.mp4");
    assemble(&ffmpeg, &segments, dir.path(), &clip, &|| false).unwrap();
    drop(segments);

    let info = probe(&ffprobe, &clip).unwrap();
    assert_eq!((info.width, info.height), (640, 480));
    assert!(
        (1_500..=4_500).contains(&info.duration_ms),
        "a 2 s window gave a {} ms clip",
        info.duration_ms
    );
    // the ring folder holds no segment once the clip is made
    let left = fs::read_dir(&ring_dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "ts")
        })
        .count();
    assert_eq!(left, 0);
}

/// How long the picture and the sound of a clip are, in seconds.
fn stream_lengths(ffprobe: &Path, clip: &Path) -> (f64, Option<f64>) {
    let out = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,duration",
            "-of",
            "csv=p=0",
        ])
        .arg(clip)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (mut video, mut audio) = (0.0, None);
    for line in text.lines() {
        // "video,3.000000" (the order of the two fields is the order asked for)
        let mut parts = line.trim().split(',');
        let a = parts.next().unwrap_or("");
        let b = parts.next().unwrap_or("");
        let (kind, secs) = if a == "audio" || a == "video" {
            (a, b)
        } else {
            (b, a)
        };
        let secs: f64 = secs.parse().unwrap_or(0.0);
        match kind {
            "video" => video = secs,
            "audio" => audio = Some(secs),
            _ => {}
        }
    }
    (video, audio)
}

fn record_with_heard_sound(quiet: bool) -> Option<(f64, Option<f64>)> {
    use crate::pcm::{tests::Tone, Feed, PcmSource};
    let (ffmpeg, ffprobe) = find_tools(&[])?;
    let _one = one_at_a_time();
    let screen = Screen::start(96)?;
    let dir = tempfile::tempdir().unwrap();
    let ring_dir = dir.path().join("ring");
    fs::create_dir(&ring_dir).unwrap();
    let feed = Feed::open(move || Ok(Box::new(Tone::new(quiet)) as Box<dyn PcmSource>)).unwrap();
    let mut cfg = Config::new(
        ffmpeg.clone(),
        Capture::X11 {
            display: screen.display.clone(),
            size: Some((640, 480)),
        },
        ring_dir,
    );
    cfg.window = Window::seconds(30);
    cfg.fps = 15;
    cfg.audio = vec![AudioSource {
        backend: AudioBackend::Pcm {
            port: feed.port(),
            rate: feed.format().rate,
            channels: feed.format().channels,
        },
        device: String::new(),
    }];
    let mut rec = Recorder::start(&cfg).unwrap();
    rec.keep(Box::new(feed));
    assert_eq!(rec.early_failure(Duration::from_millis(1500)), None);
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(5) {
        rec.poll().unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
    let segments = rec.stop(Duration::from_secs(10)).unwrap();
    let clip = dir.path().join("clip.mp4");
    assemble(&ffmpeg, &segments, dir.path(), &clip, &|| false).unwrap();
    drop(segments);
    Some(stream_lengths(&ffprobe, &clip))
}

#[test]
fn a_recording_with_sound_cargoframe_heard_has_a_sound_track_as_long_as_the_picture() {
    let Some((video, audio)) = record_with_heard_sound(false) else {
        eprintln!("skipped: ffmpeg or a virtual X screen is missing");
        return;
    };
    let audio = audio.expect("the clip has no sound");
    assert!(video > 3.0, "{video}");
    assert!(
        (audio - video).abs() < 0.6,
        "picture {video} s, sound {audio} s"
    );
}

#[test]
fn a_computer_that_plays_nothing_still_gives_a_recording_with_a_sound_track() {
    let Some((video, audio)) = record_with_heard_sound(true) else {
        eprintln!("skipped: ffmpeg or a virtual X screen is missing");
        return;
    };
    let audio = audio.expect("the clip has no sound");
    assert!(
        (audio - video).abs() < 0.6,
        "picture {video} s, sound {audio} s"
    );
}

#[test]
fn a_recorder_that_cannot_start_reports_why_and_a_cancelled_one_leaves_nothing() {
    let Some((ffmpeg, _)) = find_tools(&[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    // no X server on this display
    let cfg = Config::new(
        ffmpeg.clone(),
        Capture::X11 {
            display: ":96".into(),
            size: Some((640, 480)),
        },
        dir.path().to_path_buf(),
    );
    let mut early = Recorder::start(&cfg).unwrap();
    let said = early.early_failure(Duration::from_secs(10));
    assert!(
        said.is_some_and(|m| !m.is_empty()),
        "it should say why at once"
    );
    early.cancel();
    let mut rec = Recorder::start(&cfg).unwrap();
    let began = Instant::now();
    let err = loop {
        match rec.poll() {
            Err(e) => break e,
            Ok(_) if began.elapsed() > Duration::from_secs(10) => panic!("it never gave up"),
            Ok(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    assert!(matches!(err, RecordError::Died(_)), "{err}");
    assert!(!err.to_string().is_empty());
    rec.cancel();

    let _one = one_at_a_time();
    let Some(screen) = Screen::start(95) else {
        eprintln!("skipped the cancel check: no virtual X screen");
        return;
    };
    let ring_dir = dir.path().join("ring2");
    fs::create_dir(&ring_dir).unwrap();
    let mut cfg = Config::new(
        ffmpeg,
        Capture::X11 {
            display: screen.display.clone(),
            size: Some((640, 480)),
        },
        ring_dir.clone(),
    );
    cfg.fps = 10;
    let mut rec = Recorder::start(&cfg).unwrap();
    // one that works is not reported as failed
    assert_eq!(rec.early_failure(Duration::from_millis(700)), None);
    std::thread::sleep(Duration::from_millis(1_800));
    rec.poll().unwrap();
    rec.cancel();
    let list = ring_dir.join("segments.csv");
    assert!(!list.exists(), "the list is removed");
}

#[test]
fn a_recorder_that_is_let_go_while_running_stops_ffmpeg() {
    let Some((ffmpeg, _)) = find_tools(&[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let _one = one_at_a_time();
    let Some(screen) = Screen::start(94) else {
        eprintln!("skipped: no virtual X screen");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new(
        ffmpeg,
        Capture::X11 {
            display: screen.display.clone(),
            size: Some((640, 480)),
        },
        dir.path().to_path_buf(),
    );
    cfg.fps = 10;
    let mut rec = Recorder::start(&cfg).unwrap();
    std::thread::sleep(Duration::from_millis(2_500));
    rec.poll().unwrap();
    drop(rec);

    let count = || fs::read_dir(dir.path()).unwrap().count();
    let after_drop = count();
    std::thread::sleep(Duration::from_millis(2_200));
    assert_eq!(
        count(),
        after_drop,
        "ffmpeg was still writing after the recorder was dropped"
    );
}
