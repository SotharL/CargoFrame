use super::*;
use crate::edit::{Crop, Overlay, Placed};
use std::process::Command;

const CLIP: Clip = Clip {
    has_audio: false,
    width: 320,
    height: 240,
    duration_ms: 4000,
};

fn strings(p: &Plan) -> Vec<String> {
    p.args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn after(args: &[String], flag: &str) -> String {
    let i = args
        .iter()
        .position(|a| a == flag)
        .unwrap_or_else(|| panic!("no {flag}"));
    args[i + 1].clone()
}

fn overlay(image: &str, x: i32, y: i32, from: u64, to: u64) -> Overlay {
    Overlay {
        image: image.into(),
        x,
        y,
        from_ms: from,
        to_ms: to,
    }
}

#[test]
fn the_plan_cuts_before_the_input_and_lays_marks_in_the_cut_clips_time() {
    let edit = Edit {
        start_ms: 1000,
        end_ms: Some(3000),
        crop: Some(Crop {
            x: 100,
            y: 50,
            w: 160,
            h: 120,
        }),
        overlays: vec![
            overlay("/tmp/a.png", 120, 70, 1500, 2500),
            overlay("/tmp/b.png", -4, 8, 0, 4000),
        ],
        ..Edit::default()
    };
    let p = plan(
        Path::new("/rec/in.mp4"),
        Path::new("/rec/out.mp4"),
        &edit,
        &CLIP,
    )
    .unwrap();
    let a = strings(&p);
    assert_eq!(after(&a, "-ss"), "1.000");
    assert_eq!(
        &a[a.iter().position(|x| x == "-i").unwrap() + 1],
        "file:/rec/in.mp4"
    );
    // the first -t belongs to the input, the second to the output
    assert_eq!(a.iter().filter(|x| *x == "-t").count(), 2);
    assert_eq!(after(&a, "-t"), "2.000");
    assert_eq!(a.iter().filter(|x| *x == "-i").count(), 3);
    assert!(
        a.contains(&"file:/tmp/a.png".to_string()) && a.contains(&"file:/tmp/b.png".to_string())
    );
    assert_eq!(
        after(&a, "-filter_complex"),
        "[0:v][1:v]overlay=x=120:y=70:format=auto:enable='between(t,0.500,1.500)'[o0];\
[o0][2:v]overlay=x=-4:y=8:format=auto:enable='between(t,0.000,2.000)'[o1];\
[o1]crop=160:120:100:50[out]"
    );
    assert_eq!(a.last().unwrap(), "file:/rec/out.mp4");
    assert_eq!(
        (p.length_ms, p.size, p.dropped_overlays),
        (2000, (160, 120), 0)
    );
}

#[test]
fn without_a_crop_the_picture_is_made_even() {
    let p = plan(
        Path::new("in.mp4"),
        Path::new("out.mp4"),
        &Edit::default(),
        &CLIP,
    )
    .unwrap();
    assert_eq!(
        after(&strings(&p), "-filter_complex"),
        "[0:v]crop=trunc(iw/2)*2:trunc(ih/2)*2:0:0[out]"
    );
    assert_eq!(p.length_ms, 4000);
}

#[test]
fn every_path_is_a_file_so_it_cannot_be_a_protocol_or_an_option() {
    let edit = Edit {
        overlays: vec![
            overlay("concat:/etc/passwd|x.png", 0, 0, 0, 1000),
            overlay("-evil.png", 0, 0, 0, 1000),
        ],
        ..Edit::default()
    };
    let p = plan(
        Path::new("-in.mp4"),
        Path::new("http://example.com/x.mp4"),
        &edit,
        &CLIP,
    )
    .unwrap();
    let a = strings(&p);
    for path in [
        "file:-in.mp4",
        "file:concat:/etc/passwd|x.png",
        "file:-evil.png",
        "file:http://example.com/x.mp4",
    ] {
        assert!(a.contains(&path.to_string()), "{path} in {a:?}");
    }
    // no argument is a bare protocol address
    assert!(a
        .iter()
        .all(|x| !x.starts_with("concat:") && !x.starts_with("http:")));
}

#[test]
fn a_bad_edit_makes_no_plan() {
    let edit = Edit {
        start_ms: 3900,
        ..Edit::default()
    };
    assert!(plan(Path::new("in.mp4"), Path::new("out.mp4"), &edit, &CLIP).is_err());
}

#[test]
fn marks_in_the_cut_part_are_counted() {
    let edit = Edit {
        start_ms: 2000,
        overlays: vec![overlay("a.png", 0, 0, 0, 1000)],
        ..Edit::default()
    };
    let p = plan(Path::new("in.mp4"), Path::new("out.mp4"), &edit, &CLIP).unwrap();
    assert_eq!(p.dropped_overlays, 1);
    assert!(!strings(&p).iter().any(|a| a.contains("a.png")));
}

#[test]
fn ffprobe_json_is_read() {
    let ok = r#"{"streams":[{"width":1920,"height":1080}],"format":{"duration":"10.013000"}}"#;
    assert_eq!(
        parse_probe(ok).unwrap(),
        Clip {
            width: 1920,
            height: 1080,
            duration_ms: 10_013,
            has_audio: false,
        }
    );
    // the sound is found among the streams, wherever the picture is
    let with_sound = r#"{"streams":[{"codec_type":"audio"},{"codec_type":"video","width":640,"height":360}],"format":{"duration":"2.5"}}"#;
    let clip = parse_probe(with_sound).unwrap();
    assert_eq!(
        (clip.width, clip.height, clip.duration_ms),
        (640, 360, 2_500)
    );
    assert!(clip.has_audio);
    let silent = r#"{"streams":[{"codec_type":"video","width":640,"height":360}],"format":{"duration":"2.5"}}"#;
    assert!(!parse_probe(silent).unwrap().has_audio);
    // a file with a sound and no picture is not a recording
    let only_sound = r#"{"streams":[{"codec_type":"audio"}],"format":{"duration":"2.5"}}"#;
    assert!(parse_probe(only_sound).is_err());
    for bad in [
        "not json",
        r#"{"streams":[],"format":{"duration":"1"}}"#,
        r#"{"streams":[{"width":0,"height":10}],"format":{"duration":"1"}}"#,
        r#"{"streams":[{"width":10,"height":10}],"format":{"duration":"N/A"}}"#,
        r#"{"streams":[{"width":10,"height":10}],"format":{}}"#,
        r#"{"streams":[{"width":10,"height":10}],"format":{"duration":"0"}}"#,
    ] {
        assert!(parse_probe(bad).is_err(), "{bad}");
    }
}

#[test]
fn programs_are_found_next_to_the_app_before_the_search_path() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(locate("cf-no-such-tool", &[dir.path().to_path_buf()]), None);
    let tool = dir.path().join("cf-tool");
    std::fs::write(&tool, b"x").unwrap();
    assert_eq!(locate("cf-tool", &[dir.path().to_path_buf()]), Some(tool));
    // a folder is not a program
    std::fs::create_dir(dir.path().join("cf-dir")).unwrap();
    assert_eq!(locate("cf-dir", &[dir.path().to_path_buf()]), None);
}

