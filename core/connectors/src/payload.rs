//! What a share carries, and what comes back.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

/// Copies on disk of what a capture holds.
#[derive(Debug, Clone, Default)]
pub struct LocalCopies {
    /// The screenshot as shown (edited, if the person edited it).
    pub screenshot: Option<PathBuf>,
    /// The ticked files, with the name each has in the project (`logs/app.log`).
    pub files: Vec<(String, PathBuf)>,
}

/// A text file written into a message instead of being attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineFile {
    /// The name the file has in the project (`logs/app.log`).
    pub name: String,
    pub text: String,
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
    /// The screenshot as shown (edited, if the person edited it). For a recording, the recording.
    pub image: Option<Attachment>,
    /// A screenshot that goes with a recording (a frame taken from it, say). Only set when `image`
    /// is the recording: otherwise the screenshot is `image`.
    pub still: Option<Attachment>,
    /// The ticked project files, zipped. `None` when none were ticked.
    pub files_zip: Option<Attachment>,
    /// Copies of the screenshot and the ticked files on disk, in the capture's own folder, for a
    /// destination that takes real files (a chat they are pasted into). `None` for the others.
    pub local: Option<LocalCopies>,
    /// Small text files the person chose to write into the message (Claude only).
    pub inline_files: Vec<InlineFile>,
    /// The prompt, for destinations that use one.
    pub prompt: Option<String>,
    /// Mapped fields, in the order the layout lists them.
    pub fields: Vec<Field>,
    /// Every variable of the capture (`app.title`, `rule.version`...), for templates.
    pub vars: BTreeMap<String, String>,
}

/// Where the person put the screenshot in a text, for services that can show it there.
pub const SCREENSHOT_TOKEN: &str = "{{capture.screenshot}}";
/// Where the person put the recording in a text.
pub const RECORDING_TOKEN: &str = "{{capture.recording}}";

/// Put the screenshot and the recording where the text says: `shot` and `rec` are what each
/// place becomes (empty where the service cannot show it in its text: the file is then sent the
/// usual way).
pub fn place_media(text: &str, shot: &str, rec: &str) -> String {
    text.replace(SCREENSHOT_TOKEN, shot)
        .replace(RECORDING_TOKEN, rec)
}

/// Whether a text has a place for the screenshot, and for the recording.
pub fn media_places(text: &str) -> (bool, bool) {
    (
        text.contains(SCREENSHOT_TOKEN),
        text.contains(RECORDING_TOKEN),
    )
}

impl Payload {
    /// The screenshot: the picture of a screenshot capture, or the frame that goes with a
    /// recording.
    pub fn screenshot(&self) -> Option<&Attachment> {
        match &self.image {
            Some(a) if a.mime.starts_with("image/") => Some(a),
            _ => self.still.as_ref(),
        }
    }

    /// The recording, when the capture is one.
    pub fn recording(&self) -> Option<&Attachment> {
        self.image.as_ref().filter(|a| a.mime.starts_with("video/"))
    }

    /// Every text of the share (fields and prompt) with the places for the screenshot and the
    /// recording taken out: for services that cannot show them in a text.
    pub fn without_media_places(mut self) -> Payload {
        for f in &mut self.fields {
            f.value = place_media(&f.value, "", "");
        }
        if let Some(p) = &mut self.prompt {
            *p = place_media(p, "", "");
        }
        self
    }

    /// The value of a field, trimmed, if there is one.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.trim())
            .filter(|v| !v.is_empty())
    }

    /// The words of a chat message: the `message` field, else the prompt, else the window title
    /// (unless a title of its own is shown above it, `titled`), then the tags as a last line.
    pub fn message(&self, titled: bool) -> Option<String> {
        let body = self
            .value("message")
            .map(str::to_string)
            .or_else(|| {
                self.prompt
                    .as_deref()
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
            })
            .or_else(|| Some(self.title.trim().to_string()).filter(|t| !t.is_empty() && !titled));
        with_tags_line(body, self)
    }

    /// The values of a field that takes several (`crash, gpu`), each once, in order.
    pub fn list(&self, key: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for v in self.value(key).unwrap_or("").split([',', '\n']) {
            let v = v.trim();
            if !v.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(v)) {
                out.push(v.to_string());
            }
        }
        out
    }

    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.key == key && !f.value.trim().is_empty())
    }
}

