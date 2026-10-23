//! The last timings of the things the performance plan puts a budget on, shown in Settings,
//! General so a slow machine can say where the time went.
//!
//! A *flow* starts with `begin` (the shortcut was pressed, or a window was asked for) and each
//! *metric* is the time from that start to a later moment: the capture being ready, the window
//! being shown, the window being usable (the page reports that itself once it has painted).

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// One line of the timings list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimingView {
    pub id: String,
    pub label: String,
    pub ms: u64,
    /// The plan's budget for this measurement, when it has one.
    pub budget_ms: Option<u64>,
    /// "cold" or "warm" for the windows: a warm window was already open or kept.
    pub note: Option<String>,
}

/// (id, label) in the order they are listed.
const METRICS: [(&str, &str); 5] = [
    ("ready", "Shortcut to capture ready"),
    ("window", "Shortcut to window shown"),
    ("paint", "Shortcut to usable window"),
    ("settings_paint", "Settings window open"),
    ("share", "Last share, start to finish"),
];

/// The plan's budgets, in milliseconds.
pub fn budget_ms(id: &str, note: Option<&str>) -> Option<u64> {
    match (id, note) {
        ("ready", _) => Some(150),
        ("paint", Some("warm")) => Some(250),
        ("paint", _) => Some(700),
        ("settings_paint", _) => Some(500),
        _ => None,
    }
}

#[derive(Default)]
struct Inner {
    started: HashMap<String, Instant>,
    notes: HashMap<String, String>,
    last: HashMap<String, (u64, Option<String>)>,
}

#[derive(Default)]
pub struct Timings {
    inner: Mutex<Inner>,
}

impl Timings {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Start (or restart) a flow.
    pub fn begin(&self, flow: &str) {
        let mut i = self.lock();
        i.started.insert(flow.to_string(), Instant::now());
        i.notes.remove(flow);
    }

    /// Say whether the flow's window was already there ("warm") or had to be built ("cold").
    pub fn note(&self, flow: &str, note: &str) {
        self.lock().notes.insert(flow.to_string(), note.to_string());
    }

    /// Record how long the flow has been running, and keep it running.
    pub fn mark(&self, flow: &str, metric: &str) -> Option<u64> {
        let mut i = self.lock();
        let ms = i.started.get(flow)?.elapsed().as_millis() as u64;
        let note = i.notes.get(flow).cloned();
        i.last.insert(metric.to_string(), (ms, note));
        Some(ms)
    }

    /// Record how long the flow took and end it, so a second report (the page redrawing) does
    /// not count time nobody was waiting.
    pub fn finish(&self, flow: &str, metric: &str) -> Option<u64> {
        let ms = self.mark(flow, metric)?;
        self.lock().started.remove(flow);
        Some(ms)
    }

    pub fn record(&self, metric: &str, ms: u64) {
        self.lock().last.insert(metric.to_string(), (ms, None));
    }

    pub fn snapshot(&self) -> Vec<TimingView> {
        let i = self.lock();
        METRICS
            .iter()
            .filter_map(|(id, label)| {
                let (ms, note) = i.last.get(*id)?.clone();
                Some(TimingView {
                    id: (*id).to_string(),
                    label: (*label).to_string(),
                    ms,
                    budget_ms: budget_ms(id, note.as_deref()),
                    note,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn a_flow_reports_the_time_since_it_began() {
        let t = Timings::new();
        t.begin("capture");
        sleep(Duration::from_millis(15));
        let ms = t.mark("capture", "ready").unwrap();
        assert!((15..2000).contains(&ms), "{ms}");
        assert_eq!(t.snapshot()[0].id, "ready");
    }

    #[test]
    fn finishing_ends_the_flow_so_a_redraw_is_not_counted() {
        let t = Timings::new();
        t.begin("capture");
        assert!(t.finish("capture", "paint").is_some());
        assert!(t.finish("capture", "paint").is_none());
        // The first answer stays.
        assert_eq!(t.snapshot().len(), 1);
    }

    #[test]
    fn nothing_is_recorded_for_a_flow_that_never_began() {
        let t = Timings::new();
        assert!(t.mark("settings", "settings_paint").is_none());
        assert!(t.snapshot().is_empty());
    }

    #[test]
    fn budgets_follow_the_plan_and_warm_windows_get_the_tighter_one() {
        let t = Timings::new();
        t.begin("capture");
        t.note("capture", "warm");
        t.finish("capture", "paint").unwrap();
        t.begin("capture");
        t.note("capture", "cold");
        t.mark("capture", "ready").unwrap();
        let by_id = |id: &str| t.snapshot().into_iter().find(|v| v.id == id).unwrap();
        assert_eq!(by_id("paint").budget_ms, Some(250));
        assert_eq!(by_id("paint").note.as_deref(), Some("warm"));
        assert_eq!(by_id("ready").budget_ms, Some(150));
        assert_eq!(budget_ms("paint", Some("cold")), Some(700));
        assert_eq!(budget_ms("settings_paint", None), Some(500));
        assert_eq!(budget_ms("share", None), None);
    }

    #[test]
    fn the_list_has_a_fixed_order() {
        let t = Timings::new();
        t.record("share", 900);
        t.begin("settings");
        t.finish("settings", "settings_paint").unwrap();
        let ids: Vec<_> = t.snapshot().into_iter().map(|v| v.id).collect();
        assert_eq!(ids, ["settings_paint", "share"]);
    }
}
