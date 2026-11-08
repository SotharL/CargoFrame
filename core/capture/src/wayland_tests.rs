use super::*;
use serde_json::json;
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> Compositor {
    let m: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Compositor::detect(&|k| m.get(k).cloned())
}

#[test]
fn the_compositor_is_found_from_its_variables() {
    assert_eq!(
        env(&[
            ("SWAYSOCK", "/run/user/1000/sway-ipc.sock"),
            ("XDG_CURRENT_DESKTOP", "sway")
        ]),
        Compositor::Sway {
            socket: "/run/user/1000/sway-ipc.sock".into()
        }
    );
    match env(&[
        ("HYPRLAND_INSTANCE_SIGNATURE", "abc"),
        ("XDG_RUNTIME_DIR", "/run/user/1000"),
    ]) {
        Compositor::Hyprland { socket } => {
            assert!(socket.ends_with("abc/.socket.sock"), "{socket:?}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(env(&[("XDG_CURRENT_DESKTOP", "KDE")]), Compositor::Kde);
    assert_eq!(
        env(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")]),
        Compositor::Gnome
    );
    assert_eq!(
        env(&[("XDG_CURRENT_DESKTOP", "GNOME-Classic:GNOME")]),
        Compositor::Gnome
    );
    assert_eq!(env(&[("XDG_CURRENT_DESKTOP", "river")]), Compositor::Other);
    assert_eq!(env(&[("SWAYSOCK", "")]), Compositor::Other);
    assert!(env(&[("SWAYSOCK", "/s")]).knows_focus());
    assert!(!env(&[("XDG_CURRENT_DESKTOP", "GNOME")]).knows_focus());
}

/// A trimmed `swaymsg -t get_tree`: one output, a workspace, two windows, the second focused.
fn sway_tree(focused_is_window: bool) -> Value {
    json!({
        "id": 1, "type": "root", "focused": false, "rect": {"x":0,"y":0,"width":3200,"height":1080},
        "nodes": [
            {"id": 2, "type": "output", "name": "__i3", "focused": false, "nodes": []},
            {"id": 3, "type": "output", "name": "DP-1", "focused": false,
             "rect": {"x":0,"y":0,"width":1280,"height":800},
             "nodes": [
                {"id": 4, "type": "workspace", "name": "1", "focused": !focused_is_window,
                 "rect": {"x":0,"y":0,"width":1280,"height":800},
                 "nodes": [
                    {"id": 5, "type": "con", "name": "Terminal", "pid": 100, "app_id": "foot",
                     "focused": false, "rect": {"x":0,"y":0,"width":640,"height":800}, "nodes": []},
                    {"id": 6, "type": "con", "name": "Foo - main.rs", "pid": 4242, "app_id": "foo",
                     "focused": focused_is_window,
                     "rect": {"x":640,"y":25,"width":640,"height":775},
                     "deco_rect": {"x":0,"y":0,"width":640,"height":25}, "nodes": []}
                 ],
                 "floating_nodes": []}
             ]}
        ]
    })
}

fn sway_outputs() -> Value {
    json!([
        {"name": "DP-1", "active": true, "focused": true, "scale": 2.0,
         "rect": {"x":0,"y":0,"width":1280,"height":800}},
        {"name": "HDMI-A-1", "active": true, "focused": false, "scale": 1.0,
         "rect": {"x":1280,"y":0,"width":1920,"height":1080}},
        {"name": "off", "active": false, "focused": false,
         "rect": {"x":0,"y":0,"width":0,"height":0}}
    ])
}

#[test]
fn sway_tells_the_focused_window_and_screens() {
    let l = sway_layout(&sway_tree(true), &sway_outputs());
    let app = l.focused.unwrap();
    assert_eq!(app.title, "Foo - main.rs");
    assert_eq!(app.pid, Some(4242));
    assert_eq!(app.window, 6);
    assert_eq!(app.bounds, PixelRect::new(640, 0, 640, 800));
    assert_eq!(l.output, Some(PixelRect::new(0, 0, 1280, 800)));
    assert_eq!(l.desktop, Some(PixelRect::new(0, 0, 3200, 1080)));
}

#[test]
fn a_focused_empty_workspace_is_no_window() {
    let l = sway_layout(&sway_tree(false), &sway_outputs());
    assert_eq!(l.focused, None);
    assert!(l.output.is_some());
}

#[test]
fn hyprland_tells_the_window_and_scales_monitors_to_logical_pixels() {
    let active = json!({
        "address": "0x55d1", "at": [12, 40], "size": [900, 600],
        "title": "Foo - main.rs", "pid": 4242, "class": "foo"
    });
    let monitors = json!([
        {"name": "eDP-1", "x": 0, "y": 0, "width": 2560, "height": 1600, "scale": 2.0,
         "transform": 0, "focused": true},
        {"name": "DP-2", "x": 1280, "y": 0, "width": 1080, "height": 1920, "scale": 1.0,
         "transform": 1, "focused": false}
    ]);
    let l = hypr_layout(&active, &monitors);
    let app = l.focused.unwrap();
    assert_eq!(app.pid, Some(4242));
    assert_eq!(app.window, 0x55d1);
    assert_eq!(app.bounds, PixelRect::new(12, 40, 900, 600));
    assert_eq!(l.output, Some(PixelRect::new(0, 0, 1280, 800)));
    // the second monitor is turned: 1920 wide in the layout
    assert_eq!(l.desktop, Some(PixelRect::new(0, 0, 3200, 1080)));
}

#[test]
fn hyprland_with_nothing_focused_answers_an_empty_object() {
    let l = hypr_layout(&json!({}), &json!([]));
    assert_eq!(l.focused, None);
    assert_eq!(l.desktop, None);
}

#[test]
fn the_kwin_script_calls_back_the_given_name_with_one_string() {
    let s = kwin_script(":1.77");
    assert!(s.contains(r#"callDBus(":1.77", "/org/cargoframe/Focus", "org.cargoframe.Focus", "Window", JSON.stringify(out));"#), "{s}");
    assert!(s.contains("workspace.activeWindow || workspace.activeClient"));
    assert!(s.contains("frameGeometry"));
}

#[test]
fn kwin_answers_are_read() {
    let l = kwin_layout(
        r#"{"window":{"pid":4242,"title":"Foo — Kate","x":10.5,"y":20,"width":800,"height":600},
            "desktop":{"x":0,"y":0,"width":2560,"height":1440}}"#,
    )
    .unwrap();
    let app = l.focused.unwrap();
    assert_eq!(app.pid, Some(4242));
    assert_eq!(app.title, "Foo — Kate");
    assert_eq!(app.bounds, PixelRect::new(11, 20, 800, 600));
    assert_eq!(l.desktop, Some(PixelRect::new(0, 0, 2560, 1440)));

    let none = kwin_layout(r#"{"desktop":{"x":0,"y":0,"width":10,"height":10}}"#).unwrap();
    assert_eq!(none.focused, None);
    assert!(matches!(
        kwin_layout("not json"),
        Err(CaptureError::FocusUnknown(_))
    ));
}

#[test]
fn logical_places_are_scaled_into_the_picture() {
    // a 1280x800 layout shown on a 2560x1600 picture (scale 2)
    let r = to_pixels(
        PixelRect::new(640, 0, 640, 800),
        PixelRect::new(0, 0, 1280, 800),
        (2560, 1600),
    );
    assert_eq!(r, PixelRect::new(1280, 0, 1280, 1600));
    // a layout that does not start at 0,0
    let r = to_pixels(
        PixelRect::new(-100, 50, 10, 10),
        PixelRect::new(-200, 0, 400, 100),
        (400, 100),
    );
    assert_eq!(r, PixelRect::new(100, 50, 10, 10));
    // an empty layout changes nothing
    let same = PixelRect::new(1, 2, 3, 4);
    assert_eq!(to_pixels(same, PixelRect::new(0, 0, 0, 0), (9, 9)), same);
}

#[test]
fn screens_are_joined() {
    assert_eq!(
        union(&[PixelRect::new(0, 0, 10, 10), PixelRect::new(-5, 20, 10, 10)]),
        Some(PixelRect::new(-5, 0, 15, 30))
    );
    assert_eq!(union(&[]), None);
}

/// A fake sway: answers GET_TREE and GET_OUTPUTS on a socket, as the real one does.
#[test]
fn sway_is_asked_over_its_socket() {
    use std::os::unix::net::UnixListener;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sway.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut s, _) = listener.accept().unwrap();
            let mut head = [0u8; 14];
            s.read_exact(&mut head).unwrap();
            assert_eq!(&head[..6], b"i3-ipc");
            let kind = u32::from_ne_bytes([head[10], head[11], head[12], head[13]]);
            let body = match kind {
                SWAY_GET_TREE => sway_tree(true).to_string(),
                SWAY_GET_OUTPUTS => sway_outputs().to_string(),
                other => panic!("unexpected request {other}"),
            };
            let mut msg = b"i3-ipc".to_vec();
            msg.extend_from_slice(&(body.len() as u32).to_ne_bytes());
            msg.extend_from_slice(&kind.to_ne_bytes());
            msg.extend_from_slice(body.as_bytes());
            s.write_all(&msg).unwrap();
        }
    });
    let l = layout(&Compositor::Sway { socket: path }).unwrap();
    server.join().unwrap();
    assert_eq!(l.focused.unwrap().title, "Foo - main.rs");
}

#[test]
fn a_missing_sway_socket_is_an_unknown_focus_not_a_failure() {
    let e = layout(&Compositor::Sway {
        socket: "/nonexistent/sway.sock".into(),
    })
    .unwrap_err();
    assert!(matches!(e, CaptureError::FocusUnknown(_)), "{e:?}");
}

#[test]
fn gnome_without_xwayland_says_why_the_screen_was_captured() {
    // DISPLAY is read from the environment; this test only runs its assertion without one.
    if std::env::var_os("DISPLAY").is_some() {
        return;
    }
    match layout(&Compositor::Gnome).unwrap_err() {
        CaptureError::FocusUnknown(why) => assert!(why.starts_with("GNOME does not tell"), "{why}"),
        other => panic!("{other:?}"),
    }
}

/// A fake KWin on a private bus: it takes the script, and when asked to run it, calls back the
/// name written in it, as KWin's `callDBus` would. This checks names, paths and signatures.
#[test]
fn kwin_is_asked_through_a_script_on_the_session_bus() {
    use std::sync::{Arc, Mutex};
    use zbus::blocking::connection::Builder;
    let Some(daemon) = find_program("dbus-daemon") else {
        eprintln!("skipped: no dbus-daemon");
        return;
    };
    struct Bus(std::process::Child);
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut bus = Bus(std::process::Command::new(daemon)
        .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap());
    let mut address = String::new();
    {
        use std::io::BufRead;
        std::io::BufReader::new(bus.0.stdout.as_mut().unwrap())
            .read_line(&mut address)
            .unwrap();
    }
    let address = address.trim().to_string();

    #[derive(Clone, Default)]
    struct Kwin {
        script: Arc<Mutex<String>>,
        address: String,
        unloaded: Arc<Mutex<u32>>,
    }
    #[zbus::interface(name = "org.kde.kwin.Scripting")]
    impl Kwin {
        #[zbus(name = "loadScript")]
        fn load_script(&self, path: String, _plugin: String) -> i32 {
            *self.script.lock().unwrap() = std::fs::read_to_string(path).unwrap();
            7
        }
        #[zbus(name = "unloadScript")]
        fn unload_script(&self, _plugin: String) -> bool {
            *self.unloaded.lock().unwrap() += 1;
            true
        }
    }
    struct Script(Kwin);
    #[zbus::interface(name = "org.kde.kwin.Script")]
    impl Script {
        #[zbus(name = "run")]
        fn run(&self) {
            let script = self.0.script.lock().unwrap().clone();
            let name = script
                .split("callDBus(\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .to_string();
            let address = self.0.address.clone();
            std::thread::spawn(move || {
                let c = Builder::address(address.as_str()).unwrap().build().unwrap();
                let answer =
                    r#"{"window":{"pid":1,"title":"Kate","x":5,"y":6,"width":70,"height":80}}"#;
                c.call_method(
                    Some(name.as_str()),
                    "/org/cargoframe/Focus",
                    Some("org.cargoframe.Focus"),
                    "Window",
                    &(answer,),
                )
                .unwrap();
            });
        }
    }
    let kwin = Kwin {
        address: address.clone(),
        ..Kwin::default()
    };
    let _service = Builder::address(address.as_str())
        .unwrap()
        .name("org.kde.KWin")
        .unwrap()
        .serve_at("/Scripting", kwin.clone())
        .unwrap()
        .serve_at("/Scripting/Script7", Script(kwin.clone()))
        .unwrap()
        .build()
        .unwrap();

    let l = kwin::layout_on(
        Builder::address(address.as_str()).unwrap(),
        Duration::from_secs(5),
    )
    .unwrap();
    let app = l.focused.unwrap();
    assert_eq!(app.title, "Kate");
    assert_eq!(app.bounds, PixelRect::new(5, 6, 70, 80));
    // the script is unloaded before (a leftover) and after
    assert_eq!(*kwin.unloaded.lock().unwrap(), 2);
    drop(bus);
}