/// A tag as a hashtag: `gpu driver` → `#gpuDriver`, `C++` → `#C`. Letters, digits and `_` only;
/// `None` when none is left.
pub fn hashtag(tag: &str) -> Option<String> {
    let mut out = String::from("#");
    for (i, word) in tag.split_whitespace().enumerate() {
        let word: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            if i > 0 && out.len() > 1 {
                out.extend(first.to_uppercase());
            } else {
                out.push(first);
            }
            out.extend(chars);
        }
    }
    (out.len() > 1).then_some(out)
}

/// The tags as hashtags, one space apart (each once).
pub fn hashtags(tags: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    for t in tags.iter().filter_map(|t| hashtag(t)) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out.join(" ")
}

/// A message with its tags (the `tags` field) as a last line, written with marks the services
/// turn into their own: "**Tags:** `crash` `gpu`".
pub fn with_tags_line(body: Option<String>, payload: &Payload) -> Option<String> {
    let tags = payload.list("tags");
    if tags.is_empty() {
        return body;
    }
    let line = format!(
        "**Tags:** {}",
        tags.iter()
            .map(|t| format!("`{}`", t.replace('`', "'")))
            .collect::<Vec<_>>()
            .join(" ")
    );
    Some(match body {
        Some(b) if !b.trim().is_empty() => format!("{}\n\n{line}", b.trim_end()),
        _ => line,
    })
}

/// A post's text with its hashtags (the `hashtags` field) at the end, within `max` characters:
/// the text is cut first, so the hashtags stay. Whether the text was cut is the second value.
pub fn with_hashtags(source: &str, payload: &Payload, max: usize) -> (String, bool) {
    let tags = hashtags(&payload.list("hashtags"));
    if tags.is_empty() {
        return crate::media::plain_text(source, max);
    }
    let room = max.saturating_sub(tags.chars().count() + 2);
    let (text, cut) = crate::media::plain_text(source, room);
    if text.is_empty() {
        let all: String = tags.chars().take(max).collect();
        return (all, cut);
    }
    (format!("{text}\n\n{tags}"), cut)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OptionItem {
    pub value: String,
    pub label: String,
    /// A heading the choice is listed under ("Pinned", "Public channels").
    pub group: Option<String>,
    /// Where the choice lives, for a list the window narrows step by step: the server...
    pub server: Option<String>,
    /// ...and, inside it, the category (a Discord category, the channel a thread is in).
    pub category: Option<String>,
}

impl OptionItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            group: None,
            server: None,
            category: None,
        }
    }

    /// Place the choice under a server and, optionally, a category.
    pub fn within(mut self, server: &str, category: Option<&str>) -> Self {
        self.server = Some(server.to_string());
        self.category = category.map(String::from);
        self
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
    /// A place the share made or used that can be picked again next time (a Discord forum post
    /// the service cannot list). The window keeps it with the recent choices.
    pub place: Option<OptionItem>,
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

    fn with(fields: &[(&str, &str)]) -> Payload {
        Payload {
            fields: fields.iter().map(|(k, v)| Field::text(k, v)).collect(),
            ..Payload::default()
        }
    }

    #[test]
    fn tags_become_hashtags_and_a_last_line_of_a_message() {
        assert_eq!(hashtag("gpu driver").as_deref(), Some("#gpuDriver"));
        assert_eq!(hashtag("C++").as_deref(), Some("#C"));
        assert_eq!(hashtag("!!"), None);
        let p = with(&[
            ("tags", "crash, gpu, Crash"),
            ("hashtags", "bug report, ui"),
        ]);
        assert_eq!(p.list("tags"), ["crash", "gpu"]);
        assert_eq!(
            with_tags_line(Some("It broke".into()), &p).as_deref(),
            Some("It broke\n\n**Tags:** `crash` `gpu`")
        );
        assert_eq!(with_tags_line(None, &with(&[])), None);
        let (text, cut) = with_hashtags("x".repeat(300).as_str(), &p, 280);
        assert!(cut);
        assert!(text.ends_with("\n\n#bugReport #ui"), "{text}");
        assert!(text.chars().count() <= 280);
        assert_eq!(
            with_hashtags("Hi", &with(&[]), 280),
            ("Hi".to_string(), false)
        );
    }
}
