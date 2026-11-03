//! Details of a file itself, whatever it holds: its size, when it changed, how many lines it has,
//! its hash. For "which build is this?" questions about a program, an archive, a database.
//!
//! The query names the detail, optionally with a style after a colon:
//!
//! | Query | Gives |
//! |-------|-------|
//! | `name` | the file's name, such as `app.log` |
//! | `size` / `size: human` | the size in bytes, or like `17.8 KB` |
//! | `modified` | when it last changed, in this computer's time: `2026-10-23 08:00:01` |
//! | `modified: date` / `utc` / `iso` | only the date, the time in UTC, or `2026-10-23T06:00:01Z` |
//! | `lines` | how many lines it has (text files only) |
//! | `sha256` / `sha256: short` | the file's SHA-256 fingerprint, or its first 12 characters |

use crate::extract::RuleError;
use chrono::{DateTime, Local, Utc};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::time::SystemTime;

/// The most that is read to count lines or compute a fingerprint.
pub const MAX_CONTENT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Default,
    Human,
    Date,
    Utc,
    Iso,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    Name,
    Size(Style),
    Modified(Style),
    Lines,
    Sha256(Style),
}

const HELP: &str =
    "write name, size, modified, lines or sha256, for example size or modified: date";

pub fn parse(query: &str) -> Result<What, String> {
    let query = query.trim();
    let (key, style) = match query.split_once(':') {
        Some((k, s)) => (k.trim(), s.trim().to_ascii_lowercase()),
        None => (query, String::new()),
    };
    let style_of = |allowed: &[(&str, Style)]| -> Result<Style, String> {
        if style.is_empty() {
            return Ok(Style::Default);
        }
        allowed
            .iter()
            .find(|(name, _)| *name == style)
            .map(|(_, s)| *s)
            .ok_or_else(|| {
                let names: Vec<&str> = allowed.iter().map(|(n, _)| *n).collect();
                format!("{key}: can be followed by {}", names.join(", "))
            })
    };
    match key.to_ascii_lowercase().as_str() {
        "name" => style_of(&[]).map(|_| What::Name),
        "size" => style_of(&[("human", Style::Human)]).map(What::Size),
        "modified" => style_of(&[
            ("date", Style::Date),
            ("utc", Style::Utc),
            ("iso", Style::Iso),
        ])
        .map(What::Modified),
        "lines" => style_of(&[]).map(|_| What::Lines),
        "sha256" | "hash" => style_of(&[("short", Style::Short)]).map(What::Sha256),
        _ => Err(HELP.into()),
    }
}

impl What {
    /// Does this need the file's content, not only what the file system says about it?
    pub fn needs_content(self) -> bool {
        matches!(self, What::Lines | What::Sha256(_))
    }
}

/// What was learned about one file. Only what was asked for is filled in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDetails {
    pub name: String,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub lines: Option<u64>,
    pub sha256: Option<String>,
}

