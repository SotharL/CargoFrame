//! Reading and writing the configuration files.

use crate::error::ConfigError;
use crate::paths::Dirs;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A configuration file type.
pub trait Document: Serialize + DeserializeOwned {
    /// Schema version this build writes and understands.
    const SCHEMA: u32;
    /// File stem for singleton documents (`general` -> `general.json`).
    const NAME: &'static str;

    /// Upgrade `value` from schema `from` to `from + 1`. Called once per step, oldest first.
    /// The default does nothing, which is right until a schema actually changes.
    fn migrate(from: u32, value: Value) -> Result<Value, ConfigError> {
        let _ = from;
        Ok(value)
    }

    /// The document as the organisation's policy leaves it (`general.json`: updates forced on
    /// or off, debug mode off). The default changes nothing.
    fn under(self, policy: &crate::Policy) -> Self {
        let _ = policy;
        self
    }
}

/// The folders that hold one file per item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Triggers,
    Connections,
    Rules,
    InfoTypes,
    Layouts,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Triggers,
        Kind::Connections,
        Kind::Rules,
        Kind::InfoTypes,
        Kind::Layouts,
    ];

    pub fn dir_name(self) -> &'static str {
        match self {
            Kind::Triggers => "triggers",
            Kind::Connections => "connections",
            Kind::Rules => "rules",
            Kind::InfoTypes => "infotypes",
            Kind::Layouts => "layouts",
        }
    }
}

/// Ids are file names: lowercase letters, digits, `-` and `_`; 1 to 64 characters; they must
/// start with a letter or digit. This also rules out `..`, separators and device names.
pub fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let ok_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    ok_first
        && id.len() <= 64
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Everything of one kind that could be read, plus the files that could not. One damaged file
/// never hides the others.
#[derive(Debug)]
pub struct Loaded<T> {
    pub items: Vec<(String, T)>,
    pub errors: Vec<ConfigError>,
}

impl<T> Default for Loaded<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Store {
    dirs: Dirs,
}

impl Store {
    pub fn new(dirs: Dirs) -> Self {
        Self { dirs }
    }

    pub fn dirs(&self) -> &Dirs {
        &self.dirs
    }

    // ----- singleton documents (general.json, safe.json) -----

    /// The document, or its default when the file does not exist yet, as the organisation's
    /// policy leaves it.
    pub fn load_or_default<T: Document + Default>(&self) -> Result<T, ConfigError> {
        let doc = read_doc::<T>(&self.singleton_path::<T>())?.unwrap_or_default();
        Ok(doc.under(&self.policy()))
    }

    /// The organisation's `policy.json`, or no policy: a file that is missing or cannot be read
    /// decides nothing (and never stops CargoFrame from starting).
    pub fn policy(&self) -> crate::Policy {
        self.dirs
            .managed()
            .and_then(|m| {
                read_doc_readonly::<crate::Policy>(&m.join("policy.json"))
                    .ok()
                    .flatten()
            })
            .unwrap_or_default()
    }

    pub fn save<T: Document>(&self, doc: &T) -> Result<(), ConfigError> {
        write_doc(&self.singleton_path::<T>(), doc)
    }

    fn singleton_path<T: Document>(&self) -> PathBuf {
        self.dirs.config().join(format!("{}.json", T::NAME))
    }

    // ----- one file per item -----

    /// An item: the organisation's when it has one with this id, else the person's own.
    pub fn load_item<T: Document>(&self, kind: Kind, id: &str) -> Result<Option<T>, ConfigError> {
        match self.managed_path(kind, id)? {
            Some(path) => read_doc_readonly(&path),
            None => read_doc(&self.item_path(kind, id)?),
        }
    }

    /// Is this item the organisation's (read from its managed folder, never written)?
    pub fn is_managed(&self, kind: Kind, id: &str) -> bool {
        matches!(self.managed_path(kind, id), Ok(Some(_)))
    }

    pub fn save_item<T: Document>(&self, kind: Kind, id: &str, doc: &T) -> Result<(), ConfigError> {
        if self.is_managed(kind, id) {
            return Err(ConfigError::Managed(id.to_string()));
        }
        write_doc(&self.item_path(kind, id)?, doc)
    }

