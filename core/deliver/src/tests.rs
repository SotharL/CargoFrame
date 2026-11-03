use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn win(id: u64, title: &str, program: &str) -> Window {
    Window {
        id,
        title: title.into(),
        program: Some(program.into()),
    }
}

fn claude() -> Window {
    win(10, "Claude", "claude")
}

fn cargoframe() -> Window {
    win(1, "CargoFrame", "cargoframe")
}

/// A pretend desktop. `front` says which window is in front, given the time that has passed
/// and the number of key presses so far.
struct FakeDesk {
    log: Log,
    now: Cell<Duration>,
    presses: Cell<usize>,
    front: Box<dyn Fn(Duration, usize) -> Option<Window>>,
    can_files: bool,
    fail_files: bool,
    fail_keys: bool,
}

impl FakeDesk {
    fn new(front: impl Fn(Duration, usize) -> Option<Window> + 'static) -> (Self, Log) {
        let log: Log = Rc::default();
        (
            Self {
                log: log.clone(),
                now: Cell::new(Duration::ZERO),
                presses: Cell::new(0),
                front: Box::new(front),
                can_files: true,
                fail_files: false,
                fail_keys: false,
            },
            log,
        )
    }

    /// Claude is in front from the start.
    fn ready() -> (Self, Log) {
        Self::new(|_, _| Some(claude()))
    }

    fn note(&self, what: String) {
        self.log.borrow_mut().push(what);
    }
}

impl Desk for FakeDesk {
    fn foreground(&self) -> Result<Option<Window>, String> {
        Ok((self.front)(self.now.get(), self.presses.get()))
    }

    fn can_paste_files(&self) -> bool {
        self.can_files
    }

    fn put_files(&self, files: &[PathBuf]) -> Result<(), String> {
        if self.fail_files {
            return Err("locked".into());
        }
        let names: Vec<_> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        self.note(format!("files:{}", names.join(",")));
        Ok(())
    }

    fn put_text(&self, text: &str) -> Result<(), String> {
        self.note(format!("text:{text}"));
        Ok(())
    }

    fn press(&self, key: Key) -> Result<(), String> {
        if self.fail_keys {
            return Err("blocked".into());
        }
        self.presses.set(self.presses.get() + 1);
        self.note(format!("press:{key:?}@{}ms", self.now.get().as_millis()));
        Ok(())
    }

    fn sleep(&self, d: Duration) {
        self.now.set(self.now.get() + d);
    }
}

struct Files {
    _dir: tempfile::TempDir,
    paths: Vec<PathBuf>,
}

/// Files of the given sizes in bytes; the first is "screenshot.png".
fn files(sizes: &[u64]) -> Files {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for (i, size) in sizes.iter().enumerate() {
        let name = if i == 0 {
            "screenshot.png".to_string()
        } else {
            format!("file{i}.log")
        };
        let p = dir.path().join(name);
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(*size).unwrap();
        paths.push(p);
    }
    Files { _dir: dir, paths }
}

fn delivery(f: &Files, text: &str) -> Delivery {
    Delivery {
        open_url: "claude://claude.ai/chat/0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0".into(),
        target: Target::App,
        files: f.paths.clone(),
        text: text.into(),
        submit: true,
        timing: Timing::for_target(Target::App, Duration::ZERO),
    }
}

fn run(
    desk: &FakeDesk,
    log: &Log,
    d: &Delivery,
    cancelled: &dyn Fn() -> bool,
) -> Result<Delivered, DeliverError> {
    let opener = |url: &str| {
        log.borrow_mut().push(format!("open:{url}"));
        Ok(())
    };
    deliver(desk, &opener, d, cancelled)
}

