//! Windows: plain top-most windows of their own class (no web page), that never take the focus,
//! let clicks through, are left out of the taskbar and, with `WDA_EXCLUDEFROMCAPTURE`, out of
//! every screen capture, the recording included. Four red bands make the outline; the dot is a
//! window cut round. A thread runs their messages and makes the dot blink; dropping the
//! indicator closes them.

use crate::geometry::{Layout, Rect};
use std::sync::mpsc;
use std::thread::JoinHandle;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateEllipticRgn, CreateSolidBrush, SetWindowRgn};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostThreadMessageW, RegisterClassW, SetLayeredWindowAttributes, SetTimer,
    SetWindowDisplayAffinity, ShowWindow, TranslateMessage, LWA_ALPHA, MSG, SW_HIDE,
    SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE, WINDOW_EX_STYLE, WM_DESTROY, WM_QUIT, WM_TIMER,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};

/// Red, as Windows writes colours (0x00BBGGRR).
const RED: u32 = 0x002D_32E5;
const BLINK: usize = 1;

pub struct Shown {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

unsafe extern "system" fn sign_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn make(r: Rect, round: bool) -> Result<HWND, String> {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null()).map_err(|e| e.to_string())?;
        let ex: WINDOW_EX_STYLE =
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let hwnd = CreateWindowExW(
            ex,
            w!("CargoFrameRecordingSign"),
            w!("CargoFrame is recording"),
            WS_POPUP,
            r.x,
            r.y,
            r.w.max(1) as i32,
            r.h.max(1) as i32,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        // see-through to clicks (WS_EX_TRANSPARENT needs a layered window), fully drawn
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        // never in a screen capture, so never in the recording (Windows 10 2004 and later)
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        if round {
            let rgn = CreateEllipticRgn(0, 0, r.w as i32 + 1, r.h as i32 + 1);
            let _ = SetWindowRgn(hwnd, Some(rgn), true);
        }
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Ok(hwnd)
    }
}

impl Shown {
    pub fn start(lay: &Layout) -> Result<Shown, String> {
        let lay = lay.clone();
        let (ready, started) = mpsc::channel::<Result<u32, String>>();
        let thread = std::thread::Builder::new()
            .name("recording-sign".into())
            .spawn(move || unsafe {
                let instance = match GetModuleHandleW(PCWSTR::null()) {
                    Ok(i) => i,
                    Err(e) => {
                        let _ = ready.send(Err(e.to_string()));
                        return;
                    }
                };
                let class = WNDCLASSW {
                    lpfnWndProc: Some(sign_proc),
                    hInstance: instance.into(),
                    lpszClassName: w!("CargoFrameRecordingSign"),
                    hbrBackground: CreateSolidBrush(COLORREF(RED)),
                    ..Default::default()
                };
                // registering twice (a second recording) fails harmlessly: the class is there
                RegisterClassW(&class);
                let mut shown = Vec::new();
                for b in &lay.bands {
                    match make(*b, false) {
                        Ok(h) => shown.push(h),
                        Err(e) => {
                            tracing::warn!("a band of the recording sign could not be made: {e}")
                        }
                    }
                }
                let dot = lay.dot.and_then(|d| make(d, true).ok());
                if let Some(d) = dot {
                    shown.push(d);
                }
                // a thread timer: one message a second to blink the dot
                let timer = SetTimer(None, BLINK, 1000, None);
                let _ = ready.send(Ok(windows::Win32::System::Threading::GetCurrentThreadId()));
                let mut on = true;
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_TIMER && msg.hwnd.is_invalid() {
                        on = !on;
                        if let Some(d) = dot {
                            let _ = ShowWindow(d, if on { SW_SHOWNOACTIVATE } else { SW_HIDE });
                        }
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = KillTimer(None, timer);
                for h in shown {
                    let _ = DestroyWindow(h);
                }
            })
            .map_err(|e| e.to_string())?;
        let thread_id = started
            .recv()
            .map_err(|_| "the recording sign stopped before it started".to_string())??;
        Ok(Shown {
            thread_id,
            thread: Some(thread),
        })
    }
}

impl Drop for Shown {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
