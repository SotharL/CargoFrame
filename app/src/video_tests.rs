use super::*;
use cargoframe_video::locate;
use std::fs;
use std::process::{Child, Command, Stdio};

const ENCODERS: &str = " Encoders:
 V..... = Video
 ------
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC (codec h264)
 V....D h264_nvenc           NVIDIA NVENC H.264 encoder (codec h264)
 V....D h264_mf              H264 via MediaFoundation (codec h264)
 V....D libx264rgb           libx264 H.264 / AVC / MPEG-4 AVC (codec h264)
 A....D aac                  AAC (Advanced Audio Coding)
";

const DEVICES: &str = "Devices:
 D. = Demuxing supported
 ..
 D  gdigrab         GDI API Windows frame grabber
 D  lavfi           Libavfilter virtual input device
";

const FILTERS: &str = "Filters:
 ... ddagrab           |->V       Grab Windows Desktop images using Desktop Duplication API.
 ... drawbox           V->V       Draw a colored box on the input image.
";

fn found(encoders: &[VideoEncoder], captures: &[VideoCapture]) -> Found {
    Found {
        ffmpeg: "ffmpeg".into(),
        ffprobe: "ffprobe".into(),
        version: "7.1".into(),
        encoders: encoders.to_vec(),
        captures: captures.to_vec(),
    }
}

fn windows() -> Session {
    Session::default()
}

#[test]
fn ffmpegs_lists_are_read_by_the_name_in_the_second_column() {
    assert_eq!(
        parse_encoders(ENCODERS),
        vec![
            VideoEncoder::Software,
            VideoEncoder::Nvenc,
            VideoEncoder::MediaFoundation
        ]
    );
    // libx264rgb is not libx264, and a description that mentions a name does not count
    assert_eq!(
        parse_encoders(" V....D libx264rgb  uses h264_nvenc and libx264 too"),
        vec![]
    );
    assert_eq!(
        parse_captures(DEVICES, FILTERS),
        vec![VideoCapture::Gdigrab, VideoCapture::Ddagrab]
    );
    assert_eq!(
        parse_captures(" D  x11grab  X11 screen capture (XCB)", ""),
        vec![VideoCapture::X11]
    );
    assert_eq!(parse_captures("", ""), vec![]);
    assert_eq!(
        parse_version("ffmpeg version 7.1-full_build-www.gyan.dev Copyright (c) 2000-2024"),
        "7.1-full_build-www.gyan.dev"
    );
    assert_eq!(parse_version("nothing useful"), "unknown version");
}

#[test]
fn a_chosen_place_is_the_only_place_looked_in() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("ffmpeg.exe");
    fs::write(&program, b"x").unwrap();

    let (dirs, chosen) = search_dirs(Some(program.to_str().unwrap()), Some(Path::new("/app")));
    assert!(chosen);
    assert_eq!(dirs, vec![dir.path().to_path_buf()]);
    let (dirs, chosen) = search_dirs(Some(dir.path().to_str().unwrap()), None);
    assert!(chosen);
    assert_eq!(dirs, vec![dir.path().to_path_buf()]);
    // empty or blank means: next to the app, then the search path
    for blank in [None, Some(""), Some("   ")] {
        let (dirs, chosen) = search_dirs(blank, Some(Path::new("/app")));
        assert!(!chosen);
        assert_eq!(dirs, vec![PathBuf::from("/app")]);
    }

    let none = Video {
        ffmpeg: Some(dir.path().join("nowhere").display().to_string()),
        ..Video::default()
    };
    let err = find(&none, None).unwrap_err();
    assert!(err.contains("was not found in"), "{err}");

    // ffmpeg without ffprobe says so
    let video = Video {
        ffmpeg: Some(dir.path().display().to_string()),
        ..Video::default()
    };
    let err = find(&video, None).unwrap_err();
    assert!(err.contains("ffprobe was not found"), "{err}");
}

