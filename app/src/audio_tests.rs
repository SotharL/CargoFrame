use super::*;

const PULSE: &str = "Auto-detected sources for pulse:
* alsa_output.pci-0000_00_1f.3.analog-stereo.monitor [Monitor of Built-in Audio Analog Stereo]
  alsa_input.pci-0000_00_1f.3.analog-stereo [Built-in Audio Analog Stereo]
  bluez_input.00_1B_66_00_00_00.0 [Headset (Bluetooth)]
";

const DSHOW_OLD: &str = "[dshow @ 000001a2b3c4d5e0] DirectShow video devices (some may be both video and audio devices)
[dshow @ 000001a2b3c4d5e0]  \"Integrated Camera\"
[dshow @ 000001a2b3c4d5e0]     Alternative name \"@device_pnp_\\\\?\\usb#vid_0bda\"
[dshow @ 000001a2b3c4d5e0] DirectShow audio devices
[dshow @ 000001a2b3c4d5e0]  \"Microphone (Realtek(R) Audio)\"
[dshow @ 000001a2b3c4d5e0]     Alternative name \"@device_cm_{33D9A762-90C8-11D0-BD43-00A0C911CE86}\\wave_{AAAA}\"
[dshow @ 000001a2b3c4d5e0]  \"Stereo Mix (Realtek(R) Audio)\"
[dshow @ 000001a2b3c4d5e0]     Alternative name \"@device_cm_{33D9A762}\\wave_{BBBB}\"
dummy: Immediate exit requested
";

