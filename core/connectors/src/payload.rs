//! What a share carries, and what comes back.

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

/// How a service field is shaped, from the person's info type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldKind {
    #[default]
    Text,
    Choice,
    MultiChoice,
    Number,
    Flag,
}

/// One value for a field of the destination service (`priority`, `subreddit`, `col1`...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub value: String,
    pub kind: FieldKind,
    /// The value was picked from a list the service itself supplied (so it is an id, not a
    /// label the person typed).
    pub from_list: bool,
}

impl Field {
    pub fn text(key: &str, value: &str) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            kind: FieldKind::Text,
            from_list: false,
        }
    }

    pub fn choice(key: &str, value: &str, from_list: bool) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            kind: FieldKind::Choice,
            from_list,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Payload {
    /// The captured window's title (or "Screen"): a default subject for services that need one.
    pub title: String,
    /// The screenshot as shown (edited, if the person edited it).
    pub image: Option<Attachment>,
    /// The ticked project files, zipped. `None` when none were ticked.
    pub files_zip: Option<Attachment>,
    /// The prompt, for destinations that use one.
    pub prompt: Option<String>,
    /// Mapped fields, in the order the layout lists them.
    pub fields: Vec<Field>,
    /// Every variable of the capture (`app.title`, `rule.version`...), for templates.
    pub vars: BTreeMap<String, String>,
}

impl Payload {
    /// The value of a field, trimmed, if there is one.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.trim())
            .filter(|v| !v.is_empty())
    }

    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.key == key && !f.value.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OptionItem {
    pub value: String,
    pub label: String,
    pub group: Option<String>,
}

impl OptionItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            group: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SendOutcome {
    /// One line for the person: "Created PROJ-123".
    pub summary: String,
    /// A link to what was created, if the service gives one.
    pub url: Option<String>,
    /// Things that did not go as asked but did not stop the share ("files were skipped").
    pub notes: Vec<String>,
    /// Text the window should offer to copy next ("Copy prompt"), for hand-offs that need a
    /// second paste.
    pub copy_next: Option<String>,
}

/// What an earlier attempt of the same share already did, so a retry continues instead of
/// repeating (an issue must not be created twice because an attachment upload timed out).
#[derive(Default)]
pub struct Progress(Mutex<BTreeMap<String, String>>);

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    pub fn set(&self, key: &str, value: &str) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.into(), value.into());
    }

    pub fn clear(&self, key: &str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
    }

    pub fn done(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_values_are_trimmed_and_empty_ones_do_not_count() {
        let p = Payload {
            fields: vec![Field::text("a", "  x "), Field::text("b", "   ")],
            ..Payload::default()
        };
        assert_eq!(p.value("a"), Some("x"));
        assert_eq!(p.value("b"), None);
        assert!(p.field("b").is_none());
        assert_eq!(p.value("nope"), None);
    }

    #[test]
    fn progress_remembers() {
        let p = Progress::new();
        assert!(!p.done("k"));
        p.set("k", "v");
        assert_eq!(p.get("k").as_deref(), Some("v"));
        assert!(p.done("k"));
    }
}
