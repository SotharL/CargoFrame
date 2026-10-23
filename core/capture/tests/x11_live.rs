//! Runs against a real X server (CI uses xvfb). Skipped when there is no `DISPLAY`.
#![cfg(target_os = "linux")]

use cargoframe_capture::x11::X11;
use cargoframe_capture::{capture_with, Backend, Mode, Subject};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, InputFocus, PropMode, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

fn intern(conn: &RustConnection, name: &str) -> u32 {
    conn.intern_atom(false, name.as_bytes())
        .unwrap()
        .reply()
        .unwrap()
        .atom
}

#[test]
fn finds_the_focused_window_captures_it_and_falls_back_when_it_is_gone() {
    if std::env::var_os("DISPLAY").is_none() {
        eprintln!("skipped: no DISPLAY (run under xvfb-run)");
        return;
    }
    let (conn, screen_num) = RustConnection::connect(None).unwrap();
    let screen = conn.setup().roots[screen_num].clone();

    // A red window with the properties a real program would set.
    let win = conn.generate_id().unwrap();
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win,
        screen.root,
        100,
        80,
        320,
        200,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new().background_pixel(0x00FF_0000),
    )
    .unwrap();
    let title = "cargoframe-capture-test";
    conn.change_property8(
        PropMode::REPLACE,
        win,
        intern(&conn, "_NET_WM_NAME"),
        intern(&conn, "UTF8_STRING"),
        title.as_bytes(),
    )
    .unwrap();
    conn.change_property32(
        PropMode::REPLACE,
        win,
        intern(&conn, "_NET_WM_PID"),
        AtomEnum::CARDINAL,
        &[std::process::id()],
    )
    .unwrap();
    conn.map_window(win).unwrap();
    conn.set_input_focus(InputFocus::NONE, win, x11rb::CURRENT_TIME)
        .unwrap();
    conn.flush().unwrap();
    conn.get_input_focus().unwrap().reply().unwrap(); // round trip: the server has handled it all
    std::thread::sleep(std::time::Duration::from_millis(200));

    let x11 = X11::connect().expect("connect");

    // 1. The focused window, with its program.
    let app = x11.focused().unwrap().expect("a focused window");
    assert_eq!(app.title, title);
    assert_eq!(app.pid, Some(std::process::id()));
    assert_eq!(
        app.exe_path.as_deref().map(|p| p.canonicalize().unwrap()),
        Some(std::env::current_exe().unwrap().canonicalize().unwrap())
    );
    assert_eq!(
        (app.bounds.x, app.bounds.y, app.bounds.w, app.bounds.h),
        (100, 80, 320, 200)
    );

    // 2. Capturing it gives the window's pixels.
    let shot = capture_with(&x11, Mode::ActiveWindow).unwrap();
    assert_eq!(shot.subject, Subject::Window);
    assert_eq!((shot.frame.width(), shot.frame.height()), (320, 200));
    assert_eq!(shot.frame.pixel(160, 100), Some([255, 0, 0, 255]));
    assert!(shot.notes.is_empty(), "{:?}", shot.notes);

    // 3. The other modes.
    let all = capture_with(&x11, Mode::AllDisplays).unwrap();
    assert_eq!(all.subject, Subject::AllDisplays);
    assert_eq!(
        (all.frame.width(), all.frame.height()),
        (
            u32::from(screen.width_in_pixels),
            u32::from(screen.height_in_pixels)
        )
    );
    // The window shows up at its place in the full picture.
    assert_eq!(all.frame.pixel(100 + 160, 80 + 100), Some([255, 0, 0, 255]));
    let display = capture_with(&x11, Mode::Display).unwrap();
    assert!(matches!(
        display.subject,
        Subject::Display | Subject::AllDisplays
    ));
    assert!(display.frame.width() > 0);

    // 4. The window goes away: no focus, so the display is captured and a note says why.
    conn.destroy_window(win).unwrap();
    conn.flush().unwrap();
    conn.get_input_focus().unwrap().reply().unwrap();
    let after = capture_with(&x11, Mode::ActiveWindow).unwrap();
    assert!(
        after.app.as_ref().map(|a| a.title.as_str()) != Some(title),
        "the closed window must not be reported"
    );
    assert!(matches!(
        after.subject,
        Subject::Display | Subject::AllDisplays
    ));
    assert!(!after.notes.is_empty());

    // 5. The picture is a valid PNG that decodes to the same pixels.
    let png = shot.frame.to_png().unwrap();
    assert_eq!(&png[1..4], b"PNG");
}