#[test]
fn auto_picks_a_safe_capture_and_software_on_windows() {
    let all = found(
        &[VideoEncoder::Software, VideoEncoder::Nvenc],
        &[VideoCapture::Gdigrab, VideoCapture::Ddagrab],
    );
    let c = choose(&Video::default(), &all, Os::Windows, &windows()).unwrap();
    assert_eq!(c.capture, VideoCapture::Gdigrab);
    assert_eq!(c.encoder, VideoEncoder::Software);

    // a build with only one of them uses that one
    let dda = found(&[VideoEncoder::Software], &[VideoCapture::Ddagrab]);
    assert_eq!(
        choose(&Video::default(), &dda, Os::Windows, &windows())
            .unwrap()
            .capture,
        VideoCapture::Ddagrab
    );
    let none = found(&[VideoEncoder::Software], &[]);
    let err = choose(&Video::default(), &none, Os::Windows, &windows()).unwrap_err();
    assert!(err.contains("gdigrab"), "{err}");

    // no x264 in this build: the first hardware encoder it has
    let hw = found(
        &[VideoEncoder::Amf, VideoEncoder::MediaFoundation],
        &[VideoCapture::Gdigrab],
    );
    assert_eq!(
        choose(&Video::default(), &hw, Os::Windows, &windows())
            .unwrap()
            .encoder,
        VideoEncoder::Amf
    );
    let nothing = found(&[], &[VideoCapture::Gdigrab]);
    assert!(choose(&Video::default(), &nothing, Os::Windows, &windows())
        .unwrap_err()
        .contains("no H.264 encoder"));
}

#[test]
fn an_explicit_choice_must_exist_in_this_ffmpeg() {
    let f = found(&[VideoEncoder::Software], &[VideoCapture::Gdigrab]);
    let v = Video {
        capture: VideoCapture::Ddagrab,
        ..Video::default()
    };
    let err = choose(&v, &f, Os::Windows, &windows()).unwrap_err();
    assert!(err.contains("Desktop Duplication"), "{err}");
    let v = Video {
        encoder: VideoEncoder::Nvenc,
        ..Video::default()
    };
    let err = choose(&v, &f, Os::Windows, &windows()).unwrap_err();
    assert!(err.contains("NVIDIA"), "{err}");
    let v = Video {
        capture: VideoCapture::Gdigrab,
        encoder: VideoEncoder::Software,
        ..Video::default()
    };
    assert!(choose(&v, &f, Os::Windows, &windows()).is_ok());
}

#[test]
fn linux_records_x11_and_says_plainly_that_wayland_is_not_supported() {
    let f = found(&[VideoEncoder::Software], &[VideoCapture::X11]);
    let x11 = Session {
        wayland: false,
        display: Some(":1".into()),
    };
    assert_eq!(
        choose(&Video::default(), &f, Os::Linux, &x11)
            .unwrap()
            .capture,
        VideoCapture::X11
    );
    let wayland = Session {
        wayland: true,
        display: Some(":0".into()),
    };
    let err = choose(&Video::default(), &f, Os::Linux, &wayland).unwrap_err();
    assert!(err.contains("Wayland"), "{err}");
    let forced = Video {
        capture: VideoCapture::X11,
        ..Video::default()
    };
    assert!(choose(&forced, &f, Os::Linux, &wayland)
        .unwrap_err()
        .contains("Wayland"));
    let no_x11 = found(&[VideoEncoder::Software], &[]);
    assert!(choose(&Video::default(), &no_x11, Os::Linux, &x11)
        .unwrap_err()
        .contains("x11grab"));
    assert!(choose(&Video::default(), &f, Os::Other, &x11).is_err());
}

#[test]
fn settings_outside_their_range_are_refused_and_typed_paths_are_tidied() {
    assert!(validate(&Video::default()).is_ok());
    for bad in [
        Video {
            fps: 4,
            ..Video::default()
        },
        Video {
            fps: 61,
            ..Video::default()
        },
        Video {
            window_secs: 0,
            ..Video::default()
        },
        Video {
            window_secs: 31,
            ..Video::default()
        },
        Video {
            max_height: 100,
            ..Video::default()
        },
        Video {
            max_height: 5000,
            ..Video::default()
        },
    ] {
        assert!(validate(&bad).is_err(), "{bad:?}");
    }
    // 0 means "no limit"
    assert!(validate(&Video {
        max_height: 0,
        ..Video::default()
    })
    .is_ok());

    let v = normalized(Video {
        ffmpeg: Some("  \"C:\\tools\\ffmpeg\\bin\"  ".into()),
        ..Video::default()
    });
    assert_eq!(v.ffmpeg.as_deref(), Some("C:\\tools\\ffmpeg\\bin"));
    assert_eq!(
        normalized(Video {
            ffmpeg: Some("  ".into()),
            ..Video::default()
        })
        .ffmpeg,
        None
    );
}