const DSHOW_NEW: &str = "[dshow @ 000002] \"Integrated Camera\" (video)
[dshow @ 000002]   Alternative name \"@device_pnp_x\"
[dshow @ 000002] \"Microphone (USB Audio)\" (audio)
[dshow @ 000002]   Alternative name \"@device_cm_x\"
[dshow @ 000002] \"virtual-audio-capturer\" (audio)
[dshow @ 000002]   Alternative name \"@device_sw_x\"
[dshow @ 000002] \"Capture card\" (none)
[in#0 @ 0000] Error opening input file dummy.
";

fn names(d: &[Device]) -> Vec<&str> {
    d.iter().map(|x| x.name.as_str()).collect()
}

fn off() -> VideoAudio {
    VideoAudio {
        desktop: AudioChoice::Off,
        microphone: AudioChoice::Off,
    }
}

fn no_listing() -> Devices {
    panic!("the system was asked for its devices when it did not have to be")
}

#[test]
fn pulse_monitors_are_what_the_computer_plays_and_the_rest_are_microphones() {
    let d = parse_pulse(PULSE);
    assert!(d.listed);
    assert_eq!(
        names(&d.desktop),
        ["alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"]
    );
    assert_eq!(
        d.desktop[0].label,
        "Monitor of Built-in Audio Analog Stereo"
    );
    assert!(d.desktop[0].default);
    assert_eq!(
        names(&d.microphone),
        [
            "alsa_input.pci-0000_00_1f.3.analog-stereo",
            "bluez_input.00_1B_66_00_00_00.0"
        ]
    );
    assert!(!d.microphone[0].default);
    // nothing listed is not an error, only an empty list
    assert!(parse_pulse("Auto-detected sources for pulse:\n")
        .desktop
        .is_empty());
    assert!(parse_pulse("").microphone.is_empty());
}

#[test]
fn directshow_lists_are_read_in_both_forms_and_loopbacks_are_told_apart() {
    for text in [DSHOW_OLD, DSHOW_NEW] {
        let d = parse_dshow(text);
        assert!(d.listed);
        assert!(
            !names(&d.microphone).iter().any(|n| n.contains("Camera")),
            "a camera is not a sound device: {d:?}"
        );
    }
    let old = parse_dshow(DSHOW_OLD);
    assert_eq!(names(&old.microphone), ["Microphone (Realtek(R) Audio)"]);
    assert_eq!(names(&old.desktop), ["Stereo Mix (Realtek(R) Audio)"]);
    let new = parse_dshow(DSHOW_NEW);
    assert_eq!(names(&new.microphone), ["Microphone (USB Audio)"]);
    assert_eq!(names(&new.desktop), ["virtual-audio-capturer"]);
    // alternative names are never devices of their own
    assert!(!names(&old.microphone)
        .iter()
        .chain(names(&old.desktop).iter())
        .any(|n| n.starts_with("@device")));
    assert!(parse_dshow("").desktop.is_empty());
}

#[test]
fn the_default_sound_on_linux_is_the_monitor_and_no_microphone_unless_asked() {
    let p = plan(&VideoAudio::default(), Os::Linux, true, &no_listing);
    assert_eq!(
        p.sources,
        [AudioSource {
            backend: AudioBackend::Pulse,
            device: DEFAULT_MONITOR.into()
        }]
    );
    assert!(p.notes.is_empty());

    let p = plan(
        &VideoAudio {
            desktop: AudioChoice::Default,
            microphone: AudioChoice::Default,
        },
        Os::Linux,
        true,
        &no_listing,
    );
    assert_eq!(
        p.sources
            .iter()
            .map(|s| s.device.as_str())
            .collect::<Vec<_>>(),
        [DEFAULT_MONITOR, DEFAULT_SOURCE]
    );

    assert!(plan(&off(), Os::Linux, true, &no_listing)
        .sources
        .is_empty());
}

#[test]
fn a_named_device_is_used_as_it_is_without_asking_the_system() {
    let p = plan(
        &VideoAudio {
            desktop: AudioChoice::Device("alsa_output.usb.monitor".into()),
            microphone: AudioChoice::Device("USB Mic".into()),
        },
        Os::Windows,
        true,
        &no_listing,
    );
    assert_eq!(
        p.sources
            .iter()
            .map(|s| (s.backend, s.device.as_str()))
            .collect::<Vec<_>>(),
        [
            (AudioBackend::Dshow, "alsa_output.usb.monitor"),
            (AudioBackend::Dshow, "USB Mic")
        ]
    );
    // the same device twice is one input
    let p = plan(
        &VideoAudio {
            desktop: AudioChoice::Device("X".into()),
            microphone: AudioChoice::Device("X".into()),
        },
        Os::Linux,
        true,
        &no_listing,
    );
    assert_eq!(p.sources.len(), 1);
}

#[test]
fn on_windows_the_default_sound_is_the_loopback_device_and_its_absence_is_said() {
    let with = || parse_dshow(DSHOW_OLD);
    let p = plan(&VideoAudio::default(), Os::Windows, true, &with);
    assert_eq!(
        p.sources,
        [AudioSource {
            backend: AudioBackend::Dshow,
            device: "Stereo Mix (Realtek(R) Audio)".into()
        }]
    );
    assert!(p.notes.is_empty());

    let p = plan(
        &VideoAudio {
            desktop: AudioChoice::Off,
            microphone: AudioChoice::Default,
        },
        Os::Windows,
        true,
        &with,
    );
    assert_eq!(p.sources[0].device, "Microphone (Realtek(R) Audio)");

    // a PC with no loopback device records without the computer's sound, and says why
    let none = || parse_dshow("[dshow @ 1] \"Microphone (USB)\" (audio)\n");
    let p = plan(&VideoAudio::default(), Os::Windows, true, &none);
    assert!(p.sources.is_empty());
    assert_eq!(p.notes, [LOOPBACK_MISSING]);
    assert!(LOOPBACK_MISSING.contains("Stereo Mix"));

    // and no microphone is said too
    let p = plan(
        &VideoAudio {
            desktop: AudioChoice::Off,
            microphone: AudioChoice::Default,
        },
        Os::Windows,
        true,
        &|| parse_dshow(""),
    );
    assert_eq!(p.notes, ["No microphone was found."]);
}

#[test]
fn on_an_unsupported_system_sound_is_left_out_with_a_word() {
    let p = plan(&VideoAudio::default(), Os::Other, true, &no_listing);
    assert!(p.sources.is_empty());
    assert_eq!(p.notes.len(), 1);
    assert!(plan(&off(), Os::Other, true, &no_listing).notes.is_empty());
}

#[test]
fn listing_with_no_ffmpeg_says_so_and_never_panics() {
    let d = list(Path::new("/no/such/ffmpeg"), Os::Linux);
    assert!(!d.listed);
    assert!(d.note.unwrap().contains("ffmpeg would not start"));
    let d = list(Path::new("/no/such/ffmpeg"), Os::Other);
    assert!(!d.listed);
    assert!(d.note.is_some());
}

#[test]
fn without_a_sound_system_on_linux_nothing_is_asked_of_ffmpeg() {
    let p = plan(&VideoAudio::default(), Os::Linux, false, &no_listing);
    assert!(p.sources.is_empty());
    assert_eq!(p.notes, [NO_SOUND_SYSTEM]);
    // a layout that wants no sound has nothing to be told
    let p = plan(&off(), Os::Linux, false, &no_listing);
    assert!(p.sources.is_empty() && p.notes.is_empty());
    // the check is for Linux only
    let p = plan(&VideoAudio::default(), Os::Windows, false, &|| {
        parse_dshow(DSHOW_OLD)
    });
    assert_eq!(p.sources.len(), 1);
}
