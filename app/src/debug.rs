//! Debug mode (System page, off by default): CargoFrame reporting a problem in itself.
//!
//! While it is on, each CargoFrame window tells the app what it shows when something changes
//! (`ui/src/lib/debugView.ts`): the page, the tab, the section and the control in focus, the
//! guide showing, the layout and the block chosen. A capture of a CargoFrame window then has
//! `{cargoframe.*}` values to write in a title or a description, and its first Files block gets
//! what a developer needs to see the problem again: the layout on screen as it is saved, the data
//! rules, the settings and the end of the log. Nothing is kept while the mode is off, and a
//! capture of any other program is not touched.

use crate::session::Session;
use cargoframe_bundle::Workspace;
use cargoframe_config::{General, Kind, Store};
use cargoframe_i18n::t;
use cargoframe_layout::Layout;
use cargoframe_rules::Rule;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// What a CargoFrame window shows, as it reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    /// `settings` or `capture`.
    pub window: String,
    pub page: String,
    pub tab: String,
    pub section: String,
    pub focus: String,
    pub guide: String,
    pub layout_id: String,
    pub layout: String,
    pub block: String,
}

/// The last window to report: the one in front when a shortcut is pressed.
static LAST: Mutex<Option<View>> = Mutex::new(None);

/// The values a capture of CargoFrame gets, in the order Data parsing lists them.
pub const NAMES: [&str; 11] = [
    "cargoframe.where",
    "cargoframe.window",
    "cargoframe.page",
    "cargoframe.tab",
    "cargoframe.section",
    "cargoframe.focus",
    "cargoframe.guide",
    "cargoframe.layout",
    "cargoframe.block",
    "cargoframe.rules",
    "cargoframe.version",
];

/// What each value is, for Data parsing and the builder's list of values.
pub fn describe(name: &str) -> &'static str {
    match name {
        "cargoframe.where" => {
            t("Debug mode: where it was in CargoFrame (window - page - tab - section)")
        }
        "cargoframe.window" => t("Debug mode: the CargoFrame window captured"),
        "cargoframe.page" => t("Debug mode: the page shown"),
        "cargoframe.tab" => t("Debug mode: the tabs open"),
        "cargoframe.section" => t("Debug mode: the section of the control in focus"),
        "cargoframe.focus" => t("Debug mode: the control in focus"),
        "cargoframe.guide" => t("Debug mode: the guide showing, and its step"),
        "cargoframe.layout" => t("Debug mode: the layout on screen"),
        "cargoframe.block" => t("Debug mode: the block chosen or in focus"),
        "cargoframe.rules" => t("Debug mode: the data rules"),
        "cargoframe.version" => t("Debug mode: CargoFrame's version"),
        _ => "",
    }
}

pub fn report(view: View) {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(view);
}

fn last() -> Option<View> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn is_on(store: &Store) -> bool {
    store.load_or_default::<General>().is_ok_and(|g| g.debug)
}

/// Whether `exe`, the program in focus, is this CargoFrame.
pub fn is_me(exe: Option<&str>) -> bool {
    let (Some(exe), Ok(me)) = (exe, std::env::current_exe()) else {
        return false;
    };
    same_program(Path::new(exe), &me)
}

fn same_program(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let s = p.display().to_string().replace('\\', "/");
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

/// What was on screen, when this capture is of CargoFrame itself and debug mode is on.
pub fn view_for(store: &Store, exe: Option<&str>) -> Option<View> {
    if !is_me(exe) || !is_on(store) {
        return None;
    }
    Some(last().unwrap_or_default())
}

/// The window's name as its title bar says it.
fn window_name(kind: &str) -> &'static str {
    match kind {
        "capture" => "CargoFrame window",
        _ => "CargoFrame settings",
    }
}

/// The values of a capture of CargoFrame. `where` is the path to what was on screen, as the
/// issues are titled: "CargoFrame settings - Layouts - Fill fields - Fill a field".
pub fn values(view: &View, rules: &[String]) -> Vec<(String, String)> {
    let window = window_name(&view.window).to_string();
    let mut path = vec![window.clone()];
    for part in [&view.page, &view.tab, &view.section] {
        let part = part.trim();
        if !part.is_empty() && !path.iter().any(|p| p.eq_ignore_ascii_case(part)) {
            path.push(part.to_string());
        }
    }
    let pairs = [
        ("cargoframe.where", path.join(" - ")),
        ("cargoframe.window", window),
        ("cargoframe.page", view.page.clone()),
        ("cargoframe.tab", view.tab.clone()),
        ("cargoframe.section", view.section.clone()),
        ("cargoframe.focus", view.focus.clone()),
        ("cargoframe.guide", view.guide.clone()),
        ("cargoframe.layout", view.layout.clone()),
        ("cargoframe.block", view.block.clone()),
        ("cargoframe.rules", rules.join(", ")),
        ("cargoframe.version", env!("CARGO_PKG_VERSION").to_string()),
    ];
    // every one has a value, so a text that names them reads whole: a dash for what was not
    // on screen (no guide)
    pairs
        .into_iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                if v.trim().is_empty() {
                    "—".to_string()
                } else {
                    v
                },
            )
        })
        .collect()
}

