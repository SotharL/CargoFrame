use super::*;
use crate::edit::{Crop, Overlay};
use std::process::Command;

const CLIP: Clip = Clip {
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
            duration_ms: 10_013
        }
    );
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