fn logged(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

fn kinds(log: &Log) -> Vec<String> {
    // The log without the timestamps on the key presses.
    logged(log)
        .into_iter()
        .map(|l| l.split('@').next().unwrap().to_string())
        .collect()
}

#[test]
fn opens_the_chat_pastes_the_files_then_the_text_and_presses_enter() {
    let f = files(&[1000, 2000]);
    let (desk, log) = FakeDesk::ready();
    let done = run(&desk, &log, &delivery(&f, "Why does it fail?"), &|| false).unwrap();
    assert_eq!(
        kinds(&log),
        vec![
            "open:claude://claude.ai/chat/0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0",
            "files:screenshot.png,file1.log",
            "press:Paste",
            "text:Why does it fail?",
            "press:Paste",
            "press:Enter",
        ]
    );
    assert_eq!(
        done,
        Delivered {
            window: "Claude".into(),
            files: 2,
            skipped_files: 0,
            text: true,
            submitted: true
        }
    );
}

#[test]
fn nothing_is_pasted_before_claude_is_in_front_and_has_had_time_to_load() {
    let f = files(&[1000]);
    // CargoFrame is in front for the first 3 seconds, then Claude.
    let (desk, log) = FakeDesk::new(|t, _| {
        Some(if t < Duration::from_secs(3) {
            cargoframe()
        } else {
            claude()
        })
    });
    let d = delivery(&f, "hi");
    run(&desk, &log, &d, &|| false).unwrap();
    let first_paste = logged(&log)
        .into_iter()
        .find(|l| l.starts_with("press:Paste"))
        .unwrap();
    let at: u128 = first_paste
        .split('@')
        .nth(1)
        .unwrap()
        .trim_end_matches("ms")
        .parse()
        .unwrap();
    let earliest = Duration::from_secs(3) + d.timing.settle;
    assert!(
        at >= earliest.as_millis(),
        "pasted at {at} ms, before Claude had settled at {} ms",
        earliest.as_millis()
    );
}

#[test]
fn if_claude_never_comes_to_the_front_nothing_is_pasted() {
    let f = files(&[1000]);
    let (desk, log) = FakeDesk::new(|_, _| Some(win(2, "Inbox - Mail", "outlook")));
    let err = run(&desk, &log, &delivery(&f, "hi"), &|| false).unwrap_err();
    let DeliverError::NotInFront { waited, seen, .. } = &err else {
        panic!("{err:?}")
    };
    assert!(*waited >= Duration::from_secs(25));
    assert_eq!(seen.as_deref(), Some("Inbox - Mail"));
    assert!(err.to_string().contains("nothing was pasted"));
    assert!(err.to_string().contains("Inbox - Mail"));
    assert_eq!(
        kinds(&log).len(),
        1,
        "only the link was opened: {:?}",
        logged(&log)
    );
}

#[test]
fn the_window_must_be_the_right_kind_of_window() {
    let app = Target::App;
    let browser = Target::Browser;
    // The Claude app.
    assert!(app.accepts(&win(1, "Claude", "claude")));
    assert!(!app.accepts(&win(1, "", "claude")));
    // A terminal whose title says claude is not the app.
    assert!(!app.accepts(&win(1, "claude", "windowsterminal")));
    assert!(!app.accepts(&win(1, "Claude", "cargoframe")));
    assert!(!app.accepts(&Window {
        id: 1,
        title: "Claude".into(),
        program: None
    }));
    // A browser showing Claude.
    assert!(browser.accepts(&win(1, "Bug report - Claude - Google Chrome", "chrome")));
    assert!(browser.accepts(&win(1, "Claude \u{2014} Mozilla Firefox", "firefox")));
    assert!(browser.accepts(&win(
        1,
        "Claude and 2 more pages - Microsoft Edge",
        "msedge"
    )));
    // Not a browser, or not Claude.
    assert!(!browser.accepts(&win(1, "claude", "windowsterminal")));
    assert!(!browser.accepts(&win(1, "claude - bash", "gnome-terminal-server")));
    assert!(!browser.accepts(&win(1, "Inbox - Google Chrome", "chrome")));
    assert!(!browser.accepts(&win(1, "Claude", "notepad")));
}

#[test]
fn clicking_away_before_the_text_stops_it_and_enter_is_never_pressed() {
    let f = files(&[1000]);
    // After the files are pasted (1 key press) the person clicks into another window.
    let (desk, log) = FakeDesk::new(|_, presses| {
        Some(if presses >= 1 {
            win(2, "Notes", "notepad")
        } else {
            claude()
        })
    });
    let err = run(&desk, &log, &delivery(&f, "secret plan"), &|| false).unwrap_err();
    assert!(
        matches!(err, DeliverError::LostFocus { stage: "write", .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("Notes"));
    let log = kinds(&log);
    assert!(!log.iter().any(|l| l.starts_with("text:")), "{log:?}");
    assert!(!log.iter().any(|l| l == "press:Enter"), "{log:?}");
}

#[test]
fn clicking_away_while_files_upload_means_enter_is_not_pressed() {
    let f = files(&[1000, 5 * 1024 * 1024]);
    // Two pastes are done; the person switches window after that.
    let (desk, log) = FakeDesk::new(|_, presses| {
        Some(if presses >= 2 {
            win(2, "Mail", "outlook")
        } else {
            claude()
        })
    });
    let err = run(&desk, &log, &delivery(&f, "look"), &|| false).unwrap_err();
    assert!(
        matches!(
            err,
            DeliverError::LostFocus {
                stage: "upload",
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("Enter was not pressed"));
    assert!(!kinds(&log).iter().any(|l| l == "press:Enter"));
}

#[test]
fn another_window_of_the_same_kind_is_not_the_same_window() {
    let f = files(&[1000]);
    let (desk, log) = FakeDesk::new(|_, presses| {
        Some(if presses >= 1 {
            win(11, "Claude", "claude") // a second Claude window
        } else {
            claude()
        })
    });
    let err = run(&desk, &log, &delivery(&f, "x"), &|| false).unwrap_err();
    assert!(matches!(err, DeliverError::LostFocus { .. }), "{err:?}");
}

#[test]
fn losing_the_window_while_the_chat_loads_pastes_nothing() {
    let f = files(&[1000]);
    let (desk, log) = FakeDesk::new(|t, _| {
        Some(if t < Duration::from_millis(1000) {
            claude()
        } else {
            win(2, "Notes", "notepad")
        })
    });
    let err = run(&desk, &log, &delivery(&f, "x"), &|| false).unwrap_err();
    assert!(
        matches!(err, DeliverError::LostFocus { stage: "wait", .. }),
        "{err:?}"
    );
    assert_eq!(kinds(&log).len(), 1, "{:?}", logged(&log));
}

#[test]
fn with_submit_off_the_message_waits_in_the_message_box() {
    let f = files(&[1000]);
    let (desk, log) = FakeDesk::ready();
    let mut d = delivery(&f, "check");
    d.submit = false;
    let done = run(&desk, &log, &d, &|| false).unwrap();
    assert!(!done.submitted);
    assert!(!kinds(&log).iter().any(|l| l == "press:Enter"));
    assert!(kinds(&log).iter().any(|l| l == "text:check"));
}

#[test]
fn text_only_and_files_only_both_work() {
    let (desk, log) = FakeDesk::ready();
    let none = files(&[]);
    run(&desk, &log, &delivery(&none, "just words"), &|| false).unwrap();
    assert_eq!(
        kinds(&log)[1..],
        ["text:just words", "press:Paste", "press:Enter"]
    );

    let f = files(&[10]);
    let (desk, log) = FakeDesk::ready();
    run(&desk, &log, &delivery(&f, "  "), &|| false).unwrap();
    assert_eq!(
        kinds(&log)[1..],
        ["files:screenshot.png", "press:Paste", "press:Enter"]
    );
}

#[test]
fn missing_files_and_empty_deliveries_are_refused_before_anything_is_opened() {
    let (desk, log) = FakeDesk::ready();
    let none = files(&[]);
    let err = run(&desk, &log, &delivery(&none, "  "), &|| false).unwrap_err();
    assert_eq!(err, DeliverError::NothingToSend);

    let f = files(&[10]);
    let mut d = delivery(&f, "x");
    d.files.push(f.paths[0].with_file_name("gone.log"));
    let err = run(&desk, &log, &d, &|| false).unwrap_err();
    assert!(matches!(err, DeliverError::MissingFile(p) if p.contains("gone.log")));
    // A folder is not a file either.
    d.files = vec![f.paths[0].parent().unwrap().to_path_buf()];
    assert!(matches!(
        run(&desk, &log, &d, &|| false).unwrap_err(),
        DeliverError::MissingFile(_)
    ));
    assert!(logged(&log).is_empty(), "nothing was opened or pasted");
}

#[test]
fn a_link_that_cannot_be_opened_stops_everything() {
    let f = files(&[10]);
    let (desk, log) = FakeDesk::ready();
    let d = delivery(&f, "x");
    let err = deliver(&desk, &|_| Err("no handler".into()), &d, &|| false).unwrap_err();
    assert_eq!(err, DeliverError::Open("no handler".into()));
    assert!(err.to_string().contains("Could not open Claude"));
    assert!(logged(&log).is_empty());
}

#[test]
fn a_system_that_cannot_paste_files_sends_the_text_and_says_so() {
    let f = files(&[10, 20]);
    let (mut desk, log) = FakeDesk::ready();
    desk.can_files = false;
    let done = run(&desk, &log, &delivery(&f, "words"), &|| false).unwrap();
    assert_eq!(done.files, 0);
    assert_eq!(done.skipped_files, 2);
    assert!(done.text);
    assert!(!kinds(&log).iter().any(|l| l.starts_with("files:")));
}

#[test]
fn clipboard_and_key_failures_are_reported_and_stop_the_sequence() {
    let f = files(&[10]);
    let (mut desk, log) = FakeDesk::ready();
    desk.fail_files = true;
    let err = run(&desk, &log, &delivery(&f, "x"), &|| false).unwrap_err();
    assert_eq!(err, DeliverError::Clipboard("locked".into()));
    assert!(!kinds(&log).iter().any(|l| l.starts_with("press")));

    let (mut desk, log) = FakeDesk::ready();
    desk.fail_keys = true;
    let err = run(&desk, &log, &delivery(&f, "x"), &|| false).unwrap_err();
    assert_eq!(err, DeliverError::Keys("blocked".into()));
}

#[test]
fn giving_up_stops_the_wait() {
    let f = files(&[10]);
    let (desk, log) = FakeDesk::new(|_, _| Some(cargoframe()));
    let err = run(&desk, &log, &delivery(&f, "x"), &|| true).unwrap_err();
    assert_eq!(err, DeliverError::Cancelled);
}

#[test]
fn big_uploads_get_more_time_before_enter() {
    let gap = |bytes: u64| {
        let f = files(&[10, bytes]);
        let (desk, log) = FakeDesk::ready();
        run(&desk, &log, &delivery(&f, "x"), &|| false).unwrap();
        let at = |what: &str| -> u128 {
            logged(&log)
                .iter()
                .rev()
                .find(|l| l.starts_with(what))
                .map(|l| {
                    l.split('@')
                        .nth(1)
                        .unwrap()
                        .trim_end_matches("ms")
                        .parse()
                        .unwrap()
                })
                .unwrap()
        };
        at("press:Enter") - at("press:Paste")
    };
    let small = gap(100 * 1024);
    let big = gap(20 * 1024 * 1024);
    assert!(big > small + 10_000, "small {small} ms, big {big} ms");
    // But never more than the limit, however large.
    let huge = gap(200 * 1024 * 1024);
    assert!(huge <= 45_000 + 1_000, "{huge}");
}

#[test]
fn the_browser_waits_longer_and_the_extra_wait_is_added() {
    let app = Timing::for_target(Target::App, Duration::ZERO);
    let browser = Timing::for_target(Target::Browser, Duration::ZERO);
    assert!(browser.settle > app.settle);
    let slow = Timing::for_target(Target::App, Duration::from_secs(5));
    assert_eq!(slow.settle, app.settle + Duration::from_secs(5));
}