#[test]
fn a_missing_program_is_said_plainly() {
    let plan = Plan {
        args: vec![],
        length_ms: 0,
        size: (0, 0),
        dropped_overlays: 0,
    };
    let err = run(
        Path::new("/no/such/ffmpeg"),
        &plan,
        &|| false,
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::NoEncoder(_)));
    assert!(err.to_string().contains("Install ffmpeg"));
}

// ----- with a real ffmpeg (skipped where there is none) -----

fn tools() -> Option<(PathBuf, PathBuf)> {
    Some((locate("ffmpeg", &[])?, locate("ffprobe", &[])?))
}

fn make(ffmpeg: &Path, args: &[&str]) -> bool {
    Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The colour of one pixel of the frame at `at` seconds.
fn pixel(ffmpeg: &Path, file: &Path, at: &str, w: usize, x: usize, y: usize) -> [u8; 3] {
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-ss", at, "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let i = (y * w + x) * 3;
    [out.stdout[i], out.stdout[i + 1], out.stdout[i + 2]]
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 70 && p[2] < 70
}

#[test]
fn a_real_export_cuts_crops_and_shows_the_mark_only_while_it_should() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        eprintln!("skipped: ffmpeg and ffprobe are not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (clip, mark, out) = (
        dir.path().join("in.mp4"),
        dir.path().join("mark.png"),
        dir.path().join("out.mp4"),
    );
    // a plain blue 4 s clip, and a red square with no transparency
    if !make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:r=15:d=4",
            "-pix_fmt",
            "yuv420p",
            clip.to_str().unwrap(),
        ],
    ) || !make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=0xFF0000:s=40x40,format=rgba",
            "-frames:v",
            "1",
            mark.to_str().unwrap(),
        ],
    ) {
        eprintln!("skipped: this ffmpeg cannot make test pictures");
        return;
    }
    let info = probe(&ffprobe, &clip).unwrap();
    assert_eq!((info.width, info.height), (320, 240));
    assert!((3900..=4100).contains(&info.duration_ms), "{info:?}");

    // cut 1 s..3 s, crop to the 160x120 square at (100,50); the mark sits at (120,70) on the
    // full picture from 1.5 s to 2.5 s, so in the output at (20,20), from 0.5 s to 1.5 s
    let edit = Edit {
        start_ms: 1000,
        end_ms: Some(3000),
        crop: Some(Crop {
            x: 100,
            y: 50,
            w: 160,
            h: 120,
        }),
        overlays: vec![Overlay {
            image: mark.clone(),
            x: 120,
            y: 70,
            from_ms: 1500,
            to_ms: 2500,
        }],
        ..Edit::default()
    };
    let p = plan(&clip, &out, &edit, &info).unwrap();
    run(&ffmpeg, &p, &|| false, Duration::from_secs(60)).unwrap();

    let result = probe(&ffprobe, &out).unwrap();
    assert_eq!((result.width, result.height), (160, 120));
    assert!((1850..=2150).contains(&result.duration_ms), "{result:?}");

    // inside the square and the time: red. Before and after: the blue of the clip.
    assert!(
        is_red(pixel(&ffmpeg, &out, "1.0", 160, 40, 40)),
        "no mark at 1.0 s"
    );
    assert!(
        !is_red(pixel(&ffmpeg, &out, "0.2", 160, 40, 40)),
        "mark too early"
    );
    assert!(
        !is_red(pixel(&ffmpeg, &out, "1.8", 160, 40, 40)),
        "mark too late"
    );
    // outside the square the clip is blue even while the mark shows
    let outside = pixel(&ffmpeg, &out, "1.0", 160, 120, 100);
    assert!(outside[2] > 150 && outside[0] < 80, "{outside:?}");
}

