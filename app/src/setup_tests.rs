use super::*;

fn found() -> Result<(PathBuf, String), String> {
    Ok((PathBuf::from("/usr/bin/ffmpeg"), "7.1".into()))
}

fn missing() -> Result<(PathBuf, String), String> {
    Err("ffmpeg was not found. Install it (on Windows: winget install Gyan.FFmpeg) or choose the folder it is in.".into())
}

fn probes() -> Probes {
    Probes {
        ffmpeg: found(),
        winget: true,
        loopback: Some(true),
        sound_system: true,
        xdotool: true,
        packages: Packages::Apt,
    }
}

fn item<'a>(r: &'a Report, id: &str) -> &'a Item {
    r.items.iter().find(|i| i.id == id).unwrap()
}

#[test]
fn a_ready_windows_pc_has_nothing_to_install() {
    let r = report(Os::Windows, &probes(), false, None);
    assert_eq!(r.missing, 0);
    assert_eq!(r.install, Install::None);
    assert!(r.items.iter().all(|i| i.status == Status::Ok));
    assert_eq!(item(&r, "ffmpeg").detail, "Version 7.1, /usr/bin/ffmpeg");
}

#[test]
fn without_ffmpeg_windows_offers_one_button_that_installs_it() {
    let p = Probes {
        ffmpeg: missing(),
        loopback: None,
        ..probes()
    };
    let r = report(Os::Windows, &p, false, None);
    assert_eq!(r.missing, 1);
    assert_eq!(
        r.install,
        Install::Run {
            label: "Install ffmpeg".into()
        }
    );
    assert_eq!(item(&r, "ffmpeg").status, Status::Missing);
    // only the first sentence of the long explanation is shown on the line
    assert_eq!(item(&r, "ffmpeg").detail, "ffmpeg was not found.");
    // what depends on it cannot be told yet, and is not counted as missing
    assert_eq!(item(&r, "computer_sound").status, Status::Unknown);
}

#[test]
fn without_winget_the_button_says_what_to_do_instead() {
    let p = Probes {
        ffmpeg: missing(),
        winget: false,
        loopback: None,
        ..probes()
    };
    let Install::Manual { text } = report(Os::Windows, &p, false, None).install else {
        panic!("expected a manual step");
    };
    assert!(
        text.contains("App Installer") && text.contains("Video"),
        "{text}"
    );
}

#[test]
fn a_pc_with_no_loopback_device_opens_the_sound_settings_and_installs_nothing() {
    let p = Probes {
        loopback: Some(false),
        ..probes()
    };
    let r = report(Os::Windows, &p, false, None);
    let sound = item(&r, "computer_sound");
    assert_eq!(sound.status, Status::Missing);
    assert_eq!(sound.action, Some("sound_settings"));
    assert!(sound.detail.contains("Stereo Mix"));
    assert_eq!(r.missing, 1);
    // nothing winget can fix: the button does not claim to
    assert_eq!(r.install, Install::None);
}

#[test]
fn an_install_in_progress_or_failed_is_shown_and_a_stale_failure_is_not() {
    let p = Probes {
        ffmpeg: missing(),
        loopback: None,
        ..probes()
    };
    assert_eq!(
        report(Os::Windows, &p, true, None).install,
        Install::Running
    );
    assert_eq!(
        report(Os::Windows, &p, false, Some("winget said: no network")).install,
        Install::Failed {
            message: "winget said: no network".into()
        }
    );
    // ffmpeg turned up (installed by hand, say): the old failure is not shown
    assert_eq!(
        report(
            Os::Windows,
            &probes(),
            false,
            Some("winget said: no network")
        )
        .install,
        Install::None
    );
}

#[test]
fn linux_gets_one_command_for_everything_missing() {
    let all = Probes {
        ffmpeg: missing(),
        xdotool: false,
        winget: false,
        loopback: None,
        ..probes()
    };
    let r = report(Os::Linux, &all, false, None);
    assert_eq!(r.missing, 2);
    assert_eq!(
        r.install,
        Install::Copy {
            command: "sudo apt install ffmpeg xdotool".into()
        }
    );
    let only_xdotool = Probes {
        xdotool: false,
        ..probes()
    };
    assert_eq!(
        report(Os::Linux, &only_xdotool, false, None).install,
        Install::Copy {
            command: "sudo apt install xdotool".into()
        }
    );
    let fedora = Probes {
        packages: Packages::Dnf,
        ..all.clone()
    };
    assert_eq!(
        report(Os::Linux, &fedora, false, None).install,
        Install::Copy {
            command: "sudo dnf install ffmpeg xdotool".into()
        }
    );
    // a system whose package manager is not known is told in words
    let unknown = Probes {
        packages: Packages::Unknown,
        ..all
    };
    let Install::Manual { text } = report(Os::Linux, &unknown, false, None).install else {
        panic!("expected a manual step");
    };
    assert_eq!(
        text,
        "Install ffmpeg and xdotool with your package manager."
    );
}