/// The names of the data rules, for `{cargoframe.rules}`.
pub fn rule_names(store: &Store) -> Vec<String> {
    store
        .load_all::<Rule>(Kind::Rules)
        .map(|l| {
            l.items
                .into_iter()
                .map(|(id, r)| if r.name.trim().is_empty() { id } else { r.name })
                .collect()
        })
        .unwrap_or_default()
}

/// The end of the log kept, enough to see what led to the problem.
const LOG_TAIL: u64 = 256 * 1024;

/// The files a capture of CargoFrame gets: name and content.
pub fn files(store: &Store, view: &View) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let json = |v: &serde_json::Value| serde_json::to_vec_pretty(v).unwrap_or_default();
    out.push((
        "cargoframe-screen.json".to_string(),
        json(&serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "on_screen": view,
        })),
    ));
    if !view.layout_id.is_empty() {
        if let Ok(Some(layout)) = store.load_item::<Layout>(Kind::Layouts, &view.layout_id) {
            out.push((
                format!("cargoframe-layout-{}.json", safe(&view.layout_id)),
                json(&serde_json::to_value(layout).unwrap_or_default()),
            ));
        }
    }
    if let Ok(rules) = store.load_all::<Rule>(Kind::Rules) {
        let map: serde_json::Map<String, serde_json::Value> = rules
            .items
            .into_iter()
            .map(|(id, r)| (id, serde_json::to_value(r).unwrap_or_default()))
            .collect();
        out.push(("cargoframe-rules.json".to_string(), json(&map.into())));
    }
    if let Ok(general) = store.load_or_default::<General>() {
        out.push((
            "cargoframe-settings.json".to_string(),
            json(&serde_json::to_value(general).unwrap_or_default()),
        ));
    }
    if let Some(tail) = tail_of(&store.dirs().logs().join("cargoframe.log"), LOG_TAIL) {
        out.push(("cargoframe.log".to_string(), tail));
    }
    out
}

fn safe(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn tail_of(path: &Path, max: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(max))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Add the files to the first Files block of a capture of CargoFrame, ticked.
pub fn attach(store: &Store, workspace: &Workspace, session: &mut Session) {
    let exe = session.view.app.as_ref().and_then(|a| a.exe_path.clone());
    let Some(view) = view_for(store, exe.as_deref()) else {
        return;
    };
    let Some(block) = session
        .view
        .layout
        .as_ref()
        .and_then(|l| {
            l.blocks
                .iter()
                .find(|b| matches!(b.kind, cargoframe_layout::BlockKind::Files(_)))
        })
        .map(|b| b.id.clone())
    else {
        return;
    };
    let dir = std::env::temp_dir().join(format!(
        "cargoframe-debug-{}-{}",
        std::process::id(),
        session.view.session_id
    ));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, bytes) in files(store, &view) {
        let path = dir.join(&name);
        if std::fs::write(&path, bytes).is_ok() {
            if let Err(e) = session.add_own_file(workspace, &block, &path) {
                tracing::warn!("debug mode: {name} could not be added: {e}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_values_say_where_on_screen_as_issues_are_titled() {
        let view = View {
            window: "settings".into(),
            page: "Layouts".into(),
            tab: "Fill fields".into(),
            section: "Fill a field".into(),
            focus: "Service".into(),
            ..View::default()
        };
        let v: std::collections::BTreeMap<_, _> =
            values(&view, &["Game log".into(), "Version".into()])
                .into_iter()
                .collect();
        assert_eq!(
            v["cargoframe.where"],
            "CargoFrame settings - Layouts - Fill fields - Fill a field"
        );
        assert_eq!(v["cargoframe.focus"], "Service");
        assert_eq!(v["cargoframe.rules"], "Game log, Version");
        assert_eq!(v["cargoframe.version"], env!("CARGO_PKG_VERSION"));
        // nothing reported: a dash, so a description that names it reads whole
        assert_eq!(v["cargoframe.guide"], "—");
        assert_eq!(v.len(), NAMES.len());
        for name in v.keys() {
            assert!(NAMES.contains(&name.as_str()), "{name}");
        }
    }

    #[test]
    fn a_page_named_like_its_tab_is_said_once() {
        let view = View {
            window: "capture".into(),
            page: "Bug report".into(),
            tab: "bug report".into(),
            ..View::default()
        };
        let v: std::collections::BTreeMap<_, _> = values(&view, &[]).into_iter().collect();
        assert_eq!(v["cargoframe.where"], "CargoFrame window - Bug report");
    }

    #[test]
    fn only_this_program_is_cargoframe() {
        let me = std::env::current_exe().unwrap();
        assert!(is_me(Some(&me.display().to_string())));
        assert!(!is_me(Some("/usr/bin/firefox")));
        assert!(!is_me(None));
    }

    #[test]
    fn the_log_kept_is_its_end() {
        let dir = std::env::temp_dir().join(format!("cf-debug-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.log");
        std::fs::write(&p, "0123456789").unwrap();
        assert_eq!(tail_of(&p, 4).unwrap(), b"6789");
        assert_eq!(tail_of(&p, 100).unwrap(), b"0123456789");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