#[test]
fn an_odd_sized_clip_is_exported_even() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        eprintln!("skipped: ffmpeg and ffprobe are not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (clip, out) = (dir.path().join("odd.mkv"), dir.path().join("out.mp4"));
    // a lossless clip in 4:4:4 keeps its odd size; the colour source would round it
    if !make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=321x241:r=10:d=2,format=yuv444p",
            "-c:v",
            "ffv1",
            clip.to_str().unwrap(),
        ],
    ) {
        eprintln!("skipped: this ffmpeg cannot make an odd-sized test clip");
        return;
    }
    let info = probe(&ffprobe, &clip).unwrap();
    assert_eq!((info.width, info.height), (321, 241));
    let p = plan(&clip, &out, &Edit::default(), &info).unwrap();
    assert_eq!(p.size, (320, 240));
    run(&ffmpeg, &p, &|| false, Duration::from_secs(60)).unwrap();
    let result = probe(&ffprobe, &out).unwrap();
    assert_eq!((result.width, result.height), (320, 240));
}

#[test]
fn the_sound_of_a_recording_is_kept_when_there_is_one_and_left_out_when_not() {
    let with = Clip {
        has_audio: true,
        ..CLIP
    };
    let a = strings(
        &plan(
            Path::new("/in.mp4"),
            Path::new("/out.mp4"),
            &Edit::default(),
            &with,
        )
        .unwrap(),
    );
    assert!(a.windows(2).any(|w| w == ["-map", "0:a:0"]));
    assert_eq!(after(&a, "-c:a"), "aac");
    assert!(!a.iter().any(|x| x == "-an"));
    let a = strings(
        &plan(
            Path::new("/in.mp4"),
            Path::new("/out.mp4"),
            &Edit::default(),
            &CLIP,
        )
        .unwrap(),
    );
    assert!(a.iter().any(|x| x == "-an"));
    assert!(!a.iter().any(|x| x == "-c:a" || x == "0:a:0"));
}

#[test]
fn a_real_export_keeps_the_sound_and_cuts_it_with_the_picture() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        eprintln!("skipped: ffmpeg and ffprobe are not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (clip, out) = (dir.path().join("sound.mp4"), dir.path().join("out.mp4"));
    // a recording as the recorder leaves it: H.264 and AAC in an MP4 that starts at zero
    if !make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x240:r=10:d=4",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=4",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            clip.to_str().unwrap(),
        ],
    ) {
        eprintln!("skipped: this ffmpeg cannot make a test recording with sound");
        return;
    }
    let info = probe(&ffprobe, &clip).unwrap();
    assert!(info.has_audio);
    let edit = Edit {
        start_ms: 1000,
        end_ms: Some(3000),
        ..Edit::default()
    };
    let p = plan(&clip, &out, &edit, &info).unwrap();
    run(&ffmpeg, &p, &|| false, Duration::from_secs(60)).unwrap();
    let result = probe(&ffprobe, &out).unwrap();
    assert!(result.has_audio, "the exported clip has no sound");
    assert!(
        (1_800..=2_300).contains(&result.duration_ms),
        "{}",
        result.duration_ms
    );
}

