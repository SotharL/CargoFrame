use super::*;
use crate::edit::Edit;
use crate::export::{locate, plan, probe, run};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Write `n` fake segments of 100 bytes, one second each, and the list lines for them.
fn fake(dir: &Path, n: usize) -> String {
    let mut list = String::new();
    for i in 0..n {
        fs::write(dir.join(format!("seg{i:05}.ts")), [7u8; 100]).unwrap();
        list.push_str(&format!("seg{i:05}.ts,{}.000000,{}.000000\n", i, i + 1));
    }
    list
}

fn exists(dir: &Path, i: usize) -> bool {
    dir.join(format!("seg{i:05}.ts")).exists()
}

#[test]
fn list_lines_are_read_only_when_they_are_plain() {
    assert_eq!(
        parse_line("seg00003.ts,3.000000,4.001000\n"),
        Some(Entry {
            name: "seg00003.ts",
            start: 3.0,
            end: 4.001
        })
    );
    for bad in [
        "",
        "seg.ts",
        "seg.ts,1",
        "seg.ts,a,b",
        "seg.ts,2,1",
        "seg.ts,-1,1",
        "../seg.ts,0,1",
        "/etc/passwd.ts,0,1",
        "C:\\x.ts,0,1",
        "a/b.ts,0,1",
        "..,0,1",
        "seg.mp4,0,1",
        "seg.ts,0,NaN",
    ] {
        assert_eq!(parse_line(bad), None, "{bad:?}");
    }
}

#[test]
fn only_the_window_is_kept_and_older_files_are_deleted() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("segments.csv"), fake(dir.path(), 10)).unwrap();
    let mut ring = Ring::new(dir.path(), Window::seconds(3), usize::MAX);
    assert_eq!(ring.poll(), 10);
    // ends 1..=10 s; the window keeps those within 3 s of the newest end
    assert_eq!(ring.span_ms(), 3_000);
    for gone in 0..6 {
        assert!(
            !exists(dir.path(), gone),
            "segment {gone} should be deleted"
        );
    }
    for kept in 6..10 {
        assert!(exists(dir.path(), kept), "segment {kept} should be kept");
    }
    // nothing new: nothing added, nothing read twice
    assert_eq!(ring.poll(), 0);

    let clip = ring.finish();
    assert_eq!(clip.len(), 4);
    assert_eq!(clip[0].length_ms, 1_000);
    assert_eq!(clip[0].bytes, 100);
    assert!(clip[0].path.ends_with("seg00006.ts") && clip[3].path.ends_with("seg00009.ts"));
    drop(clip);
    assert!(
        !exists(dir.path(), 9),
        "the files go when the clip is let go"
    );
}

#[test]
fn a_line_still_being_written_waits_for_its_newline() {
    let dir = tempfile::tempdir().unwrap();
    let list = fake(dir.path(), 2);
    let (first, second) = list.split_once('\n').unwrap();
    let csv = dir.path().join("segments.csv");
    let mut ring = Ring::new(dir.path(), Window::default(), usize::MAX);

    fs::write(&csv, format!("{first}\nseg00001.ts,1.000")).unwrap();
    assert_eq!(ring.poll(), 1);
    fs::write(&csv, format!("{first}\n{second}")).unwrap();
    assert_eq!(ring.poll(), 1);
    assert_eq!(ring.poll(), 0);
    assert_eq!(ring.finish().len(), 2);
}

#[test]
fn a_missing_list_or_file_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut ring = Ring::new(dir.path(), Window::default(), usize::MAX);
    assert_eq!(ring.poll(), 0, "no list yet");
    // a line for a file that is not there
    fs::write(dir.path().join("segments.csv"), "seg00000.ts,0,1\n").unwrap();
    assert_eq!(ring.poll(), 0);
    assert!(ring.finish().is_empty());
}

#[test]
fn the_disk_budget_drops_the_oldest_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("segments.csv"), fake(dir.path(), 5)).unwrap();
    let mut ring = Ring::new(dir.path(), Window::seconds(30), 250);
    ring.poll();
    assert!(ring.was_squeezed());
    assert_eq!(ring.finish().len(), 2);
}

#[test]
fn a_name_that_climbs_out_of_the_folder_is_ignored_and_never_deleted() {
    let outer = tempfile::tempdir().unwrap();
    let dir = outer.path().join("ring");
    fs::create_dir(&dir).unwrap();
    let victim = outer.path().join("victim.ts");
    fs::write(&victim, b"keep me").unwrap();
    fs::write(
        dir.join("segments.csv"),
        "../victim.ts,0,1\n/x/victim.ts,1,2\n",
    )
    .unwrap();
    let mut ring = Ring::new(&dir, Window::seconds(1), usize::MAX);
    assert_eq!(ring.poll(), 0);
    ring.clear();
    drop(ring);
    assert!(victim.exists());
}

