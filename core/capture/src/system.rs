//! What this computer is, for the `system.*` values of a capture: the system, the processor, the
//! memory, the graphics cards and their drivers, the screens, the computer's name. Read once,
//! after the program starts (the screens at each capture, as they can change); nothing here
//! leaves the computer unless a layout puts a value in what it sends.

/// A graphics card and the version of its driver.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gpu {
    pub name: String,
    pub driver: String,
}

/// What is known of this computer. An empty field is one the system did not say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct System {
    /// `Windows 11 Pro 24H2 (26100)`, `Ubuntu 24.04.1 LTS (6.8.0-45-generic)`.
    pub os: String,
    /// `AMD Ryzen 7 7800X3D 8-Core Processor (16 threads)`.
    pub cpu: String,
    /// The memory in bytes.
    pub ram_bytes: u64,
    pub gpus: Vec<Gpu>,
    pub computer: String,
}

impl System {
    /// The values a capture gets, by name (`system.os`…). A value not known is left out, so a
    /// text that uses it says so instead of sending an empty space.
    pub fn values(&self, screens: &[crate::frame::PixelRect]) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let mut put = |k: &'static str, v: String| {
            if !v.trim().is_empty() {
                out.push((k, v));
            }
        };
        put("system.os", self.os.clone());
        put("system.cpu", self.cpu.clone());
        if self.ram_bytes > 0 {
            put("system.ram", ram_text(self.ram_bytes));
        }
        put(
            "system.gpu",
            self.gpus
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        put(
            "system.gpu_driver",
            self.gpus
                .iter()
                .filter(|g| !g.driver.is_empty())
                .map(|g| {
                    if self.gpus.len() > 1 {
                        format!("{} {}", g.name, g.driver)
                    } else {
                        g.driver.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        put("system.screens", screens_text(screens));
        put("system.computer", self.computer.clone());
        out
    }
}

/// `31.9 GB`: memory as people read it (in GiB, which is what systems call GB).
pub fn ram_text(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 10.0 {
        format!("{gb:.0} GB")
    } else {
        format!("{gb:.1} GB")
    }
}

/// `3 screens: 2560×1440 (main), 1920×1080, 1920×1080`; the main one first.
pub fn screens_text(screens: &[crate::frame::PixelRect]) -> String {
    let sizes: Vec<String> = screens
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let size = format!("{}×{}", s.w, s.h);
            if i == 0 && screens.len() > 1 {
                format!("{size} (main)")
            } else {
                size
            }
        })
        .collect();
    match sizes.len() {
        0 => String::new(),
        1 => sizes[0].clone(),
        n => format!("{n} screens: {}", sizes.join(", ")),
    }
}

/// Read what this computer is. A few file reads on Linux and registry reads on Windows: quick,
/// but done once, off the path of a capture.
pub fn read() -> System {
    #[cfg(windows)]
    {
        crate::win_system::read()
    }
    #[cfg(target_os = "linux")]
    {
        linux::read(std::path::Path::new("/"))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        System::default()
    }
}

/// The screens, the main one first. Empty when the system does not say (Wayland).
pub fn screens() -> Vec<crate::frame::PixelRect> {
    #[cfg(windows)]
    {
        crate::win_system::monitors()
    }
    #[cfg(target_os = "linux")]
    {
        crate::x11::monitors().unwrap_or_default()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
pub(crate) mod linux {
    //! Linux: `/etc/os-release`, `/proc` and `/sys`, read under `root` (`/`, or a test's folder).
    use super::{Gpu, System};
    use std::fs;
    use std::path::Path;

    fn text(root: &Path, path: &str) -> String {
        fs::read_to_string(root.join(path.trim_start_matches('/'))).unwrap_or_default()
    }

    pub fn read_os(root: &Path) -> String {
        let release = text(root, "/etc/os-release");
        let name = release
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| "Linux".into());
        let kernel = text(root, "/proc/sys/kernel/osrelease").trim().to_string();
        if kernel.is_empty() {
            name
        } else {
            format!("{name} ({kernel})")
        }
    }

    pub fn read_cpu(root: &Path) -> String {
        let info = text(root, "/proc/cpuinfo");
        let model = info
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                (k.trim() == "model name").then(|| v.trim().to_string())
            })
            .unwrap_or_default();
        let threads = info.lines().filter(|l| l.starts_with("processor")).count();
        match (model.is_empty(), threads) {
            (true, _) => String::new(),
            (false, 0 | 1) => model,
            (false, n) => format!("{model} ({n} threads)"),
        }
    }

    pub fn read_ram(root: &Path) -> u64 {
        text(root, "/proc/meminfo")
            .lines()
            .find_map(|l| l.strip_prefix("MemTotal:"))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .map_or(0, |kb| kb * 1024)
    }

    /// The graphics cards: each `card<N>` of `/sys/class/drm` (not its outputs, `card0-HDMI-A-1`),
    /// named by the PCI ids' vendor and the driver, and the driver's version where the driver
    /// says one (NVIDIA's), else the kernel's (amdgpu, i915 are part of it).
    pub fn read_gpus(root: &Path) -> Vec<Gpu> {
        let drm = root.join("sys/class/drm");
        let Ok(dir) = fs::read_dir(&drm) else {
            return Vec::new();
        };
        let mut cards: Vec<String> = dir
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| {
                n.strip_prefix("card")
                    .is_some_and(|d| d.chars().all(|c| c.is_ascii_digit()) && !d.is_empty())
            })
            .collect();
        cards.sort();
        let kernel = text(root, "/proc/sys/kernel/osrelease").trim().to_string();
        let mut out: Vec<Gpu> = Vec::new();
        for card in cards {
            let dev = drm.join(&card).join("device");
            let driver = fs::read_link(dev.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_default();
            let vendor = fs::read_to_string(dev.join("vendor")).unwrap_or_default();
            let maker = match vendor.trim() {
                "0x10de" => "NVIDIA",
                "0x1002" => "AMD",
                "0x8086" => "Intel",
                "0x1af4" => "Virtio",
                "0x15ad" => "VMware",
                _ => "",
            };
            if maker.is_empty() && driver.is_empty() {
                continue;
            }
            let name = match (maker, driver.as_str()) {
                ("", d) => d.to_string(),
                (m, "") => m.to_string(),
                (m, d) => format!("{m} ({d})"),
            };
            let version = text(root, &format!("/sys/module/{driver}/version"))
                .trim()
                .to_string();
            let driver_version = if !version.is_empty() {
                format!("{driver} {version}")
            } else if !driver.is_empty() && !kernel.is_empty() {
                format!("{driver} (kernel {kernel})")
            } else {
                driver.clone()
            };
            let gpu = Gpu {
                name,
                driver: driver_version,
            };
            if !out.contains(&gpu) {
                out.push(gpu);
            }
        }
        out
    }

    pub fn read(root: &Path) -> System {
        let computer = match text(root, "/proc/sys/kernel/hostname").trim() {
            "" => std::env::var("HOSTNAME").unwrap_or_default(),
            h => h.to_string(),
        };
        System {
            os: read_os(root),
            cpu: read_cpu(root),
            ram_bytes: read_ram(root),
            gpus: read_gpus(root),
            computer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::PixelRect;

    // the card's driver is a symbolic link, as in /sys
    #[cfg(unix)]
    #[test]
    fn a_linux_computer_is_read_from_its_files() {
        use std::fs;
        let root = tempfile::tempdir().unwrap();
        let w = |p: &str, text: &str| {
            let f = root.path().join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, text).unwrap();
        };
        w(
            "etc/os-release",
            "NAME=\"Ubuntu\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\n",
        );
        w("proc/sys/kernel/osrelease", "6.8.0-45-generic\n");
        w("proc/sys/kernel/hostname", "build-pc\n");
        w(
            "proc/cpuinfo",
            "processor\t: 0\nmodel name\t: AMD Ryzen 7 7800X3D 8-Core Processor\n\nprocessor\t: 1\nmodel name\t: AMD Ryzen 7 7800X3D 8-Core Processor\n",
        );
        w(
            "proc/meminfo",
            "MemTotal:       32768000 kB\nMemFree: 1 kB\n",
        );
        // an NVIDIA card whose driver says its version, and the card's outputs left out
        w("sys/class/drm/card0/device/vendor", "0x10de\n");
        w("sys/module/nvidia/version", "560.35.03\n");
        let drivers = root.path().join("sys/bus/pci/drivers/nvidia");
        fs::create_dir_all(&drivers).unwrap();
        std::os::unix::fs::symlink(
            &drivers,
            root.path().join("sys/class/drm/card0/device/driver"),
        )
        .unwrap();
        fs::create_dir_all(root.path().join("sys/class/drm/card0-HDMI-A-1")).unwrap();

        let s = linux::read(root.path());
        assert_eq!(s.os, "Ubuntu 24.04.1 LTS (6.8.0-45-generic)");
        assert_eq!(s.cpu, "AMD Ryzen 7 7800X3D 8-Core Processor (2 threads)");
        assert_eq!(s.ram_bytes, 32_768_000 * 1024);
        assert_eq!(s.computer, "build-pc");
        assert_eq!(
            s.gpus,
            vec![Gpu {
                name: "NVIDIA (nvidia)".into(),
                driver: "nvidia 560.35.03".into()
            }]
        );
        let screens = [
            PixelRect::new(0, 0, 2560, 1440),
            PixelRect::new(2560, 0, 1920, 1080),
        ];
        let v: std::collections::BTreeMap<_, _> = s.values(&screens).into_iter().collect();
        assert_eq!(v["system.ram"], "31 GB");
        assert_eq!(v["system.gpu"], "NVIDIA (nvidia)");
        assert_eq!(v["system.gpu_driver"], "nvidia 560.35.03");
        assert_eq!(
            v["system.screens"],
            "2 screens: 2560×1440 (main), 1920×1080"
        );
    }

    #[test]
    fn what_is_not_known_is_left_out() {
        let v = System::default().values(&[]);
        assert!(v.is_empty(), "{v:?}");
        assert_eq!(
            screens_text(&[PixelRect::new(0, 0, 1920, 1080)]),
            "1920×1080"
        );
        assert_eq!(ram_text(8 * 1024 * 1024 * 1024), "8.0 GB");
    }
}