#[test]
fn linux_without_a_sound_system_says_so_but_has_no_command_for_it() {
    let p = Probes {
        sound_system: false,
        ..probes()
    };
    let r = report(Os::Linux, &p, false, None);
    assert_eq!(item(&r, "sound_system").status, Status::Missing);
    assert_eq!(r.missing, 1);
    assert_eq!(r.install, Install::None);
}

#[test]
fn the_package_manager_is_read_from_the_os_release_file() {
    let ubuntu = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
    let mint = "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
    let fedora = "ID=fedora\n";
    let rocky = "ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n";
    let arch = "ID=arch\n";
    let manjaro = "ID=manjaro\nID_LIKE=arch\n";
    let suse = "ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n";
    for (text, want) in [
        (ubuntu, Packages::Apt),
        (mint, Packages::Apt),
        (fedora, Packages::Dnf),
        (rocky, Packages::Dnf),
        (arch, Packages::Pacman),
        (manjaro, Packages::Pacman),
        (suse, Packages::Zypper),
        ("ID=nixos\n", Packages::Unknown),
        ("", Packages::Unknown),
    ] {
        assert_eq!(Packages::from_os_release(text), want, "{text}");
    }
    assert_eq!(Packages::Unknown.command(&["ffmpeg"]), None);
}

#[test]
fn winget_is_run_for_the_one_fixed_package_and_never_asks_a_question() {
    let args = winget_args();
    assert_eq!(&args[..3], ["install", "--id", "Gyan.FFmpeg"]);
    for flag in [
        "--exact",
        "--silent",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--disable-interactivity",
    ] {
        assert!(args.contains(&flag), "{flag}");
    }
    // nothing but the package id names what is installed
    assert_eq!(args.iter().filter(|a| a.contains('.')).count(), 1);
}

#[test]
fn what_winget_said_is_shortened_to_the_last_lines_without_progress_bars() {
    let said = "Found FFmpeg [Gyan.FFmpeg] Version 7.1\n  ██████▒▒▒▒  45%\n\nDownloading https://example.invalid/x.zip\nInstaller hash does not match; this cannot be installed.\n";
    assert_eq!(
        winget_failure(said, Some(1)),
        "winget said: Downloading https://example.invalid/x.zip Installer hash does not match; this cannot be installed."
    );
    assert_eq!(
        winget_failure("", Some(2316632107u32 as i32)),
        "winget stopped with code -1978335189. Try again, or install ffmpeg by hand."
    );
    assert_eq!(
        winget_failure("  \n", None),
        "winget stopped before it finished."
    );
    // a long line is cut
    assert!(winget_failure(&"x".repeat(2000), Some(1)).chars().count() < 330);
}

#[test]
fn installs_run_one_at_a_time_and_remember_how_the_last_one_went() {
    let installer = Installer::default();
    let (release, hold) = std::sync::mpsc::channel::<()>();
    installer
        .start(move || {
            hold.recv().ok();
            Err("no network".into())
        })
        .unwrap();
    assert!(installer.running());
    assert_eq!(
        installer.start(|| Ok(())).unwrap_err(),
        "An install is already running."
    );
    release.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while installer.running() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!installer.running());
    assert_eq!(installer.failure().as_deref(), Some("no network"));

    // a new install clears the failure, and one that works leaves none
    installer.start(|| Ok(())).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while installer.running() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(installer.failure(), None);

    // a failure can be dropped when the thing turned up some other way
    installer.start(|| Err("x".into())).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while installer.running() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(installer.failure().is_some());
    installer.clear_failure();
    assert_eq!(installer.failure(), None);
}

#[test]
fn the_setup_page_opens_by_itself_once() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    assert!(first_run(&config), "the first time");
    assert!(!first_run(&config), "never again");
    // a place that cannot be written to does not make it open every time
    let file = dir.path().join("a-file");
    std::fs::write(&file, b"x").unwrap();
    assert!(!first_run(&file.join("config")));
}

#[test]
fn winget_shortcuts_are_looked_in_only_on_windows() {
    if cfg!(windows) {
        assert!(winget_links().unwrap().ends_with("WinGet/Links"));
    } else {
        assert_eq!(winget_links(), None);
    }
}

#[test]
fn looking_at_this_computer_never_panics() {
    let video = cargoframe_config::Video {
        ffmpeg: Some("/no/such/folder".into()),
        ..Default::default()
    };
    let p = probe(Os::current(), &video);
    assert!(p.ffmpeg.is_err());
    let r = report(Os::current(), &p, false, None);
    assert!(r.missing >= 1);
}