/// Count lines and fingerprint what `reader` holds, reading at most [`MAX_CONTENT_BYTES`].
/// `size` is the file's size, to refuse a file too big before reading any of it.
pub fn read_content(
    reader: &mut dyn Read,
    size: u64,
    want: What,
) -> Result<(Option<u64>, Option<String>), RuleError> {
    if size > MAX_CONTENT_BYTES {
        return Err(RuleError::TooLarge(MAX_CONTENT_BYTES));
    }
    let hash = matches!(want, What::Sha256(_));
    let count = matches!(want, What::Lines);
    let mut hasher = Sha256::new();
    let mut newlines = 0u64;
    let mut last = b'\n';
    let mut any = false;
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| RuleError::Unreadable(e.to_string()))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_CONTENT_BYTES {
            return Err(RuleError::TooLarge(MAX_CONTENT_BYTES));
        }
        let chunk = &buf[..n];
        if hash {
            hasher.update(chunk);
        }
        if count {
            if chunk.contains(&0) {
                return Err(RuleError::Unreadable(
                    "it looks like a binary file, which has no lines".into(),
                ));
            }
            newlines += chunk.iter().filter(|b| **b == b'\n').count() as u64;
            last = chunk[n - 1];
            any = true;
        }
    }
    let lines = count.then(|| newlines + u64::from(any && last != b'\n'));
    let sha = hash.then(|| hex(&hasher.finalize()));
    Ok((lines, sha))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `17.8 KB`, `3 bytes`, `1.5 GB`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return if bytes == 1 {
            "1 byte".to_string()
        } else {
            format!("{bytes} bytes")
        };
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// The value the query asks for.
pub fn value(d: &FileDetails, what: What) -> Result<String, RuleError> {
    let missing = |what: &str| RuleError::Unreadable(format!("the {what} could not be read"));
    Ok(match what {
        What::Name => d.name.clone(),
        What::Size(Style::Human) => human_size(d.size),
        What::Size(_) => d.size.to_string(),
        What::Modified(style) => {
            let t = d.modified.ok_or_else(|| missing("modified time"))?;
            match style {
                Style::Date => DateTime::<Local>::from(t).format("%Y-%m-%d").to_string(),
                Style::Utc => DateTime::<Utc>::from(t)
                    .format("%Y-%m-%d %H:%M:%S UTC")
                    .to_string(),
                Style::Iso => DateTime::<Utc>::from(t)
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string(),
                _ => DateTime::<Local>::from(t)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            }
        }
        What::Lines => d.lines.ok_or_else(|| missing("line count"))?.to_string(),
        What::Sha256(style) => {
            let full = d.sha256.clone().ok_or_else(|| missing("fingerprint"))?;
            if style == Style::Short {
                full.chars().take(12).collect()
            } else {
                full
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn content(text: &[u8], want: What) -> (Option<u64>, Option<String>) {
        read_content(&mut &text[..], text.len() as u64, want).unwrap()
    }

    #[test]
    fn queries_name_a_detail_and_maybe_a_style() {
        assert_eq!(parse("name"), Ok(What::Name));
        assert_eq!(parse(" Size "), Ok(What::Size(Style::Default)));
        assert_eq!(parse("size: human"), Ok(What::Size(Style::Human)));
        assert_eq!(parse("modified"), Ok(What::Modified(Style::Default)));
        assert_eq!(parse("modified: Date"), Ok(What::Modified(Style::Date)));
        assert_eq!(parse("modified: utc"), Ok(What::Modified(Style::Utc)));
        assert_eq!(parse("lines"), Ok(What::Lines));
        assert_eq!(parse("sha256: short"), Ok(What::Sha256(Style::Short)));
        assert_eq!(parse("hash"), Ok(What::Sha256(Style::Default)));
        assert!(parse("").unwrap_err().contains("name, size"));
        assert!(parse("colour").is_err());
        assert!(parse("size: huge").unwrap_err().contains("human"));
        assert!(parse("lines: 5").is_err());
    }

    #[test]
    fn only_lines_and_the_fingerprint_need_the_content() {
        assert!(What::Lines.needs_content());
        assert!(What::Sha256(Style::Short).needs_content());
        assert!(!What::Size(Style::Default).needs_content());
        assert!(!What::Modified(Style::Date).needs_content());
        assert!(!What::Name.needs_content());
    }

    #[test]
    fn lines_count_the_last_line_with_or_without_a_final_newline() {
        let lines = |t: &[u8]| content(t, What::Lines).0;
        assert_eq!(lines(b""), Some(0));
        assert_eq!(lines(b"one"), Some(1));
        assert_eq!(lines(b"one\n"), Some(1));
        assert_eq!(lines(b"one\ntwo"), Some(2));
        assert_eq!(lines(b"one\r\ntwo\r\n"), Some(2));
        assert_eq!(lines(b"\n\n"), Some(2));
    }

    #[test]
    fn lines_across_the_read_buffer_edge_are_counted_once() {
        let mut text = vec![b'a'; 64 * 1024 - 1];
        text.push(b'\n');
        text.extend_from_slice(b"tail");
        assert_eq!(content(&text, What::Lines).0, Some(2));
    }

    #[test]
    fn a_binary_file_has_no_lines_but_does_have_a_fingerprint() {
        let bin = [0u8, 1, 2, 10, 3];
        let err = read_content(&mut &bin[..], 5, What::Lines).unwrap_err();
        assert!(err.to_string().contains("binary"));
        assert!(content(&bin, What::Sha256(Style::Default)).1.is_some());
    }

    #[test]
    fn the_fingerprint_is_sha256_in_lowercase_hex() {
        let (_, sha) = content(b"abc", What::Sha256(Style::Default));
        assert_eq!(
            sha.unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let (_, empty) = content(b"", What::Sha256(Style::Default));
        assert_eq!(
            empty.unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_file_over_the_limit_is_refused_before_any_of_it_is_read() {
        let err =
            read_content(&mut std::io::empty(), MAX_CONTENT_BYTES + 1, What::Lines).unwrap_err();
        assert!(matches!(err, RuleError::TooLarge(_)));
    }

    #[test]
    fn sizes_read_like_a_person_would_say_them() {
        assert_eq!(human_size(0), "0 bytes");
        assert_eq!(human_size(1), "1 byte");
        assert_eq!(human_size(1023), "1023 bytes");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(18_204), "17.8 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(human_size(1536 * 1024 * 1024), "1.5 GB");
        assert_eq!(human_size(u64::MAX), "16777216.0 TB");
    }

    #[test]
    fn values_come_out_as_text() {
        let d = FileDetails {
            name: "app.log".into(),
            size: 18_204,
            modified: Some(UNIX_EPOCH + Duration::from_secs(1_792_000_000)),
            lines: Some(42),
            sha256: Some("0123456789abcdef0123".into()),
        };
        let v = |q: &str| value(&d, parse(q).unwrap()).unwrap();
        assert_eq!(v("name"), "app.log");
        assert_eq!(v("size"), "18204");
        assert_eq!(v("size: human"), "17.8 KB");
        assert_eq!(v("lines"), "42");
        assert_eq!(v("sha256"), "0123456789abcdef0123");
        assert_eq!(v("sha256: short"), "0123456789ab");
        assert_eq!(v("modified: utc"), "2026-10-14 17:46:40 UTC");
        assert_eq!(v("modified: iso"), "2026-10-14T17:46:40Z");
        // Local time depends on the computer: only its shape is checked.
        assert_eq!(v("modified").len(), "2026-10-14 17:46:40".len());
        assert_eq!(v("modified: date").len(), "2026-10-14".len());
    }

    #[test]
    fn a_detail_that_was_not_read_says_so() {
        let d = FileDetails::default();
        assert!(value(&d, What::Lines)
            .unwrap_err()
            .to_string()
            .contains("line count"));
        assert!(value(&d, What::Modified(Style::Date)).is_err());
        assert!(value(&d, What::Sha256(Style::Default)).is_err());
    }
}
