//! Reading real pictures: on Linux with tesseract installed (skipped where it is not).

#[cfg(target_os = "linux")]
fn has_tesseract() -> bool {
    std::process::Command::new("tesseract")
        .arg("--version")
        .output()
        .is_ok()
}

#[cfg(target_os = "linux")]
#[test]
fn reads_light_text_on_a_dark_ground_and_says_where_it_is() {
    if !has_tesseract() {
        return;
    }
    let png = include_bytes!("fixtures/dark.png");
    let r = cargoframe_ocr::read(png, 0).unwrap();
    assert_eq!(r.text, "Build 4.2.1 failed on save");
    assert_eq!(r.reader, "tesseract");
    let l = &r.lines[0];
    // in the pixels of the picture given (520×70), around the text
    assert!(
        l.x < 30 && l.y < 30 && l.x + l.w <= 520 && l.y + l.h <= 70,
        "{l:?}"
    );
    assert!(l.w > 300, "{l:?}");
}

#[cfg(target_os = "linux")]
#[test]
fn every_try_reads_small_text() {
    if !has_tesseract() {
        return;
    }
    let png = include_bytes!("fixtures/small.png");
    for attempt in 0..cargoframe_ocr::TRIES {
        let r = cargoframe_ocr::read(png, attempt).unwrap();
        assert_eq!(r.attempt, attempt);
        assert!(
            r.text.contains("file not found"),
            "try {attempt}: {:?}",
            r.text
        );
    }
}

#[test]
fn a_broken_picture_is_said_to_be_one() {
    let e = cargoframe_ocr::read(b"nope", 0).unwrap_err();
    assert!(e.to_string().contains("picture"), "{e}");
}
