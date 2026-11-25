//! *Check this PC* (System, debug mode): what CI cannot try, tried on the PC itself.
//!
//! The Windows code paths are compiled and unit-tested on a cloud runner, which has no screens
//! to capture, no sound card and no graphics card. This runs the real thing where CargoFrame is
//! used: the computer's details, a screenshot of every screen, the apps CargoFrame leans on, a
//! three-second recording, the sign-in entry, the credential store, the shortcuts, the settings
//! folder and the web view. Each step is timed and says what it found; the report is kept in the
//! log folder, where the bug report of debug mode attaches it (`debug.rs`).

use crate::state::AppState;
use crate::video;
use cargoframe_capture as cap;
use cargoframe_config::{Store, Video};
use cargoframe_connectors::SecretStore;
use cargoframe_i18n::{t, tf};
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, Manager};

/// The report's file, in the log folder.
pub const REPORT: &str = "pc-check.md";
/// How long the test recording runs.
const RECORD_SECS: u64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Ok,
    /// Works, with something to know (a missing app, another copy at sign-in).
    Warn,
    Fail,
}

/// One step: what was tried, how it went, what it found, how long it took.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub id: &'static str,
    pub name: String,
    pub outcome: Outcome,
    pub detail: String,
    pub ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub steps: Vec<Step>,
    /// The report as written to the log folder (Markdown), to copy.
    pub report: String,
    /// Where it was written, when it could be.
    pub saved: Option<String>,
}

/// Run one step, timed.
fn step(id: &'static str, name: &str, f: impl FnOnce() -> (Outcome, String)) -> Step {
    let began = Instant::now();
    let (outcome, detail) = f();
    let ms = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::info!(step = id, outcome = ?outcome, ms, "pc check: {detail}");
    Step {
        id,
        name: name.to_string(),
        outcome,
        detail,
        ms,
    }
}

/// The report a person can read and a developer can act on.
pub fn report(steps: &[Step], when: &str) -> String {
    let mark = |o: Outcome| match o {
        Outcome::Ok => "✓",
        Outcome::Warn => "!",
        Outcome::Fail => "✗",
    };
    let cell = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut out = format!(
        "# {}\n\nCargoFrame {} · {when}\n\n| | {} | {} | ms |\n| --- | --- | --- | ---: |\n",
        t("CargoFrame check of this PC"),
        env!("CARGO_PKG_VERSION"),
        t("Step"),
        t("What it found"),
    );
    for s in steps {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            mark(s.outcome),
            cell(&s.name),
            cell(&s.detail),
            s.ms
        ));
    }
    out
}

fn system_step() -> Step {
    step("system", t("This computer"), || {
        let v: std::collections::BTreeMap<_, _> = crate::sysinfo::values().into_iter().collect();
        let get = |k: &str| v.get(k).cloned().unwrap_or_default();
        let detail = format!(
            "{} · {} · {} · {} ({}) · {}",
            get("system.os"),
            get("system.cpu"),
            get("system.ram"),
            get("system.gpu"),
            get("system.gpu_driver"),
            get("system.screens"),
        );
        (Outcome::Ok, detail)
    })
}

fn screenshot_step() -> Step {
    step(
        "screenshot",
        t("A screenshot of every screen"),
        || match cap::capture(cap::Mode::AllDisplays) {
            Ok(c) => (
                Outcome::Ok,
                tf(
                    "{w} × {h} pixels",
                    &[("w", &c.frame.width()), ("h", &c.frame.height())],
                ),
            ),
            Err(e) => (Outcome::Fail, e.to_string()),
        },
    )
}

fn apps_steps(app: &AppHandle) -> Vec<Step> {
    let began = Instant::now();
    let report = crate::commands::setup_report(app);
    let ms = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
    report
        .items
        .into_iter()
        .map(|item| {
            let outcome = match item.status {
                crate::setup::Status::Ok => Outcome::Ok,
                _ => Outcome::Warn,
            };
            let detail = if item.detail.is_empty() {
                t(item.what_for).to_string()
            } else {
                item.detail
            };
            Step {
                id: item.id,
                name: t(item.name).to_string(),
                outcome,
                detail,
                ms,
            }
        })
        .collect()
}

fn recording_step(state: &AppState) -> Step {
    step("recording", t("A 3-second recording"), || {
        let video = state
            .store
            .load_or_default::<Video>()
            .map(video::normalized)
            .unwrap_or_default();
        let work = state.store.dirs().data().join("pc-check");
        let trial = state.recording.trial(
            &video,
            video::Os::current(),
            &video::Session::current(),
            &work,
            RECORD_SECS,
        );
        let _ = std::fs::remove_dir_all(&work);
        let detail = format!(
            "{} ({} ms, {} KB)",
            trial.message,
            trial.took_ms,
            trial.clip_bytes / 1024
        );
        (if trial.ok { Outcome::Ok } else { Outcome::Fail }, detail)
    })
}

