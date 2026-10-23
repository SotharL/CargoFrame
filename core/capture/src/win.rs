//! Windows backend. The window in focus comes from `GetForegroundWindow`; its visible bounds
//! from DWM (without the invisible resize border); pixels from a copy of the desktop, which
//! shows exactly what the person sees, including menus and pop-ups over the window.
//!
//! This is the only module that needs `unsafe`.
#![allow(unsafe_code)]

use crate::error::{grab, CaptureError};
use crate::frame::{Frame, PixelRect};
use crate::policy::{Backend, FocusedApp};
use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

pub struct Windows;

fn rect_of(r: RECT) -> PixelRect {
    PixelRect::new(
        r.left,
        r.top,
        u32::try_from(r.right - r.left).unwrap_or(0),
        u32::try_from(r.bottom - r.top).unwrap_or(0),
    )
}

fn process_path(pid: u32) -> Option<PathBuf> {
    // SAFETY: plain Win32 calls; the handle is closed before returning and the buffer outlives
    // the call that fills it.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        Some(PathBuf::from(std::ffi::OsString::from_wide(
            &buf[..len as usize],
        )))
    }
}

impl Backend for Windows {
    fn focused(&self) -> Result<Option<FocusedApp>, CaptureError> {
        // SAFETY: plain Win32 calls on a window handle that may vanish at any moment; each call
        // reports failure instead of faulting.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return Ok(None);
            }
            let mut rect = RECT::default();
            let framed = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut rect as *mut RECT).cast::<c_void>(),
                std::mem::size_of::<RECT>() as u32,
            )
            .is_ok();
            if !framed {
                GetWindowRect(hwnd, &mut rect).map_err(grab)?;
            }
            let mut title = [0u16; 512];
            let n = GetWindowTextW(hwnd, &mut title);
            let title = String::from_utf16_lossy(&title[..usize::try_from(n).unwrap_or(0)]);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            Ok(Some(FocusedApp {
                window: hwnd.0 as usize as u64,
                exe_path: if pid == 0 { None } else { process_path(pid) },
                title,
                pid: (pid != 0).then_some(pid),
                bounds: rect_of(rect),
            }))
        }
    }

    fn pointer_display(&self) -> Result<Option<PixelRect>, CaptureError> {
        // SAFETY: plain Win32 calls with properly sized, initialised structures.
        unsafe {
            let mut at = POINT::default();
            GetCursorPos(&mut at).map_err(grab)?;
            let monitor = MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                Ok(Some(rect_of(info.rcMonitor)))
            } else {
                Ok(None)
            }
        }
    }

    fn desktop(&self) -> Result<PixelRect, CaptureError> {
        // SAFETY: GetSystemMetrics has no preconditions.
        let (x, y, w, h) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        if w <= 0 || h <= 0 {
            return Err(CaptureError::NoDisplay("the desktop has no size".into()));
        }
        Ok(PixelRect::new(x, y, w as u32, h as u32))
    }

    fn grab(&self, area: PixelRect) -> Result<Frame, CaptureError> {
        let desktop = self.desktop()?;
        let shot = win_screenshot::capture::capture_display()
            .map_err(|e| CaptureError::Grab(format!("{e:?}")))?;
        let mut rgba = shot.pixels;
        // GDI leaves the alpha byte at 0.
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        let whole = Frame::new(shot.width, shot.height, rgba)?;
        let relative = PixelRect::new(area.x - desktop.x, area.y - desktop.y, area.w, area.h);
        whole
            .crop(relative)
            .ok_or_else(|| CaptureError::Grab("the area is outside the desktop".into()))
    }
}
