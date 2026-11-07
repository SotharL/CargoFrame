use super::*;
use cargoframe_video::pcm::{PcmFormat, PcmSource};
use cargoframe_video::{locate, Crop};
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
fn a_test_recording_of_a_real_screen_gives_a_clip_and_a_report() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let Some(ffprobe) = locate("ffprobe", &[]) else {
        eprintln!("skipped: ffprobe is not installed");
        return;
    };
    let _one = one_at_a_time();
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

    let rec = Recording::new();
    rec.start_with(
        f.clone(),
        choice.clone(),
        &video,
        &session,
        &work,
        trial_window(3),
    )
    .unwrap();
    let t = rec.run_for(3);
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
    assert_eq!(rec.view(), RecordingView::Idle);

    // no X server on this display: a plain failure, and the old clip is gone with the old test
    let gone = Session {
        wayland: false,
        display: Some(":90".into()),
    };
    rec.start_with(
        f.clone(),
        choice.clone(),
        &video,
        &gone,
        &work,
        trial_window(3),
    )
    .unwrap();
    let t = rec.run_for(3);
    assert!(!t.ok);
    assert!(!t.message.is_empty());
    assert_eq!(t.clip, None);
    assert!(!work.join(TRIAL_CLIP).exists());
    assert_eq!(rec.view(), RecordingView::Idle, "told once, then idle");
}

#[test]
fn a_recording_started_by_hand_keeps_only_the_last_seconds_until_it_is_stopped() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let Some(ffprobe) = locate("ffprobe", &[]) else {
        eprintln!("skipped: ffprobe is not installed");
        return;
    };
    let _one = one_at_a_time();
    let Some(screen) = Screen::start(92) else {
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
    let rec = Recording::new();
    assert_eq!(rec.view(), RecordingView::Idle);
    assert!(rec.stop().message.contains("No recording is running"));

    rec.start_with(f.clone(), choice.clone(), &video, &session, &work, 2)
        .unwrap();
    assert!(rec
        .start_with(f.clone(), choice.clone(), &video, &session, &work, 2)
        .unwrap_err()
        .contains("already running"));
    std::thread::sleep(Duration::from_millis(5_500));
    let RecordingView::Recording {
        elapsed_ms,
        buffered_ms,
        keep_secs,
        ..
    } = rec.view()
    else {
        panic!("it should be recording: {:?}", rec.view());
    };
    assert!(elapsed_ms >= 5_000, "{elapsed_ms}");
    assert_eq!(keep_secs, 2);
    assert!(
        buffered_ms <= 2_500,
        "{buffered_ms} ms are kept of a 2 s window"
    );

    let t = rec.stop();
    assert!(t.ok, "{}", t.message);
    assert!(
        (1_500..=4_000).contains(&t.clip_ms),
        "5 s were recorded, 2 s kept, and the clip is {} ms",
        t.clip_ms
    );
    assert!(
        t.message.contains("only the last 2 s are kept"),
        "{}",
        t.message
    );
    assert_eq!(rec.view(), RecordingView::Idle);
    // cancelling when nothing runs is fine, and the folder is left tidy
    rec.cancel();
    assert!(!work.join("ring").exists());
}

#[test]
fn bad_settings_never_start_a_recording_and_a_cancelled_one_leaves_nothing() {
    let work = tempfile::tempdir().unwrap();
    let rec = Recording::new();
    let bad = Video {
        fps: 1,
        ..Video::default()
    };
    let t = rec.trial(&bad, Os::Linux, &Session::default(), work.path(), 1);
    assert!(
        !t.ok && t.message.contains("Frames per second"),
        "{}",
        t.message
    );
    assert!(
        fs::read_dir(work.path()).unwrap().next().is_none(),
        "nothing was created"
    );
    assert_eq!(rec.view(), RecordingView::Idle);
    rec.cancel();
}

#[test]
fn a_clip_is_sent_to_the_window_only_when_it_is_there_and_not_huge() {
    let dir = tempfile::tempdir().unwrap();
    let missing = read_clip(&dir.path().join(TRIAL_CLIP), PREVIEW_LIMIT).unwrap_err();
    assert!(missing.contains("no test clip yet"), "{missing}");
    // a folder is not a clip
    let err = read_clip(dir.path(), PREVIEW_LIMIT).unwrap_err();
    assert!(err.contains("no test clip yet"), "{err}");

    let clip = dir.path().join(TRIAL_CLIP);
    fs::write(&clip, [1u8; 2048]).unwrap();
    assert_eq!(read_clip(&clip, PREVIEW_LIMIT).unwrap().len(), 2048);
    let err = read_clip(&clip, 1024).unwrap_err();
    assert!(
        err.contains("too large") && err.contains("video player"),
        "{err}"
    );
}