fn sign_in_step() -> Step {
    step("sign_in", t("Starting when you sign in"), || {
        let others = crate::autostart::others();
        if !others.is_empty() {
            return (
                Outcome::Warn,
                tf(
                    "Another copy starts at sign-in: {paths}",
                    &[("paths", &others.join(", "))],
                ),
            );
        }
        match crate::autostart::is_enabled() {
            Ok(true) => (Outcome::Ok, t("On, for this copy").to_string()),
            Ok(false) => (Outcome::Ok, t("Off").to_string()),
            Err(e) => (Outcome::Fail, e),
        }
    })
}

fn credentials_step() -> Step {
    step("credentials", t("The credential store"), || {
        let store = crate::secrets::KeyringStore;
        let key = "cargoframe-pc-check";
        let tried = store
            .set(key, "check")
            .and_then(|()| store.get(key))
            .and_then(|v| store.delete(key).map(|()| v));
        match tried {
            Ok(Some(v)) if v == "check" => {
                (Outcome::Ok, t("A token is kept and read back").to_string())
            }
            Ok(_) => (
                Outcome::Fail,
                t("A token kept could not be read back").to_string(),
            ),
            Err(e) => (Outcome::Fail, e.to_string()),
        }
    })
}

fn shortcuts_step(state: &AppState) -> Step {
    step("shortcuts", t("The shortcuts"), || {
        let status = state.shortcut_status();
        if status.errors.is_empty() {
            return (Outcome::Ok, t("Every shortcut works").to_string());
        }
        let list: Vec<String> = status
            .errors
            .iter()
            .map(|(id, why)| format!("{id}: {why}"))
            .collect();
        (Outcome::Warn, list.join("; "))
    })
}

fn folder_step(store: &Store) -> Step {
    step("folders", t("The settings folder"), || {
        let probe = store.dirs().config().join(".pc-check");
        match std::fs::write(&probe, b"check").and_then(|()| std::fs::remove_file(&probe)) {
            Ok(()) => (
                Outcome::Ok,
                tf(
                    "Written to: {path}",
                    &[("path", &store.dirs().config().display())],
                ),
            ),
            Err(e) => (Outcome::Fail, e.to_string()),
        }
    })
}

fn webview_step() -> Step {
    step(
        "webview",
        t("The web view the windows use"),
        || match tauri::webview_version() {
            Ok(v) => (Outcome::Ok, v),
            Err(e) => (Outcome::Fail, e.to_string()),
        },
    )
}

/// Run every step, keep the report in the log folder, and give it back.
pub fn run(app: &AppHandle) -> Check {
    let state = app.state::<AppState>();
    let mut steps = vec![system_step(), screenshot_step()];
    steps.extend(apps_steps(app));
    steps.push(recording_step(&state));
    steps.push(sign_in_step());
    steps.push(credentials_step());
    steps.push(shortcuts_step(&state));
    steps.push(folder_step(&state.store));
    steps.push(webview_step());
    let when = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let report = report(&steps, &when);
    let path = state.store.dirs().logs().join(REPORT);
    let saved = std::fs::create_dir_all(state.store.dirs().logs())
        .and_then(|()| std::fs::write(&path, &report))
        .ok()
        .map(|()| path.display().to_string());
    Check {
        steps,
        report,
        saved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_is_a_table_with_a_mark_per_step() {
        let steps = vec![
            Step {
                id: "screenshot",
                name: "A screenshot of every screen".into(),
                outcome: Outcome::Ok,
                detail: "5760 × 1080 pixels".into(),
                ms: 41,
            },
            Step {
                id: "recording",
                name: "A 3-second recording".into(),
                outcome: Outcome::Fail,
                detail: "ffmpeg | stopped\nearly".into(),
                ms: 3200,
            },
        ];
        let r = report(&steps, "2026-11-25 05:40 UTC");
        assert!(r.contains(&format!("CargoFrame {}", env!("CARGO_PKG_VERSION"))));
        assert!(r.contains("| ✓ | A screenshot of every screen | 5760 × 1080 pixels | 41 |"));
        // a sign or a line break in what was found cannot break the table
        assert!(r.contains("| ✗ | A 3-second recording | ffmpeg \\| stopped early | 3200 |"));
    }

    #[test]
    fn the_steps_that_need_no_screen_run_here() {
        let sys = system_step();
        assert_eq!(sys.outcome, Outcome::Ok);
        assert!(sys.detail.contains(" · "), "{}", sys.detail);
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(cargoframe_config::Dirs::under(dir.path()));
        std::fs::create_dir_all(store.dirs().config()).unwrap();
        let folder = folder_step(&store);
        assert_eq!(folder.outcome, Outcome::Ok, "{}", folder.detail);
        // nothing is left behind
        assert!(!store.dirs().config().join(".pc-check").exists());
        assert_eq!(sign_in_step().id, "sign_in");
    }
}