    /// True when a file was removed.
    pub fn delete_item(&self, kind: Kind, id: &str) -> Result<bool, ConfigError> {
        if self.is_managed(kind, id) {
            return Err(ConfigError::Managed(id.to_string()));
        }
        let path = self.item_path(kind, id)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(ConfigError::io(path, e)),
        }
    }

    /// Every readable item of a kind, sorted by id.
    pub fn load_all<T: Document>(&self, kind: Kind) -> Result<Loaded<T>, ConfigError> {
        let mut out = Loaded::default();
        for id in self.list_ids(kind)? {
            match self.load_item::<T>(kind, &id) {
                Ok(Some(doc)) => out.items.push((id, doc)),
                Ok(None) => {}
                Err(e) => out.errors.push(e),
            }
        }
        Ok(out)
    }

    /// Ids of the items of a kind, the organisation's and the person's, sorted and each once.
    /// Files that are not `<valid id>.json` are ignored.
    pub fn list_ids(&self, kind: Kind) -> Result<Vec<String>, ConfigError> {
        let mut ids = ids_in(&self.dirs.config().join(kind.dir_name()))?;
        if let Some(m) = self.dirs.managed() {
            // an organisation's folder that cannot be read hides nothing of the person's own
            ids.extend(ids_in(&m.join(kind.dir_name())).unwrap_or_default());
        }
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    /// The organisation's file for an item, when it has one.
    fn managed_path(&self, kind: Kind, id: &str) -> Result<Option<PathBuf>, ConfigError> {
        if !valid_id(id) {
            return Err(ConfigError::InvalidId(id.to_string()));
        }
        Ok(self
            .dirs
            .managed()
            .map(|m| m.join(kind.dir_name()).join(format!("{id}.json")))
            .filter(|p| p.is_file()))
    }

    fn item_path(&self, kind: Kind, id: &str) -> Result<PathBuf, ConfigError> {
        if !valid_id(id) {
            return Err(ConfigError::InvalidId(id.to_string()));
        }
        Ok(self
            .dirs
            .config()
            .join(kind.dir_name())
            .join(format!("{id}.json")))
    }
}

/// The `<valid id>.json` files of a folder, as ids. A folder that is not there holds none.
fn ids_in(dir: &Path) -> Result<Vec<String>, ConfigError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(ConfigError::io(dir, e)),
    };
    Ok(entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            valid_id(id).then(|| id.to_string())
        })
        .collect())
}

/// Read and parse a file CargoFrame must not write (the organisation's): an older schema is
/// migrated in memory only, with no backup and no write back.
fn read_doc_readonly<T: Document>(path: &Path) -> Result<Option<T>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ConfigError::io(path, e)),
    };
    let mut value: Value = serde_json::from_str(&text).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let found = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(1);
    if found > T::SCHEMA {
        return Err(ConfigError::Newer {
            path: path.to_path_buf(),
            found,
            supported: T::SCHEMA,
        });
    }
    for from in found..T::SCHEMA {
        value = T::migrate(from, value)?;
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
}

/// Read, migrate if needed, and parse. `None` when the file does not exist.
fn read_doc<T: Document>(path: &Path) -> Result<Option<T>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ConfigError::io(path, e)),
    };
    let mut value: Value = serde_json::from_str(&text).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let found = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(1);
    if found > T::SCHEMA {
        return Err(ConfigError::Newer {
            path: path.to_path_buf(),
            found,
            supported: T::SCHEMA,
        });
    }

    let migrated = found < T::SCHEMA;
    if migrated {
        // Keep the original before touching anything.
        let backup = PathBuf::from(format!("{}.bak-v{found}", path.display()));
        if !backup.exists() {
            fs::write(&backup, &text).map_err(|e| ConfigError::io(&backup, e))?;
        }
        for from in found..T::SCHEMA {
            value = T::migrate(from, value)?;
        }
        if let Some(obj) = value.as_object_mut() {
            obj.insert("schema_version".into(), Value::from(T::SCHEMA));
        }
    }

    let doc: T = serde_json::from_value(value).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    if migrated {
        write_doc(path, &doc)?;
    }
    Ok(Some(doc))
}