#[test]
fn a_failed_export_says_why_and_a_cancelled_or_slow_one_is_stopped() {
    let Some((ffmpeg, _)) = tools() else {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    // an input that is not there
    let p = plan(
        &dir.path().join("missing.mp4"),
        &dir.path().join("o.mp4"),
        &Edit::default(),
        &CLIP,
    )
    .unwrap();
    let err = run(&ffmpeg, &p, &|| false, Duration::from_secs(30)).unwrap_err();
    assert!(
        matches!(&err, ExportError::Failed(m) if m.contains("No such file")),
        "{err}"
    );

    // something that would run for minutes
    let long = Plan {
        args: [
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=1280x720:r=30:d=600",
            "-f",
            "null",
            "-",
        ]
        .iter()
        .map(OsString::from)
        .collect(),
        length_ms: 0,
        size: (0, 0),
        dropped_overlays: 0,
    };
    let started = Instant::now();
    assert_eq!(
        run(&ffmpeg, &long, &|| true, Duration::from_secs(60)).unwrap_err(),
        ExportError::Cancelled
    );
    assert_eq!(
        run(&ffmpeg, &long, &|| false, Duration::from_millis(300)).unwrap_err(),
        ExportError::TimedOut
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "stopping took {:?}",
        started.elapsed()
    );
}

#[test]
fn a_moving_crop_slides_from_place_to_place() {
    let places = [
        Placed {
            at_ms: 1_000,
            x: 10,
            y: 20,
        },
        Placed {
            at_ms: 1_500,
            x: 110,
            y: 20,
        },
        Placed {
            at_ms: 1_500,
            x: 120,
            y: 40,
        },
    ];
    // on the sent clip, which starts at 0.5 s of the recording
    assert_eq!(
        moving(&places, 500, |p| p.x),
        "10+(100)*clip((t-(0.500))/0.500,0,1)+(10)*gte(t,1.000)"
    );
    assert_eq!(moving(&places, 500, |p| p.y), "20+(20)*gte(t,1.000)");
}

#[test]
fn the_keys_list_fills_the_gaps_with_the_blank_picture() {
    let blank = Path::new("/w/blank.png");
    let shown = vec![
        (PathBuf::from("/w/k0.png"), 500, 1_500),
        (PathBuf::from("/w/it's.png"), 1_500, 2_000),
        // after the end: left out
        (PathBuf::from("/w/k2.png"), 5_000, 6_000),
    ];
    let list = keys_list(blank, &shown, 1_000, 3_000);
    assert_eq!(
        list,
        "ffconcat version 1.0\n\
         file 'file:/w/k0.png'\nduration 0.500\n\
         file 'file:/w/it'\\''s.png'\nduration 0.500\n\
         file 'file:/w/blank.png'\nduration 1.000\n\
         file 'file:/w/blank.png'\n"
    );
}

#[test]
fn texts_come_after_the_crop_on_the_sent_picture_one_over_the_other() {
    let edit = Edit {
        crop: Some(Crop {
            x: 100,
            y: 50,
            w: 160,
            h: 120,
        }),
        layers: vec![
            crate::edit::TextLayer {
                list: "/w/keys.txt".into(),
                x: 8,
                y: 90,
            },
            crate::edit::TextLayer {
                list: "/w/clock.txt".into(),
                x: 100,
                y: 4,
            },
        ],
        overlays: vec![overlay("/w/mark.png", 0, 0, 0, 4_000)],
        ..Edit::default()
    };
    let p = plan(
        Path::new("/w/in.mp4"),
        Path::new("/w/out.mp4"),
        &edit,
        &CLIP,
    )
    .unwrap();
    let a = strings(&p);
    let graph = after(&a, "-filter_complex");
    assert!(
        graph.ends_with("[o0]crop=160:120:100:50[t0];[t0][2:v]overlay=x=8:y=90:format=auto:eof_action=pass[t1];[t1][3:v]overlay=x=100:y=4:format=auto:eof_action=pass[out]"),
        "{graph}"
    );
    for list in ["file:/w/keys.txt", "file:/w/clock.txt"] {
        let i = a.iter().position(|x| x == list).unwrap();
        assert_eq!(a[i - 5..i], ["-f", "concat", "-safe", "0", "-i"]);
    }
}

#[test]
fn a_real_export_follows_the_moved_window_and_shows_the_keys_in_time() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        eprintln!("skipped: ffmpeg and ffprobe are not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let p = |n: &str| dir.path().join(n);
    // a blue clip with a red square that jumps from (20,20) to (200,20) at 2 s
    let made = make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:r=15:d=4[b];color=c=red:s=40x40:r=15:d=4[r];[b][r]overlay=x='if(lt(t,2),20,200)':y=20",
            "-pix_fmt",
            "yuv420p",
            p("in.mp4").to_str().unwrap(),
        ],
    ) && make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=black@0.0:s=60x20,format=rgba",
            "-frames:v",
            "1",
            p("blank.png").to_str().unwrap(),
        ],
    ) && make(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=0x00FF00:s=60x20,format=rgba",
            "-frames:v",
            "1",
            p("key.png").to_str().unwrap(),
        ],
    );
    if !made {
        eprintln!("skipped: this ffmpeg cannot make test pictures");
        return;
    }
    let info = probe(&ffprobe, &p("in.mp4")).unwrap();
    std::fs::write(
        p("keys.txt"),
        keys_list(&p("blank.png"), &[(p("key.png"), 1_000, 2_000)], 0, 4_000),
    )
    .unwrap();
    // an 80x80 crop on the square, which moves with it between 1.9 s and 2.0 s
    let edit = Edit {
        crop: Some(Crop {
            x: 0,
            y: 0,
            w: 80,
            h: 80,
        }),
        follow: vec![
            Placed {
                at_ms: 0,
                x: 0,
                y: 0,
            },
            Placed {
                at_ms: 1_900,
                x: 0,
                y: 0,
            },
            Placed {
                at_ms: 2_000,
                x: 180,
                y: 0,
            },
        ],
        layers: vec![crate::edit::TextLayer {
            list: p("keys.txt"),
            x: 10,
            y: 60,
        }],
        ..Edit::default()
    };
    let plan = plan(&p("in.mp4"), &p("out.mp4"), &edit, &info).unwrap();
    run(&ffmpeg, &plan, &|| false, Duration::from_secs(60)).unwrap();
    let out = probe(&ffprobe, &p("out.mp4")).unwrap();
    assert_eq!((out.width, out.height), (80, 80));
    // the square is in the crop before and after it moved
    for at in ["0.5", "3.0"] {
        assert!(
            is_red(pixel(&ffmpeg, &p("out.mp4"), at, 80, 40, 40)),
            "the crop lost the window at {at} s"
        );
    }
    // the key shows from 1 s to 2 s, at (10,60) on the sent picture
    let green = |c: [u8; 3]| c[1] > 180 && c[0] < 90 && c[2] < 90;
    assert!(
        green(pixel(&ffmpeg, &p("out.mp4"), "1.5", 80, 20, 70)),
        "no key at 1.5 s"
    );
    for at in ["0.5", "2.5"] {
        assert!(
            !green(pixel(&ffmpeg, &p("out.mp4"), at, 80, 20, 70)),
            "a key at {at} s"
        );
    }
    // with the first second cut, both keep time with the picture
    std::fs::write(
        p("keys.txt"),
        keys_list(
            &p("blank.png"),
            &[(p("key.png"), 1_000, 2_000)],
            1_000,
            4_000,
        ),
    )
    .unwrap();
    let cut = Edit {
        start_ms: 1_000,
        ..edit
    };
    let plan = super::plan(&p("in.mp4"), &p("cut.mp4"), &cut, &info).unwrap();
    run(&ffmpeg, &plan, &|| false, Duration::from_secs(60)).unwrap();
    assert!(
        green(pixel(&ffmpeg, &p("cut.mp4"), "0.5", 80, 20, 70)),
        "no key at 0.5 s of the cut clip"
    );
    assert!(
        !green(pixel(&ffmpeg, &p("cut.mp4"), "1.5", 80, 20, 70)),
        "a key at 1.5 s of the cut clip"
    );
    for at in ["0.2", "2.0"] {
        assert!(
            is_red(pixel(&ffmpeg, &p("cut.mp4"), at, 80, 40, 40)),
            "the cut clip lost the window at {at} s"
        );
    }
}
