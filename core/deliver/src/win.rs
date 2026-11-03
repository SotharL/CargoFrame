//! Key presses on Windows. This is the only unsafe code in the crate.
#![allow(unsafe_code)]

use crate::Key;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_RETURN, VK_V,
};

fn event(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Press a key (or Ctrl+V) as one batch, so nothing the person types can land in between.
pub fn press(key: Key) -> Result<(), String> {
    let down = KEYBD_EVENT_FLAGS(0);
    let up = KEYEVENTF_KEYUP;
    let events: Vec<INPUT> = match key {
        Key::Paste => vec![
            event(VK_CONTROL, down),
            event(VK_V, down),
            event(VK_V, up),
            event(VK_CONTROL, up),
        ],
        Key::Enter => vec![event(VK_RETURN, down), event(VK_RETURN, up)],
    };
    // SAFETY: `events` is a valid slice of fully initialised INPUT structures and the size passed
    // is the size of one of them, as SendInput requires.
    let sent = unsafe { SendInput(&events, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == events.len() {
        Ok(())
    } else {
        Err("Windows did not accept the key presses (is the other program running as administrator?)".into())
    }
}
