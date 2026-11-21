//! Windows: what this computer is, from the registry (the system, the processor, the graphics
//! cards and their drivers as Device Manager shows them) and from the system itself (the memory,
//! the screens). Read access to these keys needs no administrator.
#![allow(unsafe_code)]

use crate::frame::PixelRect;
use crate::system::{Gpu, System};
use std::ffi::c_void;
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A text value of a key under HKEY_LOCAL_MACHINE; empty when it is not there.
fn reg_text(key: &str, name: &str) -> String {
    let (k, n) = (wide(key), wide(name));
    let mut size: u32 = 0;
    // SAFETY: the key and name are NUL-terminated and live through the call; with no buffer the
    // call only writes the size it needs into `size`.
    let first = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if first != ERROR_SUCCESS || size < 2 {
        return String::new();
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    let mut got = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `got` bytes, which the call does not write past.
    let second = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast::<c_void>()),
            Some(&mut got),
        )
    };
    if second != ERROR_SUCCESS {
        return String::new();
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

fn os() -> String {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let mut name = reg_text(KEY, "ProductName");
    let build = reg_text(KEY, "CurrentBuild");
    // Windows 11 still says "Windows 10" in ProductName: its builds start at 22000
    if build.parse::<u32>().is_ok_and(|b| b >= 22000) {
        name = name.replacen("Windows 10", "Windows 11", 1);
    }
    let release = reg_text(KEY, "DisplayVersion");
    let mut out = if name.is_empty() {
        "Windows".to_string()
    } else {
        name
    };
    if !release.is_empty() {
        out.push(' ');
        out.push_str(&release);
    }
    if !build.is_empty() {
        out.push_str(&format!(" ({build})"));
    }
    out
}

fn cpu() -> String {
    let name = reg_text(
        r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
        "ProcessorNameString",
    );
    match std::thread::available_parallelism().map(|n| n.get()) {
        Ok(n) if n > 1 && !name.is_empty() => format!("{name} ({n} threads)"),
        _ => name,
    }
}

fn ram() -> u64 {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `m` is a MEMORYSTATUSEX with its length set, as the call asks.
    match unsafe { GlobalMemoryStatusEx(&mut m) } {
        Ok(()) => m.ullTotalPhys,
        Err(_) => 0,
    }
}

/// The display adapters of Device Manager, with their driver versions; Windows' own fallback
/// adapters ("Microsoft Basic Display", remote ones) are left out when there is another.
fn gpus() -> Vec<Gpu> {
    const CLASS: &str =
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut out: Vec<Gpu> = Vec::new();
    for i in 0..16 {
        let key = format!(r"{CLASS}\{i:04}");
        let name = reg_text(&key, "DriverDesc");
        if name.is_empty() {
            continue;
        }
        let gpu = Gpu {
            name,
            driver: reg_text(&key, "DriverVersion"),
        };
        if !out.contains(&gpu) {
            out.push(gpu);
        }
    }
    let real: Vec<Gpu> = out
        .iter()
        .filter(|g| !g.name.starts_with("Microsoft") && !g.name.contains("Remote Display"))
        .cloned()
        .collect();
    if real.is_empty() {
        out
    } else {
        real
    }
}

pub fn read() -> System {
    System {
        os: os(),
        cpu: cpu(),
        ram_bytes: ram(),
        gpus: gpus(),
        computer: std::env::var("COMPUTERNAME").unwrap_or_default(),
    }
}

unsafe extern "system" fn each_monitor(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the `Vec` `monitors` passes, alive for the whole enumeration.
    let list = unsafe { &mut *(data.0 as *mut Vec<(PixelRect, bool)>) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` has its size set, as the call asks.
    if unsafe { GetMonitorInfoW(m, &mut info) }.as_bool() {
        let r = info.rcMonitor;
        list.push((
            PixelRect::new(
                r.left,
                r.top,
                u32::try_from(r.right - r.left).unwrap_or(0),
                u32::try_from(r.bottom - r.top).unwrap_or(0),
            ),
            info.dwFlags & MONITORINFOF_PRIMARY != 0,
        ));
    }
    BOOL(1)
}

/// The screens, the main one first.
pub fn monitors() -> Vec<PixelRect> {
    let mut list: Vec<(PixelRect, bool)> = Vec::new();
    // SAFETY: the callback only uses `list` through the pointer given, during this call.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(each_monitor),
            LPARAM(&mut list as *mut Vec<(PixelRect, bool)> as isize),
        )
    };
    list.sort_by_key(|(_, primary)| !primary);
    list.into_iter().map(|(r, _)| r).collect()
}