fn write_doc<T: Document>(path: &Path, doc: &T) -> Result<(), ConfigError> {
    let mut value = serde_json::to_value(doc).map_err(|e| ConfigError::Invalid(e.to_string()))?;
    match value.as_object_mut() {
        Some(obj) => {
            obj.insert("schema_version".into(), Value::from(T::SCHEMA));
        }
        None => {
            return Err(ConfigError::Invalid(format!(
                "{} must serialize to a JSON object",
                T::NAME
            )))
        }
    }
    let mut text =
        serde_json::to_string_pretty(&value).map_err(|e| ConfigError::Invalid(e.to_string()))?;
    text.push('\n');
    write_atomic(path, text.as_bytes())
}

/// Write to a temporary file next to the target, flush it to disk, then rename over the target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    let parent = path
        .parent()
        .ok_or_else(|| ConfigError::Invalid(format!("{} has no folder", path.display())))?;
    fs::create_dir_all(parent).map_err(|e| ConfigError::io(parent, e))?;
    let tmp = path.with_extension(format!("json.tmp{}", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(ConfigError::io(path, e));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{General, MemoryMode, Safe, Theme};
    use serde::Deserialize;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::new(Dirs::under(dir.path()));
        (dir, s)
    }

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Widget {
        schema_version: u32,
        name: String,
    }

    impl Document for Widget {
        const SCHEMA: u32 = 2;
        const NAME: &'static str = "widget";
        fn migrate(from: u32, mut value: Value) -> Result<Value, ConfigError> {
            if from == 1 {
                // v1 called the field "title".
                if let Some(obj) = value.as_object_mut() {
                    if let Some(t) = obj.remove("title") {
                        obj.insert("name".into(), t);
                    }
                }
            }
            Ok(value)
        }
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_d, s) = store();
        let g: General = s.load_or_default().unwrap();
        assert_eq!(g, General::default());
        assert_eq!(g.theme, Theme::System);
        assert_eq!(g.memory_mode, MemoryMode::Balanced);
        assert!(!g.check_updates);
        // every notice is on until the person switches it off
        assert!(g.notifications.routine && g.notifications.problems);
    }

    #[test]
    fn a_general_file_from_before_notifications_keeps_them_on() {
        let (d, s) = store();
        fs::create_dir_all(d.path().join("config")).unwrap();
        fs::write(
            d.path().join("config/general.json"),
            "{\"schema_version\": 1, \"theme\": \"dark\"}",
        )
        .unwrap();
        let g: General = s.load_or_default().unwrap();
        assert_eq!(g.theme, Theme::Dark);
        assert!(g.notifications.routine && g.notifications.problems);
        // and a choice is kept
        let off = General {
            notifications: crate::Notifications {
                routine: false,
                problems: true,
            },
            ..g
        };
        s.save(&off).unwrap();
        let back: General = s.load_or_default().unwrap();
        assert!(!back.notifications.routine && back.notifications.problems);
    }

    #[test]
    fn save_then_load_round_trips_and_is_readable_json() {
        let (d, s) = store();
        let g = General {
            theme: Theme::Dark,
            memory_mode: MemoryMode::LowMemory,
            ..General::default()
        };
        s.save(&g).unwrap();
        assert_eq!(s.load_or_default::<General>().unwrap(), g);

        let text = fs::read_to_string(d.path().join("config/general.json")).unwrap();
        assert!(text.contains("\"schema_version\": 1"));
        assert!(text.contains("\"theme\": \"dark\""));
        assert!(text.contains("\"memory_mode\": \"low_memory\""));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn video_settings_start_safe_and_keep_what_is_missing_at_its_default() {
        use crate::models::{Video, VideoCapture, VideoEncoder};
        let (d, s) = store();
        let fresh = s.load_or_default::<Video>().unwrap();
        assert_eq!(fresh, Video::default());
        assert_eq!(
            (fresh.capture, fresh.encoder),
            (VideoCapture::Auto, VideoEncoder::Auto)
        );
        assert_eq!(
            (fresh.fps, fresh.window_secs, fresh.max_height),
            (30, 15, 0)
        );

        fs::create_dir_all(d.path().join("config")).unwrap();
        fs::write(
            d.path().join("config/video.json"),
            r#"{ "schema_version": 1, "fps": 15, "encoder": "media_foundation", "someday": true }"#,
        )
        .unwrap();
        let v = s.load_or_default::<Video>().unwrap();
        assert_eq!(v.fps, 15);
        assert_eq!(v.encoder, VideoEncoder::MediaFoundation);
        assert_eq!(v.window_secs, 15, "missing fields keep their default");
        assert_eq!(v.max_height, 0);

        // an update never changes what a person set: the file's values stay, whatever the defaults
        fs::write(
            d.path().join("config/video.json"),
            r#"{ "schema_version": 1, "fps": 30, "window_secs": 10, "max_height": 1080 }"#,
        )
        .unwrap();
        let kept = s.load_or_default::<Video>().unwrap();
        assert_eq!(
            (kept.fps, kept.window_secs, kept.max_height),
            (30, 10, 1080)
        );
    }

    #[test]
    fn writes_leave_no_temporary_files() {
        let (d, s) = store();
        s.save(&Safe::default()).unwrap();
        s.save(&Safe::default()).unwrap();
        let names: Vec<String> = fs::read_dir(d.path().join("config"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec!["safe.json"]);
    }

    #[test]
    fn unknown_fields_and_missing_fields_are_tolerated() {
        let (d, s) = store();
        fs::create_dir_all(d.path().join("config")).unwrap();
        fs::write(
            d.path().join("config/general.json"),
            r#"{"schema_version":1,"theme":"light","from_the_future":true}"#,
        )
        .unwrap();
        let g: General = s.load_or_default().unwrap();
        assert_eq!(g.theme, Theme::Light);
        assert_eq!(g.memory_mode, MemoryMode::Balanced);
    }

    #[test]
    fn newer_schema_is_refused_and_left_alone() {
        let (d, s) = store();
        fs::create_dir_all(d.path().join("config")).unwrap();
        let path = d.path().join("config/general.json");
        fs::write(&path, r#"{"schema_version":99,"theme":"dark"}"#).unwrap();
        let err = s.load_or_default::<General>().unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Newer {
                found: 99,
                supported: 1,
                ..
            }
        ));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"schema_version":99,"theme":"dark"}"#
        );
    }

    #[test]
    fn corrupt_json_is_an_error_not_a_reset() {
        let (d, s) = store();
        fs::create_dir_all(d.path().join("config")).unwrap();
        let path = d.path().join("config/general.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(matches!(
            s.load_or_default::<General>(),
            Err(ConfigError::Parse { .. })
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn older_schema_is_migrated_with_a_backup() {
        let (d, s) = store();
        let dir = d.path().join("config/rules");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("w1.json");
        let original = r#"{"schema_version":1,"title":"hello"}"#;
        fs::write(&path, original).unwrap();

        let w: Widget = s.load_item(Kind::Rules, "w1").unwrap().unwrap();
        assert_eq!(w.name, "hello");

        // Backup holds the original text; the file on disk is now current.
        assert_eq!(
            fs::read_to_string(dir.join("w1.json.bak-v1")).unwrap(),
            original
        );
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["schema_version"], 2);
        assert_eq!(on_disk["name"], "hello");
        assert!(on_disk.get("title").is_none());

        // The backup is not listed as an item.
        assert_eq!(s.list_ids(Kind::Rules).unwrap(), vec!["w1"]);
    }

    #[test]
    fn items_save_list_and_delete() {
        let (_d, s) = store();
        for id in ["b-two", "a_one", "3rd"] {
            s.save_item(
                Kind::Rules,
                id,
                &Widget {
                    schema_version: 2,
                    name: id.into(),
                },
            )
            .unwrap();
        }
        assert_eq!(
            s.list_ids(Kind::Rules).unwrap(),
            vec!["3rd", "a_one", "b-two"]
        );
        assert_eq!(s.list_ids(Kind::Layouts).unwrap(), Vec::<String>::new());

        let w: Widget = s.load_item(Kind::Rules, "a_one").unwrap().unwrap();
        assert_eq!(w.name, "a_one");
        assert!(s
            .load_item::<Widget>(Kind::Rules, "nope")
            .unwrap()
            .is_none());

        assert!(s.delete_item(Kind::Rules, "a_one").unwrap());
        assert!(!s.delete_item(Kind::Rules, "a_one").unwrap());
        assert_eq!(s.list_ids(Kind::Rules).unwrap(), vec!["3rd", "b-two"]);
    }

    #[test]
    fn list_ignores_junk() {
        let (d, s) = store();
        let dir = d.path().join("config/triggers");
        fs::create_dir_all(dir.join("subfolder.json")).unwrap();
        for name in [
            "ok.json",
            "Upper.json",
            "notes.txt",
            "x.json.tmp1",
            ".hidden.json",
        ] {
            fs::write(dir.join(name), "{}").unwrap();
        }
        assert_eq!(s.list_ids(Kind::Triggers).unwrap(), vec!["ok"]);
    }

    #[test]
    fn ids_cannot_escape_the_folder() {
        let (_d, s) = store();
        let w = Widget {
            schema_version: 2,
            name: "x".into(),
        };
        for bad in [
            "",
            "../evil",
            "a/b",
            "a\\b",
            "A",
            "-lead",
            ".dot",
            "con:",
            &"x".repeat(65),
        ] {
            assert!(
                matches!(
                    s.save_item(Kind::Rules, bad, &w),
                    Err(ConfigError::InvalidId(_))
                ),
                "accepted {bad:?}"
            );
            assert!(s.load_item::<Widget>(Kind::Rules, bad).is_err());
            assert!(s.delete_item(Kind::Rules, bad).is_err());
        }
        assert!(valid_id("a"));
        assert!(valid_id("0-a_b"));
        assert!(valid_id(&"x".repeat(64)));
    }

    #[test]
    fn dirs_follow_the_documented_layout() {
        let d = Dirs::under(Path::new("/r"));
        assert_eq!(d.config(), Path::new("/r/config"));
        assert_eq!(d.data(), Path::new("/r/data"));
        assert_eq!(d.kept_zips(), Path::new("/r/data/kept"));
        assert_eq!(d.logs(), Path::new("/r/data/logs"));
    }

    /// A person's settings and an organisation's managed folder beside them.
    fn managed_store() -> (tempfile::TempDir, Store, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let managed = dir.path().join("managed");
        let s = Store::new(Dirs::under(dir.path()).with_managed(&managed));
        (dir, s, managed)
    }

    fn put(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn the_organisations_items_are_listed_with_the_persons_and_win_on_the_same_id() {
        let (_d, s, managed) = managed_store();
        let w = |name: &str| Widget {
            schema_version: 2,
            name: name.into(),
        };
        s.save_item(Kind::Layouts, "mine", &w("Mine")).unwrap();
        s.save_item(Kind::Layouts, "bug", &w("My bug")).unwrap();
        put(
            &managed.join("layouts/bug.json"),
            r#"{"schema_version":2,"name":"Company bug"}"#,
        );
        put(
            &managed.join("layouts/report.json"),
            r#"{"schema_version":2,"name":"Company report"}"#,
        );
        put(&managed.join("layouts/Not An Id.json"), "{}");
        assert_eq!(
            s.list_ids(Kind::Layouts).unwrap(),
            ["bug", "mine", "report"]
        );
        assert_eq!(
            s.load_item::<Widget>(Kind::Layouts, "bug")
                .unwrap()
                .unwrap()
                .name,
            "Company bug"
        );
        assert!(s.is_managed(Kind::Layouts, "bug") && s.is_managed(Kind::Layouts, "report"));
        assert!(!s.is_managed(Kind::Layouts, "mine"));
        // never changed or removed from here, and the person's own file is left as it was
        assert!(matches!(
            s.save_item(Kind::Layouts, "report", &w("x")),
            Err(ConfigError::Managed(_))
        ));
        assert!(matches!(
            s.delete_item(Kind::Layouts, "bug"),
            Err(ConfigError::Managed(_))
        ));
        assert!(managed.join("layouts/report.json").exists());
        assert!(s.save_item(Kind::Layouts, "mine", &w("Mine 2")).is_ok());
    }

    #[test]
    fn an_older_managed_file_is_read_without_writing_anything() {
        let (_d, s, managed) = managed_store();
        let path = managed.join("layouts/old.json");
        put(&path, r#"{"schema_version":1,"name":"Old"}"#);
        let before = fs::read_to_string(&path).unwrap();
        let w = s
            .load_item::<Widget>(Kind::Layouts, "old")
            .unwrap()
            .unwrap();
        assert_eq!(w.name, "Old");
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        assert!(!managed.join("layouts/old.json.bak-v1").exists());
    }

    #[test]
    fn the_policy_decides_updates_and_debug_mode_whatever_was_saved() {
        let (_d, s, managed) = managed_store();
        s.save(&General {
            check_updates: true,
            debug: true,
            ..General::default()
        })
        .unwrap();
        assert!(s.policy().is_empty());
        put(
            &managed.join("policy.json"),
            r#"{"schema_version":1,"check_updates":false,"debug_mode":false}"#,
        );
        let g = s.load_or_default::<General>().unwrap();
        assert!(!g.check_updates && !g.debug);
        // a policy that cannot be read decides nothing
        put(&managed.join("policy.json"), "{ not json");
        assert!(s.policy().is_empty());
        assert!(s.load_or_default::<General>().unwrap().check_updates);
    }

    #[test]
    fn without_a_managed_folder_nothing_changes() {
        let (_d, s) = store();
        assert!(s.policy().is_empty());
        assert!(!s.is_managed(Kind::Layouts, "bug"));
    }
}

#[cfg(test)]
mod load_all_tests {
    use super::*;
    use crate::models::Trigger;

    #[test]
    fn one_damaged_file_does_not_hide_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::new(Dirs::under(dir.path()));
        s.save_item(
            Kind::Triggers,
            "good",
            &Trigger {
                name: "Good".into(),
                ..Trigger::default()
            },
        )
        .unwrap();
        let bad = s.dirs().config().join("triggers").join("bad.json");
        std::fs::write(&bad, "{ not json").unwrap();
        let loaded = s.load_all::<Trigger>(Kind::Triggers).unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].0, "good");
        assert_eq!(loaded.errors.len(), 1);
    }

    #[test]
    fn a_trigger_file_from_before_video_is_a_screenshot_trigger() {
        use crate::models::Media;
        let dir = tempfile::tempdir().unwrap();
        let s = Store::new(Dirs::under(dir.path()));
        let folder = s.dirs().config().join("triggers");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("old.json"),
            r#"{"schema_version": 1, "name": "Old", "shortcut": "F9", "capture": "display"}"#,
        )
        .unwrap();
        let old = s
            .load_item::<Trigger>(Kind::Triggers, "old")
            .unwrap()
            .unwrap();
        assert_eq!(old.media, Media::Screenshot);

        s.save_item(
            Kind::Triggers,
            "clip",
            &Trigger {
                name: "Clip".into(),
                media: Media::Video,
                ..Trigger::default()
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(folder.join("clip.json")).unwrap();
        assert!(text.contains("\"media\": \"video\""), "{text}");
        let back = s
            .load_item::<Trigger>(Kind::Triggers, "clip")
            .unwrap()
            .unwrap();
        assert_eq!(back.media, Media::Video);
    }

    #[test]
    fn layouts_and_chats_use_the_same_machinery() {
        use cargoframe_layout::Layout;
        let dir = tempfile::tempdir().unwrap();
        let s = Store::new(Dirs::under(dir.path()));
        s.save_item(Kind::Layouts, "bug", &Layout::empty("bug", "Bug report"))
            .unwrap();
        let l = s
            .load_item::<Layout>(Kind::Layouts, "bug")
            .unwrap()
            .unwrap();
        assert_eq!(l.name, "Bug report");
        let text = std::fs::read_to_string(s.dirs().config().join("layouts/bug.json")).unwrap();
        assert!(text.contains("\"schema_version\": 1"));
        let chats = s.load_or_default::<cargoframe_handoff::ChatList>().unwrap();
        assert!(chats.is_empty());
    }
}
