//! GNOME: CargoFrame's shortcuts as custom shortcuts in GNOME's keyboard settings, the ones a
//! person adds under Settings, Keyboard, Custom Shortcuts. Each one CargoFrame makes sits under
//! a path that starts with `cargoframe-`; any other custom shortcut is left as it is.

use crate::Binding;
use cargoframe_config::shortcut;
use std::collections::BTreeMap;

pub const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
pub const CUSTOM: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const BASE: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";
const OURS: &str = "cargoframe-";

/// Reading and writing GNOME's settings.
pub trait Settings {
    fn get(&self, schema: &str, key: &str) -> Result<String, String>;
    fn set(&self, schema: &str, key: &str, value: &str) -> Result<(), String>;
    fn reset(&self, schema: &str, key: &str) -> Result<(), String>;
}

/// The `gsettings` program.
pub struct Gsettings;

impl Gsettings {
    fn run(args: &[&str]) -> Result<String, String> {
        let out = std::process::Command::new("gsettings")
            .args(args)
            .output()
            .map_err(|e| format!("gsettings could not start: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    /// Whether GNOME's custom shortcuts can be written here.
    pub fn available() -> bool {
        Self::run(&["list-keys", MEDIA_KEYS])
            .is_ok_and(|keys| keys.lines().any(|k| k.trim() == "custom-keybindings"))
    }
}

impl Settings for Gsettings {
    fn get(&self, schema: &str, key: &str) -> Result<String, String> {
        Self::run(&["get", schema, key])
    }
    fn set(&self, schema: &str, key: &str, value: &str) -> Result<(), String> {
        Self::run(&["set", schema, key, value]).map(|_| ())
    }
    fn reset(&self, schema: &str, key: &str) -> Result<(), String> {
        Self::run(&["reset", schema, key]).map(|_| ())
    }
}

/// A string written the way GLib reads it back: `'it\'s'`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// A list of strings as gsettings prints it: `['/a/', '/b/']` or `@as []`.
pub fn parse_list(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\'' && c != '"' {
            continue;
        }
        let end = c;
        let mut item = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        item.push(n);
                    }
                }
                c if c == end => break,
                c => item.push(c),
            }
        }
        items.push(item);
    }
    items
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        return "@as []".into();
    }
    format!(
        "[{}]",
        items
            .iter()
            .map(|i| quote(i))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The path of a trigger's custom shortcut.
pub fn path_for(trigger: &str) -> String {
    let slug: String = trigger
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("{BASE}{OURS}{slug}/")
}

fn ours(path: &str) -> bool {
    path.strip_prefix(BASE)
        .is_some_and(|rest| rest.starts_with(OURS))
}

/// Make GNOME's custom shortcuts match `wanted`: add or update CargoFrame's, remove the ones
/// it no longer has, keep everyone else's. Returns the triggers that could not be set.
pub fn sync(s: &dyn Settings, wanted: &[(Binding, String)]) -> BTreeMap<String, String> {
    let mut errors = BTreeMap::new();
    let fail_all = |why: String| -> BTreeMap<String, String> {
        wanted
            .iter()
            .map(|(b, _)| (b.id.clone(), why.clone()))
            .collect()
    };
    let current = match s.get(MEDIA_KEYS, "custom-keybindings") {
        Ok(t) => parse_list(&t),
        Err(e) => return fail_all(format!("GNOME's keyboard settings could not be read: {e}")),
    };
    let new_paths: Vec<String> = wanted.iter().map(|(b, _)| path_for(&b.id)).collect();

    // ours that are no longer wanted: their keys are reset so nothing is left behind
    for old in current.iter().filter(|p| ours(p) && !new_paths.contains(p)) {
        let schema = format!("{CUSTOM}:{old}");
        for key in ["name", "command", "binding"] {
            let _ = s.reset(&schema, key);
        }
    }
    for ((b, command), path) in wanted.iter().zip(&new_paths) {
        let schema = format!("{CUSTOM}:{path}");
        let set = s
            .set(&schema, "name", &quote(&format!("CargoFrame: {}", b.name)))
            .and_then(|_| s.set(&schema, "command", &quote(command)))
            .and_then(|_| {
                s.set(
                    &schema,
                    "binding",
                    &quote(&shortcut::for_gnome(&b.shortcut)),
                )
            });
        if let Err(e) = set {
            errors.insert(
                b.id.clone(),
                format!("GNOME did not take {}: {e}", b.shortcut),
            );
        }
    }
    let mut next: Vec<String> = current.iter().filter(|p| !ours(p)).cloned().collect();
    next.extend(
        new_paths
            .iter()
            .zip(wanted)
            .filter(|(_, (b, _))| !errors.contains_key(&b.id))
            .map(|(p, _)| p.clone()),
    );
    if next != current {
        if let Err(e) = s.set(MEDIA_KEYS, "custom-keybindings", &list(&next)) {
            return fail_all(format!(
                "GNOME's keyboard settings could not be changed: {e}"
            ));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake(RefCell<HashMap<(String, String), String>>);

    impl Settings for Fake {
        fn get(&self, schema: &str, key: &str) -> Result<String, String> {
            Ok(self
                .0
                .borrow()
                .get(&(schema.into(), key.into()))
                .cloned()
                .unwrap_or_else(|| "@as []".into()))
        }
        fn set(&self, schema: &str, key: &str, value: &str) -> Result<(), String> {
            self.0
                .borrow_mut()
                .insert((schema.into(), key.into()), value.into());
            Ok(())
        }
        fn reset(&self, schema: &str, key: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(&(schema.into(), key.into()));
            Ok(())
        }
    }

    fn b(id: &str, name: &str, keys: &str) -> (Binding, String) {
        (
            Binding {
                id: id.into(),
                name: name.into(),
                shortcut: keys.into(),
            },
            format!("/opt/CargoFrame.AppImage open --trigger {id}"),
        )
    }

    #[test]
    fn lists_are_read_and_written_as_gsettings_does() {
        assert_eq!(parse_list("@as []"), Vec::<String>::new());
        assert_eq!(
            parse_list("['/a/', '/b\\'s/']"),
            vec!["/a/".to_string(), "/b's/".to_string()]
        );
        assert_eq!(list(&[]), "@as []");
        assert_eq!(list(&["/a/".into()]), "['/a/']");
        assert_eq!(quote(r"it's a \ test"), r"'it\'s a \\ test'");
    }

    #[test]
    fn shortcuts_are_added_beside_the_persons_own() {
        let s = Fake::default();
        s.set(
            MEDIA_KEYS,
            "custom-keybindings",
            &list(&[format!("{BASE}custom0/")]),
        )
        .unwrap();
        let errors = sync(&s, &[b("bug", "Bug report", "Ctrl+Shift+F9")]);
        assert!(errors.is_empty());
        let paths = parse_list(&s.get(MEDIA_KEYS, "custom-keybindings").unwrap());
        assert_eq!(
            paths,
            vec![format!("{BASE}custom0/"), format!("{BASE}cargoframe-bug/")]
        );
        let schema = format!("{CUSTOM}:{BASE}cargoframe-bug/");
        assert_eq!(s.get(&schema, "binding").unwrap(), "'<Control><Shift>F9'");
        assert_eq!(s.get(&schema, "name").unwrap(), "'CargoFrame: Bug report'");
        assert_eq!(
            s.get(&schema, "command").unwrap(),
            "'/opt/CargoFrame.AppImage open --trigger bug'"
        );
    }

    #[test]
    fn a_removed_trigger_takes_its_shortcut_with_it_and_nothing_else() {
        let s = Fake::default();
        sync(
            &s,
            &[
                b("bug", "Bug", "Ctrl+Shift+F9"),
                b("chat", "Chat", "Ctrl+Shift+F10"),
            ],
        );
        let mut all = parse_list(&s.get(MEDIA_KEYS, "custom-keybindings").unwrap());
        all.insert(0, format!("{BASE}custom3/"));
        s.set(MEDIA_KEYS, "custom-keybindings", &list(&all))
            .unwrap();

        sync(&s, &[b("chat", "Chat", "Ctrl+Shift+F10")]);
        let paths = parse_list(&s.get(MEDIA_KEYS, "custom-keybindings").unwrap());
        assert_eq!(
            paths,
            vec![format!("{BASE}custom3/"), format!("{BASE}cargoframe-chat/")]
        );
        let gone = format!("{CUSTOM}:{BASE}cargoframe-bug/");
        assert_eq!(
            s.get(&gone, "binding").unwrap(),
            "@as []",
            "the old keys are reset"
        );
    }

    #[test]
    fn odd_trigger_ids_make_valid_paths() {
        assert_eq!(
            path_for("Bug_Report 2"),
            format!("{BASE}cargoframe-bug-report-2/")
        );
    }
}
