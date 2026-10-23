//! Names inside the zip.

use std::collections::BTreeSet;

const MAX_NAME: usize = 240;

/// A zip entry name made from a project-relative path: `/` separators, no `..`, no drive or
/// leading slash, no control characters, not too long. `None` when nothing safe is left.
pub fn safe_name(raw: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for part in raw.replace('\\', "/").split('/') {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') {
            return None;
        }
        let cleaned: String = part.chars().filter(|c| !c.is_control()).collect();
        if cleaned.is_empty() {
            return None;
        }
        parts.push(cleaned);
    }
    if parts.is_empty() {
        return None;
    }
    let mut name = parts.join("/");
    if name.chars().count() > MAX_NAME {
        // Keep the end: the file name and extension matter most.
        let skip = name.chars().count() - MAX_NAME;
        name = name.chars().skip(skip).collect();
        name = name.trim_start_matches('/').to_string();
    }
    Some(name)
}

/// Hands out unique names: a repeat becomes `name-2.ext`, `name-3.ext`, and so on.
#[derive(Default)]
pub struct Names {
    used: BTreeSet<String>,
}

impl Names {
    pub fn unique(&mut self, name: &str) -> String {
        let key = |n: &str| n.to_lowercase();
        if self.used.insert(key(name)) {
            return name.to_string();
        }
        let (dir, file) = match name.rfind('/') {
            Some(i) => (&name[..=i], &name[i + 1..]),
            None => ("", name),
        };
        let (stem, ext) = match file.rfind('.') {
            Some(i) if i > 0 => (&file[..i], &file[i..]),
            _ => (file, ""),
        };
        for n in 2u32.. {
            let candidate = format!("{dir}{stem}-{n}{ext}");
            if self.used.insert(key(&candidate)) {
                return candidate;
            }
        }
        unreachable!("an unused name always exists")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_are_normalised_and_dots_dropped() {
        assert_eq!(safe_name("logs\\a.log").as_deref(), Some("logs/a.log"));
        assert_eq!(safe_name("./a//b.txt").as_deref(), Some("a/b.txt"));
        assert_eq!(safe_name(" a / b ").as_deref(), Some("a/b"));
    }

    #[test]
    fn anything_that_could_escape_is_refused() {
        for bad in ["../a", "a/../b", "C:\\x", "a:b", "", "/", "..", "\u{7}"] {
            assert_eq!(safe_name(bad), None, "{bad:?}");
        }
        // A leading slash only means "from the root of the zip", which is where it ends up.
        assert_eq!(safe_name("/etc/passwd").as_deref(), Some("etc/passwd"));
    }

    #[test]
    fn control_characters_are_removed_and_long_names_keep_their_end() {
        assert_eq!(safe_name("a\u{1}b.txt").as_deref(), Some("ab.txt"));
        let long = format!("{}/file.log", "d".repeat(400));
        let name = safe_name(&long).unwrap();
        assert!(name.chars().count() <= MAX_NAME);
        assert!(name.ends_with("file.log"));
    }

    #[test]
    fn duplicates_get_a_counter_ignoring_case() {
        let mut n = Names::default();
        assert_eq!(n.unique("files/a.log"), "files/a.log");
        assert_eq!(n.unique("files/A.log"), "files/A-2.log");
        assert_eq!(n.unique("files/a.log"), "files/a-3.log");
        assert_eq!(n.unique("README"), "README");
        assert_eq!(n.unique("README"), "README-2");
    }
}