#[test]
fn the_recorder_gets_the_settings_and_the_whole_x11_screen() {
    let f = found(&[VideoEncoder::Software], &[VideoCapture::X11]);
    let session = Session {
        wayland: false,
        display: Some(":2".into()),
    };
    let video = Video {
        fps: 20,
        window_secs: 7,
        max_height: 0,
        ..Video::default()
    };
    let choice = Choice {
        capture: VideoCapture::X11,
        encoder: VideoEncoder::Software,
    };
    let cfg = recorder_config(&f, &choice, &video, &session, Path::new("/r"));
    assert_eq!(
        cfg.capture,
        Capture::X11 {
            display: ":2".into(),
            size: None
        }
    );
    assert_eq!(cfg.fps, 20);
    assert_eq!(cfg.window.as_secs(), 7);
    assert_eq!(cfg.max_height, None);

    let choice = Choice {
        capture: VideoCapture::Ddagrab,
        encoder: VideoEncoder::Vaapi,
    };
    let cfg = recorder_config(&f, &choice, &Video::default(), &session, Path::new("/r"));
    assert_eq!(cfg.capture, Capture::Ddagrab { output: 0 });
    assert!(matches!(cfg.encoder, Encoder::Vaapi { .. }));
    assert_eq!(cfg.max_height, Some(1080));
}

#[test]
fn the_tools_view_says_why_nothing_can_be_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let v = Video {
        ffmpeg: Some(dir.path().display().to_string()),
        ..Video::default()
    };
    let view = tools_view(&v, Os::Windows, &windows());
    assert!(!view.found);
    assert!(view.problem.unwrap().contains("ffmpeg"));
    assert!(view.encoders.is_empty() && view.auto_capture.is_none());
}

#[test]
fn a_failure_gets_a_hint_that_fits_what_was_tried() {
    let hw = Choice {
        capture: VideoCapture::Gdigrab,
        encoder: VideoEncoder::Nvenc,
    };
    assert!(hint(&hw).contains("Software"));
    let dda = Choice {
        capture: VideoCapture::Ddagrab,
        encoder: VideoEncoder::Software,
    };
    assert!(hint(&dda).contains("GDI"));
    let plain = Choice {
        capture: VideoCapture::Gdigrab,
        encoder: VideoEncoder::Software,
    };
    assert_eq!(hint(&plain), "");
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
fn a_test_recording_of_a_real_screen_gives_a_clip_and_a_report() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let Some(ffprobe) = locate("ffprobe", &[]) else {
        eprintln!("skipped: ffprobe is not installed");
        return;
    };
    let Some(screen) = Screen::start(91) else {
        eprintln!("skipped: no virtual X screen (Xvfb) to record");
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let work = work.path().join("video-test");
    let f = Found {
        ffmpeg,
        ffprobe,
        ..found(&[VideoEncoder::Software], &[VideoCapture::X11])
    };
    let session = Session {
        wayland: false,
        display: Some(screen.display.clone()),
    };
    let choice = Choice {
        capture: VideoCapture::X11,
        encoder: VideoEncoder::Software,
    };
    let video = Video {
        fps: 10,
        ..Video::default()
    };

    let t = run_trial(&f, &choice, &video, &session, &work, 3);
    assert!(t.ok, "{}", t.message);
    assert_eq!((t.width, t.height), (640, 480));
    assert!(
        (2_000..=5_000).contains(&t.clip_ms),
        "a 3 s test gave {} ms",
        t.clip_ms
    );
    assert!(t.clip_bytes > 0 && t.took_ms >= 3_000);
    assert!(Path::new(t.clip.as_deref().unwrap()).is_file());
    assert!(t.message.contains("640×480"), "{}", t.message);
    assert!(
        !work.join("ring").exists(),
        "the pieces are cleaned up; only the clip is kept"
    );

    // no X server on this display: a plain failure, and the old clip is gone with the old test
    let gone = Session {
        wayland: false,
        display: Some(":90".into()),
    };
    let t = run_trial(&f, &choice, &video, &gone, &work, 3);
    assert!(!t.ok);
    assert!(!t.message.is_empty());
    assert_eq!(t.clip, None);
    assert!(!work.join(TRIAL_CLIP).exists());
}

#[test]
fn only_one_test_recording_runs_at_a_time_and_bad_settings_never_start_one() {
    let work = tempfile::tempdir().unwrap();
    let busy = TRIAL.lock().unwrap();
    let t = trial(
        &Video::default(),
        Os::Linux,
        &Session::default(),
        work.path(),
        1,
    );
    assert!(
        !t.ok && t.message.contains("already running"),
        "{}",
        t.message
    );
    drop(busy);

    let bad = Video {
        fps: 1,
        ..Video::default()
    };
    let t = trial(&bad, Os::Linux, &Session::default(), work.path(), 1);
    assert!(
        !t.ok && t.message.contains("Frames per second"),
        "{}",
        t.message
    );
    assert!(
        fs::read_dir(work.path()).unwrap().next().is_none(),
        "nothing was created"
    );
}