#[test]
fn the_output_arguments_write_a_list_and_segments_inside_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let ring = Ring::new(dir.path(), Window::default(), usize::MAX);
    let a: Vec<String> = ring
        .output_args(1)
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(a[..4], ["-f", "segment", "-segment_time", "1"]);
    assert!(a.contains(&format!(
        "file:{}",
        dir.path().join("segments.csv").display()
    )));
    assert_eq!(
        a.last().unwrap(),
        &format!("file:{}", dir.path().join("seg%05d.ts").display())
    );
    assert!(Ring::new(dir.path(), Window::default(), 1)
        .output_args(0)
        .iter()
        .any(|a| a == "1"));
}

#[test]
fn joining_needs_segments_and_quotes_every_name() {
    let none: Vec<Segment> = vec![];
    assert_eq!(
        join(&none, Path::new("l.txt"), Path::new("o.mp4")).unwrap_err(),
        JoinError::Empty
    );
    let dir = tempfile::tempdir().unwrap();
    let quirky = dir.path().join("it's.ts");
    fs::write(&quirky, b"x").unwrap();
    let segs = vec![Segment {
        path: quirky,
        bytes: 1,
        length_ms: 1000,
    }];
    let (list, args) = join(&segs, Path::new("/w/list.txt"), Path::new("/w/out.mp4")).unwrap();
    assert_eq!(
        list,
        format!("file '{}/it'\\''s.ts'\n", dir.path().display())
    );
    let a: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(a.windows(2).any(|w| w == ["-i", "file:/w/list.txt"]));
    assert!(a.windows(2).any(|w| w == ["-c", "copy"]));
    assert_eq!(a.last().unwrap(), "file:/w/out.mp4");

    let newline = vec![Segment {
        path: dir.path().join("a\nb.ts"),
        bytes: 1,
        length_ms: 1000,
    }];
    assert_eq!(
        join(&newline, Path::new("l"), Path::new("o")).unwrap_err(),
        JoinError::BadName
    );
}

#[test]
fn a_real_recording_keeps_a_few_seconds_and_joins_into_a_clip() {
    let (Some(ffmpeg), Some(ffprobe)) = (locate("ffmpeg", &[]), locate("ffprobe", &[])) else {
        eprintln!("skipped: ffmpeg and ffprobe are not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let ring_dir = dir.path().join("ring");
    fs::create_dir(&ring_dir).unwrap();
    let mut ring = Ring::new(&ring_dir, Window::seconds(3), usize::MAX);

    // A live picture for 7 s (-re runs the test source in real time), a keyframe every second.
    let mut recorder = Command::new(&ffmpeg);
    recorder
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc=s=320x240:r=15")
        .args([
            "-t",
            "7",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "15",
            "-pix_fmt",
            "yuv420p",
        ])
        .args(ring.output_args(1))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = recorder.spawn() else {
        eprintln!("skipped: could not start ffmpeg");
        return;
    };
    let mut most_on_disk = 0;
    while child.try_wait().unwrap().is_none() {
        ring.poll();
        let on_disk = fs::read_dir(&ring_dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "ts")
            })
            .count();
        most_on_disk = most_on_disk.max(on_disk);
        std::thread::sleep(Duration::from_millis(100));
    }
    // 7 s were recorded, but the folder never held much more than the window
    assert!(
        most_on_disk <= 6,
        "{most_on_disk} segments were on disk at once"
    );

    let clip = ring.finish();
    assert!(!clip.is_empty());
    assert!(!exists(&ring_dir, 0), "the first second was long gone");
    let (list, args) = join(
        &clip,
        &dir.path().join("list.txt"),
        &dir.path().join("clip.mp4"),
    )
    .unwrap();
    fs::File::create(dir.path().join("list.txt"))
        .unwrap()
        .write_all(list.as_bytes())
        .unwrap();
    let joined = crate::export::Plan {
        args,
        length_ms: 0,
        size: (0, 0),
        dropped_overlays: 0,
    };
    run(&ffmpeg, &joined, &|| false, Duration::from_secs(60)).unwrap();

    let info = probe(&ffprobe, &dir.path().join("clip.mp4")).unwrap();
    assert_eq!((info.width, info.height), (320, 240));
    assert!(
        (2_500..=5_500).contains(&info.duration_ms),
        "a 3 s window gave a {} ms clip",
        info.duration_ms
    );

    // and the clip goes through the normal edit and export
    let out = dir.path().join("final.mp4");
    let p = plan(&dir.path().join("clip.mp4"), &out, &Edit::default(), &info).unwrap();
    run(&ffmpeg, &p, &|| false, Duration::from_secs(60)).unwrap();
    assert!(out.metadata().unwrap().len() > 0);
}
