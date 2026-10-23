//! A tiny X11 program for end-to-end tests: one window with a title, a process id and a few
//! coloured rectangles, which takes the input focus and then idles. The test copies this
//! binary to `<projects>/Foo/Foo` so CargoFrame can climb from the program to its project.
//!
//! Usage: `fake_app [title]`. Linux only; elsewhere it does nothing.
#[cfg(target_os = "linux")]
fn main() {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, InputFocus,
        PropMode, Rectangle, WindowClass,
    };
    use x11rb::wrapper::ConnectionExt as _;
    use x11rb::COPY_DEPTH_FROM_PARENT;

    let title = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Foo - main.rs".into());
    let (conn, screen_num) = x11rb::connect(None).expect("connect to the X server");
    let screen = conn.setup().roots[screen_num].clone();
    let atom = |name: &str| {
        conn.intern_atom(false, name.as_bytes())
            .unwrap()
            .reply()
            .unwrap()
            .atom
    };

    let win = conn.generate_id().unwrap();
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win,
        screen.root,
        120,
        90,
        480,
        300,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(0x0023_2b3d)
            .event_mask(EventMask::EXPOSURE),
    )
    .unwrap();
    conn.change_property8(
        PropMode::REPLACE,
        win,
        atom("_NET_WM_NAME"),
        atom("UTF8_STRING"),
        title.as_bytes(),
    )
    .unwrap();
    conn.change_property32(
        PropMode::REPLACE,
        win,
        atom("_NET_WM_PID"),
        AtomEnum::CARDINAL,
        &[std::process::id()],
    )
    .unwrap();
    conn.map_window(win).unwrap();
    conn.set_input_focus(InputFocus::NONE, win, x11rb::CURRENT_TIME)
        .unwrap();

    let gc = conn.generate_id().unwrap();
    conn.create_gc(gc, win, &CreateGCAux::new().foreground(0x00e0_4f4f))
        .unwrap();
    let draw = |c: u32, r: Rectangle| {
        conn.change_gc(
            gc,
            &x11rb::protocol::xproto::ChangeGCAux::new().foreground(c),
        )
        .unwrap();
        conn.poly_fill_rectangle(win, gc, &[r]).unwrap();
    };
    // A title bar, an "error" strip and a blue bar, so a screenshot is easy to recognise.
    let paint = || {
        draw(
            0x002b_3245,
            Rectangle {
                x: 0,
                y: 0,
                width: 480,
                height: 28,
            },
        );
        draw(
            0x00e0_4f4f,
            Rectangle {
                x: 20,
                y: 220,
                width: 440,
                height: 40,
            },
        );
        draw(
            0x004f_8ee0,
            Rectangle {
                x: 20,
                y: 60,
                width: 200,
                height: 20,
            },
        );
        conn.flush().unwrap();
    };
    paint();
    println!("fake_app ready: {title}");
    // Repaint on expose so the picture is still there after other windows come and go.
    while let Ok(ev) = conn.wait_for_event() {
        if matches!(ev, x11rb::protocol::Event::Expose(_)) {
            paint();
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}
