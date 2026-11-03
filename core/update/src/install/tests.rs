use super::*;

#[test]
fn windows_runs_the_setup_silently_as_an_update_and_asks_for_a_restart() {
    let p = plan(Platform::Windows, Path::new(r"C:\Temp\setup.exe"), None).unwrap();
    assert_eq!(p.program, PathBuf::from(r"C:\Temp\setup.exe"));
    assert_eq!(p.args, ["/S", "/UPDATE", "/R"]);
}

#[test]
fn linux_runs_the_packaged_script_apart_from_this_program_and_restarts() {
    let p = plan(
        Platform::Linux,
        Path::new("/tmp/x.tar.gz"),
        Some(Path::new("/tmp/work/CargoFrame-0.2.0-linux-x64/install.sh")),
    )
    .unwrap();
    assert_eq!(p.program, PathBuf::from("setsid"));
    assert_eq!(
        p.args,
        [
            "--fork",
            "bash",
            "/tmp/work/CargoFrame-0.2.0-linux-x64/install.sh",
            "--restart"
        ]
    );
    assert!(plan(Platform::Linux, Path::new("/tmp/x.tar.gz"), None).is_none());
}

#[test]
fn the_script_is_found_in_the_packages_own_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert!(find_script(dir.path()).is_none());
    let inner = dir.path().join("CargoFrame-0.2.0-linux-x64");
    fs::create_dir_all(&inner).unwrap();
    fs::write(inner.join("install.sh"), "#!/bin/sh\n").unwrap();
    assert_eq!(find_script(dir.path()), Some(inner.join("install.sh")));
}

#[test]
fn another_platforms_installer_is_not_run() {
    let other = if cfg!(windows) {
        Platform::Linux
    } else {
        Platform::Windows
    };
    let dir = tempfile::tempdir().unwrap();
    let err = install(other, &dir.path().join("x"), dir.path()).unwrap_err();
    assert_eq!(err, UpdateError::Unsupported);
}

#[cfg(target_os = "linux")]
#[test]
fn a_linux_package_is_unpacked_and_its_script_started_on_its_own() {
    if Platform::current() != Some(Platform::Linux) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path().join("pkg");
    let inner = pkg.join("CargoFrame-0.2.0-linux-x64");
    fs::create_dir_all(&inner).unwrap();
    let marker = dir.path().join("marker.txt");
    fs::write(
        inner.join("install.sh"),
        format!(
            "#!/usr/bin/env bash\necho \"$@\" > '{}'\n",
            marker.display()
        ),
    )
    .unwrap();
    let tarball = dir.path().join("CargoFrame-0.2.0-linux-x64.tar.gz");
    let made = Command::new("tar")
        .arg("-czf")
        .arg(&tarball)
        .arg("-C")
        .arg(&pkg)
        .arg("CargoFrame-0.2.0-linux-x64")
        .status()
        .unwrap();
    assert!(made.success());

    install(Platform::Linux, &tarball, &dir.path().join("work")).unwrap();

    for _ in 0..50 {
        if marker.exists() && !fs::read_to_string(&marker).unwrap_or_default().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "--restart");
}

#[cfg(target_os = "linux")]
#[test]
fn a_package_that_is_not_a_package_is_an_install_error() {
    if Platform::current() != Some(Platform::Linux) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.tar.gz");
    fs::write(&bad, b"this is not a tarball").unwrap();
    let err = install(Platform::Linux, &bad, &dir.path().join("work")).unwrap_err();
    assert!(matches!(err, UpdateError::Install(_)), "{err:?}");
}