fn make_test_clip(ffmpeg: &Path, work: &Path) {
    fs::create_dir_all(work).unwrap();
    let out = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc=s=320x240:r=10:d=3")
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(work.join(TRIAL_CLIP))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_test_clip_is_cut_cropped_and_marked_into_a_copy_and_bad_edits_are_explained() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    if locate("ffprobe", &[]).is_none() {
        eprintln!("skipped: ffprobe is not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("video-test");
    let idle = Recording::new();
    let video = Video::default();

    let edit = TestEdit {
        start_ms: 500,
        end_ms: Some(2_500),
        crop: Some(Crop {
            x: 0,
            y: 0,
            w: 160,
            h: 120,
        }),
        mark: TestMark::Square,
    };
    // nothing recorded yet
    let t = export_test(&video, &edit, &idle, &work);
    assert!(
        !t.ok && t.message.contains("no test clip yet"),
        "{}",
        t.message
    );

    make_test_clip(&ffmpeg, &work);
    let t = export_test(&video, &edit, &idle, &work);
    assert!(t.ok, "{}", t.message);
    assert_eq!((t.width, t.height), (160, 120));
    assert!(
        (1_500..=2_500).contains(&t.clip_ms),
        "a 2 s cut gave {} ms",
        t.clip_ms
    );
    assert!(Path::new(t.clip.as_deref().unwrap()).is_file());
    assert!(work.join("mark.png").is_file(), "the test mark was drawn");

    // without a crop or a mark, the whole picture
    let plain = TestEdit {
        start_ms: 0,
        end_ms: None,
        crop: None,
        mark: TestMark::None,
    };
    let t = export_test(&video, &plain, &idle, &work);
    assert!(t.ok, "{}", t.message);
    assert_eq!((t.width, t.height), (320, 240));

    // problems are said in words
    let too_short = TestEdit {
        end_ms: Some(100),
        ..plain.clone()
    };
    let t = export_test(&video, &too_short, &idle, &work);
    assert!(!t.ok && t.message.contains("at least"), "{}", t.message);
    let outside = TestEdit {
        crop: Some(Crop {
            x: 300,
            y: 0,
            w: 100,
            h: 100,
        }),
        ..plain
    };
    let t = export_test(&video, &outside, &idle, &work);
    assert!(!t.ok && t.message.contains("outside"), "{}", t.message);
}

#[test]
fn a_mark_the_page_drew_is_checked_kept_and_laid_over_the_copy() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    if locate("ffprobe", &[]).is_none() {
        eprintln!("skipped: ffprobe is not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("video-test");
    let idle = Recording::new();
    let video = Video::default();

    // nowhere to keep it before a test clip exists, and only a PNG is accepted
    assert!(store_mark(
        &work,
        Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
    )
    .unwrap_err()
    .contains("no test clip yet"));
    make_test_clip(&ffmpeg, &work);
    assert!(store_mark(&work, Some(b"GIF89a not a png"))
        .unwrap_err()
        .contains("not a PNG"));
    assert!(!work.join(DRAWN_MARK).exists());

    let drawn = TestEdit {
        start_ms: 0,
        end_ms: None,
        crop: None,
        mark: TestMark::Drawn,
    };
    // asked to lay a mark that was never drawn
    let t = export_test(&video, &drawn, &idle, &work);
    assert!(!t.ok && t.message.contains("not drawn"), "{}", t.message);

    // a see-through picture the size of the clip, as the page will draw it
    let png = dir.path().join("drawn.png");
    let made = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("color=c=blue@0.5:s=320x240,format=rgba")
        .args(["-frames:v", "1"])
        .arg(&png)
        .output()
        .unwrap();
    assert!(made.status.success());
    store_mark(&work, Some(&fs::read(&png).unwrap())).unwrap();
    assert!(work.join(DRAWN_MARK).is_file());

    let cropped = TestEdit {
        crop: Some(Crop {
            x: 80,
            y: 60,
            w: 160,
            h: 120,
        }),
        ..drawn.clone()
    };
    let t = export_test(&video, &cropped, &idle, &work);
    assert!(t.ok, "{}", t.message);
    assert_eq!((t.width, t.height), (160, 120), "the crop comes last");

    // forgetting it
    store_mark(&work, None).unwrap();
    assert!(!work.join(DRAWN_MARK).exists());
    store_mark(&work, None).unwrap();
}

// ----- a press of a video trigger -----

#[test]
fn a_press_starts_when_nothing_records_and_stops_what_a_trigger_started() {
    let recording = RecordingView::Recording {
        elapsed_ms: 4_000,
        buffered_ms: 4_000,
        squeezed: false,
        keep_secs: 10,
    };
    assert_eq!(step(&RecordingView::Idle, false), Step::Start);
    assert_eq!(step(&RecordingView::Idle, true), Step::Start);
    assert_eq!(step(&recording, true), Step::Stop);
    assert!(matches!(step(&recording, false), Step::Refuse(m) if m.contains("Settings")));
    assert!(matches!(
        step(&RecordingView::Finishing, true),
        Step::Refuse(_)
    ));
}

#[test]
fn a_recording_that_ended_by_itself_is_reported_to_the_trigger_that_started_it() {
    let failed = RecordingView::Failed {
        message: "the screen could not be read".into(),
    };
    assert_eq!(
        step(&failed, true),
        Step::Report("the screen could not be read".into())
    );
    // a failure left by a test in Settings is no business of a trigger: it just starts
    assert_eq!(step(&failed, false), Step::Start);
}

#[test]
fn a_start_for_a_trigger_that_fails_leaves_nothing_marked_as_a_trigger_recording() {
    let rec = Recording::new();
    assert!(!rec.started_by_trigger());
    let video = Video {
        ffmpeg: Some("/no/such/ffmpeg".into()),
        ..Video::default()
    };
    let session = Session {
        wayland: false,
        display: None,
    };
    let dir = tempfile::tempdir().unwrap();
    // ffmpeg is not there, so neither start works, and neither leaves the flag set
    assert!(rec
        .start_for_trigger(&video, Os::Linux, &session, dir.path(), 5, &Wish::default())
        .is_err());
    assert!(!rec.started_by_trigger());
}

#[test]
fn a_recording_from_a_trigger_becomes_a_session_whose_clip_plays_back() {
    use crate::session::tests::{fake_capture, no_connections, NOW};
    use crate::session::{prepare_video, ClipInfo};
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let Some(ffprobe) = locate("ffprobe", &[]) else {
        eprintln!("skipped: ffprobe is not installed");
        return;
    };
    let _one = one_at_a_time();
    let Some(screen) = Screen::start(88) else {
        eprintln!("skipped: no virtual X screen (Xvfb) to record");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("video-recording");
    let f = Found {
        ffmpeg,
        ffprobe: ffprobe.clone(),
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
    let rec = Recording::new();
    rec.start_with(f, choice, &video, &session, &work, 2)
        .unwrap();
    rec.by_trigger
        .store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(3_500));
    // the next press stops it
    assert_eq!(step(&rec.view(), rec.started_by_trigger()), Step::Stop);
    let trial = rec.stop();
    assert!(trial.ok, "{}", trial.message);

    let store = cargoframe_config::Store::new(cargoframe_config::Dirs::under(dir.path()));
    let workspace = cargoframe_bundle::Workspace::new(dir.path().join("captures"));
    let trigger = cargoframe_config::Trigger {
        media: cargoframe_config::Media::Video,
        ..cargoframe_config::Trigger::default()
    };
    let made = prepare_video(
        &store,
        &no_connections(),
        &workspace,
        "t",
        &trigger,
        fake_capture(None),
        NOW,
        1,
        std::path::Path::new(trial.clip.as_deref().expect("a clip")),
        ClipInfo {
            width: trial.width,
            height: trial.height,
            duration_ms: trial.clip_ms,
            bytes: trial.clip_bytes,
            has_audio: trial.clip_audio,
        },
    )
    .unwrap();
    let clip = made.clip.clone().expect("the session has the clip");
    // the recording work folder keeps nothing a new recording could wipe: the clip moved
    assert!(!work.join(TRIAL_CLIP).exists());
    let info = cargoframe_video::probe(&ffprobe, &clip.path).unwrap();
    assert_eq!((info.width, info.height), (640, 480));
    assert_eq!(info.duration_ms, clip.duration_ms);
    // and what `get_session_clip` sends is the whole file
    let bytes = read_clip(&clip.path, PREVIEW_LIMIT).unwrap();
    assert_eq!(bytes.len() as u64, clip.bytes);
}

// ----- what a layout's video block asks for -----

fn layout_with(video: cargoframe_layout::VideoBlock) -> Layout {
    let mut l = Layout::empty("l", "l");
    l.blocks.push(cargoframe_layout::Block {
        id: "v".into(),
        rect: cargoframe_layout::Rect {
            x: 0,
            y: 0,
            w: 12,
            h: 6,
        },
        kind: BlockKind::Video(video),
    });
    l
}

#[test]
fn a_layout_decides_whether_the_screen_or_the_window_in_focus_is_recorded() {
    use cargoframe_layout::{AudioChoice, VideoBlock, VideoSubject};
    let never = || -> Option<Focus> { panic!("the window in focus was looked for when not asked") };
    // no layout, or no video block: the whole screen, with the computer's sound
    let (wish, notes) = wish_for(None, &never);
    assert_eq!(wish.area, None);
    assert_eq!(wish.audio, Some(VideoAudio::default()));
    assert!(notes.is_empty());
    let (wish, _) = wish_for(Some(&Layout::empty("l", "l")), &never);
    assert_eq!(wish.area, None);
    let (wish, _) = wish_for(Some(&layout_with(VideoBlock::default())), &never);
    assert_eq!(wish.area, None);

    let window = layout_with(VideoBlock {
        subject: VideoSubject::FocusedWindow,
        audio: VideoAudio {
            desktop: AudioChoice::Off,
            microphone: AudioChoice::Default,
            ..VideoAudio::default()
        },
        ..VideoBlock::default()
    });
    let place = Area {
        x: 40,
        y: 30,
        w: 800,
        h: 600,
    };
    let focus = |area: Area| Focus {
        area,
        program: Some(77),
    };
    let (wish, notes) = wish_for(Some(&window), &|| Some(focus(place)));
    assert_eq!(wish.area, Some(place));
    // the program is only passed on when its sound is asked for
    assert_eq!(wish.program, None);
    assert_eq!(wish.audio.unwrap().microphone, AudioChoice::Default);
    assert!(notes.is_empty());

    // a window that cannot be found, or is a stub, falls back to the screen and says so
    let (wish, notes) = wish_for(Some(&window), &|| None);
    assert_eq!(wish.area, None);
    assert!(
        notes[0].contains("could not tell which window"),
        "{notes:?}"
    );
    let stub = Area {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
    };
    let (wish, notes) = wish_for(Some(&window), &|| Some(focus(stub)));
    assert_eq!(wish.area, None);
    assert!(notes[0].contains("too small"), "{notes:?}");
}

#[test]
fn a_layout_that_asks_for_one_programs_sound_gets_the_program_in_focus() {
    use cargoframe_layout::{DesktopSound, VideoBlock};
    let one = layout_with(VideoBlock {
        audio: VideoAudio {
            desktop_from: DesktopSound::FocusedProgram,
            ..VideoAudio::default()
        },
        ..VideoBlock::default()
    });
    let place = Area {
        x: 0,
        y: 0,
        w: 800,
        h: 600,
    };
    let focus = |program| Focus {
        area: place,
        program,
    };
    // the screen is recorded, but the focus is still looked at: for the program
    let (wish, notes) = wish_for(Some(&one), &|| Some(focus(Some(4242))));
    assert_eq!(wish.area, None);
    assert_eq!(wish.program, Some(4242));
    assert!(notes.is_empty());
    // not found: nothing to pass on (the sound plan says what happens then)
    assert_eq!(wish_for(Some(&one), &|| Some(focus(None))).0.program, None);
    assert_eq!(wish_for(Some(&one), &|| None).0.program, None);
    // a desktop sound that is off has no program to look for
    let off = layout_with(VideoBlock {
        audio: VideoAudio {
            desktop: cargoframe_layout::AudioChoice::Off,
            desktop_from: DesktopSound::FocusedProgram,
            ..VideoAudio::default()
        },
        ..VideoBlock::default()
    });
    let never = || -> Option<Focus> { panic!("the focus was looked for when nothing needed it") };
    assert_eq!(wish_for(Some(&off), &never).0.program, None);
}

/// A sound source that hears nothing, however long it waits: a computer that plays nothing.
struct Quiet;

impl PcmSource for Quiet {
    fn format(&self) -> PcmFormat {
        PcmFormat {
            rate: 48_000,
            channels: 2,
        }
    }

    fn read(&mut self, _out: &mut Vec<u8>, wait: Duration) -> Result<(), String> {
        std::thread::sleep(wait);
        Ok(())
    }
}

fn hears_nothing(_program: Option<u32>) -> Result<Feed, String> {
    Feed::open(|| Ok(Box::new(Quiet)))
}

fn cannot_hear(_program: Option<u32>) -> Result<Feed, String> {
    Err("no sound output was found".into())
}

fn computer() -> AudioSource {
    AudioSource {
        backend: AudioBackend::Loopback { program: None },
        device: "computer".into(),
    }
}

#[test]
fn a_sound_cargoframe_listens_to_becomes_an_input_that_ffmpeg_reads_from_a_local_port() {
    let microphone = AudioSource {
        backend: AudioBackend::Dshow,
        device: "USB Mic".into(),
    };
    let mut notes = Vec::new();
    let (inputs, feeds) = open_sounds(&[computer(), microphone.clone()], hears_nothing, &mut notes);
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(feeds.len(), 1);
    // the computer's sound is now raw samples on a port; the microphone is untouched
    assert_eq!(
        inputs[0].backend,
        AudioBackend::Pcm {
            port: feeds[0].port(),
            rate: 48_000,
            channels: 2
        }
    );
    assert_eq!(inputs[0].device, "computer");
    assert_eq!(inputs[1], microphone);
}

#[test]
fn a_sound_that_cannot_be_listened_to_is_left_out_and_said_and_the_rest_stays() {
    let microphone = AudioSource {
        backend: AudioBackend::Dshow,
        device: "USB Mic".into(),
    };
    let mut notes = Vec::new();
    let (inputs, feeds) = open_sounds(&[computer(), microphone.clone()], cannot_hear, &mut notes);
    assert!(feeds.is_empty());
    assert_eq!(inputs, [microphone]);
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("sound of the computer"), "{notes:?}");
    assert!(notes[0].contains("no sound output was found"), "{notes:?}");

    // one program's sound is named as that
    let mut notes = Vec::new();
    let one = AudioSource {
        backend: AudioBackend::Loopback {
            program: Some(4242),
        },
        device: "program 4242".into(),
    };
    open_sounds(&[one], cannot_hear, &mut notes);
    assert!(notes[0].contains("program in focus"), "{notes:?}");
}

#[test]
fn a_recording_with_a_sound_cargoframe_listens_to_has_a_sound_track() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let Some(ffprobe) = locate("ffprobe", &[]) else {
        eprintln!("skipped: ffprobe is not installed");
        return;
    };
    let _one = one_at_a_time();
    let Some(screen) = Screen::start(89) else {
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
    let extras = Extras {
        sources: vec![computer()],
        patience: Duration::from_millis(600),
        listen: Some(hears_nothing),
        ..Extras::default()
    };
    let rec = Recording::new();
    let started = rec
        .start_with_extras(f, choice, &video, &session, &work, trial_window(3), &extras)
        .unwrap();
    // a computer that plays nothing still gives a recording with a sound track
    assert!(started.sound);
    assert!(started.notes.is_empty(), "{:?}", started.notes);
    let t = rec.run_for(3);
    assert!(t.ok, "{}", t.message);
    assert!(t.clip_audio, "the clip has no sound track");
}

#[test]
fn a_window_is_recorded_on_its_own_and_a_missing_sound_device_costs_only_the_sound() {
    let Some(ffmpeg) = locate("ffmpeg", &[]) else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let Some(ffprobe) = locate("ffprobe", &[]) else {
        eprintln!("skipped: ffprobe is not installed");
        return;
    };
    let _one = one_at_a_time();
    let Some(screen) = Screen::start(93) else {
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
    let extras = Extras {
        area: Some(Area {
            x: 100,
            y: 50,
            w: 321,
            h: 241,
        }),
        sources: vec![AudioSource {
            backend: recorder::AudioBackend::Pulse,
            device: "cargoframe-no-such-device".into(),
        }],
        // a sound system that is not there is slow to say so
        patience: Duration::from_secs(8),
        listen: None,
    };
    let rec = Recording::new();
    let started = rec
        .start_with_extras(f, choice, &video, &session, &work, trial_window(3), &extras)
        .unwrap();
    // the device is not there: the recording goes on without it and says so
    assert!(!started.sound);
    assert!(
        started
            .notes
            .iter()
            .any(|n| n.contains("The sound could not be recorded")),
        "{:?}",
        started.notes
    );
    let t = rec.run_for(2);
    assert!(t.ok, "{}", t.message);
    // the window only, with even sides
    assert_eq!((t.width, t.height), (320, 240));
    assert!(!t.clip_audio);
}
