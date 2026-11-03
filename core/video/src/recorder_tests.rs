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
            size: (2560, 1440),
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
fn vaapi_names_its_device_first_and_uploads_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(
        Capture::X11 {
            display: ":0".into(),
            size: (1280, 720),
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
            size: (640, 480),
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
            size: (640, 480),
        },
        dir.path().to_path_buf(),
    );
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
            size: (640, 480),
        },
        ring_dir.clone(),
    );
    cfg.fps = 10;
    let mut rec = Recorder::start(&cfg).unwrap();
    std::thread::sleep(Duration::from_millis(2_500));
    rec.poll().unwrap();
    rec.cancel();
    let list = ring_dir.join("segments.csv");
    assert!(!list.exists(), "the list is removed");
}
