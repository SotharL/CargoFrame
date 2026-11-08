//! Ask the desktop portal for a screen cast and read two seconds of it with GStreamer, as a
//! recording does. For checking a Wayland desktop by hand: `cargo run --example screencast_probe`.
#[cfg(target_os = "linux")]
fn main() {
    let cast = cargoframe_capture::screencast::ScreenCast::start(None).expect("screen cast");
    println!("stream {:?} token {:?}", cast.stream, cast.restore_token);
    let out = std::process::Command::new("gst-launch-1.0")
        .args([
            "-q",
            "pipewiresrc",
            &format!("path={}", cast.stream.node),
            "do-timestamp=true",
            "keepalive-time=100",
            "num-buffers=20",
            "!",
            "videoconvert",
            "!",
            "video/x-raw,format=I420",
            "!",
            "y4menc",
            "!",
            "fdsink",
            "fd=1",
        ])
        .output()
        .expect("gst-launch-1.0");
    let head = String::from_utf8_lossy(&out.stdout[..out.stdout.len().min(60)]).to_string();
    println!(
        "gst status {} bytes {} head {:?} err {}",
        out.status,
        out.stdout.len(),
        head.lines().next().unwrap_or(""),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[cfg(not(target_os = "linux"))]
fn main() {}
