//! Try a delivery by hand: open a Claude chat and paste a test screenshot, a test file and a
//! message into it, exactly as a share does. For checking a new computer, not for users.
//!
//!   cargo run -p cargoframe-deliver --example try_claude -- <chat link | new> [options]
//!
//! Options: `--browser` (paste into a browser, not the app), `--no-enter` (leave the message in
//! the box), `--extra N` (N more seconds before pasting), `--no-file` (the screenshot only).
//! While it runs, every change of the window in front is printed, which shows what the Claude
//! window calls itself.

use cargoframe_deliver::{Deliver, Delivery, System, Target, Timing};
use cargoframe_handoff::{open_url, parse_chat_link, Destination, Scheme};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(chat) = args.first() else {
        eprintln!(
            "usage: try_claude <chat link | new> [--browser] [--no-enter] [--extra N] [--no-file]"
        );
        std::process::exit(2);
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    let extra: u64 = args
        .iter()
        .position(|a| a == "--extra")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let destination = if chat == "new" {
        Destination::NewChat
    } else {
        match parse_chat_link(chat) {
            Some(id) => Destination::ExistingChat(id),
            None => {
                eprintln!("that is not a Claude chat link");
                std::process::exit(2);
            }
        }
    };
    let browser = flag("--browser");
    let (scheme, target) = if browser {
        (Scheme::Web, Target::Browser)
    } else {
        (Scheme::App, Target::App)
    };

    let dir = std::env::temp_dir().join(format!("cargoframe-try-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp folder");
    let shot = dir.join("screenshot.png");
    write_png(&shot);
    let mut files = vec![shot];
    if !flag("--no-file") {
        let log = dir.join("test-log.txt");
        std::fs::write(
            &log,
            "CargoFrame delivery test file.\nLine 2: this is only a test.\n",
        )
        .expect("test file");
        files.push(log);
    }

    let delivery = Delivery {
        open_url: open_url(&destination, scheme),
        target,
        files,
        text: "CargoFrame delivery test (automated, please ignore).\n\nIf you can read this together with a small striped screenshot and test-log.txt, pasting into this chat works.".into(),
        submit: !flag("--no-enter"),
        timing: Timing::for_target(target, Duration::from_secs(extra)),
    };
    println!("Opening {}", delivery.open_url);

    let started = Instant::now();
    let stop = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        // Print each change of the window in front, so a failure shows what was in front.
        scope.spawn(|| {
            let mut last = String::new();
            while !stop.load(Ordering::Relaxed) {
                let now = match cargoframe_capture::focused_app() {
                    Ok(Some(a)) => format!(
                        "{:?} \"{}\"",
                        a.exe_path
                            .as_deref()
                            .and_then(|p| p.file_name())
                            .unwrap_or_default(),
                        a.title
                    ),
                    Ok(None) => "(nothing)".into(),
                    Err(e) => format!("(error: {e})"),
                };
                if now != last {
                    println!(
                        "  {:>5.1}s  in front: {now}",
                        started.elapsed().as_secs_f32()
                    );
                    last = now;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        });
        let result = System.deliver(&delivery, &open, &|| false);
        stop.store(true, Ordering::Relaxed);
        result
    });
    match result {
        Ok(done) => println!("DONE in {:.1}s: {done:?}", started.elapsed().as_secs_f32()),
        Err(e) => {
            println!("FAILED after {:.1}s: {e}", started.elapsed().as_secs_f32());
            std::process::exit(1);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn open(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .status();
    #[cfg(not(windows))]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("the opener exited with {s}")),
        Err(e) => Err(e.to_string()),
    }
}

/// A small striped picture, so it is obvious in the chat.
fn write_png(path: &std::path::Path) {
    let (w, h) = (320u32, 120u32);
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let stripe = (x / 20 + y / 20) % 2 == 0;
            data.extend_from_slice(if stripe {
                &[30, 110, 220]
            } else {
                &[250, 190, 40]
            });
        }
    }
    let file = std::fs::File::create(path).expect("png file");
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(&data))
        .expect("png data");
}
