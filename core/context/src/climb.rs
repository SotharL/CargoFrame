//! Resolve the project folder for the program that had focus when a trigger fired.
//!
//! Rule (from the project brief): take the focused program's file path and climb back to the
//! folder that has the same name as the program. When no ancestor folder carries that name,
//! use the folder one level above the program file, which is its own parent folder.
//!
//! Example: `C:\Dev\Foo\bin\Release\Foo.exe` resolves to `C:\Dev\Foo`.
//!
//! The functions work on plain strings and accept both `\` and `/`, so the same logic is
//! testable on every platform and behaves the same for Windows and Linux paths.

/// How the folder was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match {
    /// An ancestor folder has the same name as the program. `levels_up` counts folders
    /// climbed from the program's own folder (0 = the program sits directly inside it).
    SameName { levels_up: usize },
    /// No ancestor matched; the program's own folder is used.
    ParentFallback,
}

/// A resolved project folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// Folder path, written with the separator style of the input.
    pub folder: String,
    pub how: Match,
}

/// Climb from `program_path` to the project folder.
///
/// Returns `None` when the path has no folder part at all (for example a bare file name).
pub fn resolve_project_folder(program_path: &str) -> Option<Resolution> {
    let sep = if program_path.contains('\\') {
        '\\'
    } else {
        '/'
    };
    let parts = split(program_path);
    // Need at least a folder and a file name.
    if parts.len() < 2 {
        return None;
    }
    let (file, dirs) = parts.split_last()?;
    let stem = file_stem(file);
    if stem.is_empty() {
        return None;
    }

    let prefix = root_prefix(program_path, sep);

    // Climb from the program's own folder upwards. `dirs` is ordered root to leaf, so walk
    // it from the end. A leading drive such as `C:` is a root marker, never a candidate.
    for (levels_up, idx) in (0..dirs.len()).rev().enumerate() {
        let name = &dirs[idx];
        if is_drive(name) {
            continue;
        }
        if name.eq_ignore_ascii_case(stem) {
            return Some(Resolution {
                folder: join(&prefix, &dirs[..=idx], sep),
                how: Match::SameName { levels_up },
            });
        }
    }

    Some(Resolution {
        folder: join(&prefix, dirs, sep),
        how: Match::ParentFallback,
    })
}

/// Split on both separators and drop empty segments.
fn split(path: &str) -> Vec<&str> {
    path.split(['\\', '/']).filter(|s| !s.is_empty()).collect()
}

fn is_drive(segment: &str) -> bool {
    let b = segment.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// `Foo.exe` -> `Foo`, `foo.tar.gz` -> `foo.tar`, `.hidden` -> `.hidden`, `app` -> `app`.
fn file_stem(file: &str) -> &str {
    match file.rfind('.') {
        Some(0) | None => file,
        Some(i) => &file[..i],
    }
}

/// What a path starts with before the first segment: `/` for rooted Unix paths,
/// `\\` for UNC paths, nothing for drive paths (the drive is a segment of its own).
fn root_prefix(path: &str, sep: char) -> String {
    if path.starts_with("\\\\") {
        "\\\\".to_string()
    } else if path.starts_with('/') || (sep == '\\' && path.starts_with('\\')) {
        sep.to_string()
    } else {
        String::new()
    }
}

fn join(prefix: &str, parts: &[&str], sep: char) -> String {
    let mut out = String::from(prefix);
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.push_str(p);
    }
    // A bare drive ("C:") is not a usable folder; give it its trailing separator.
    if parts.len() == 1 && is_drive(parts[0]) {
        out.push(sep);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(p: &str) -> String {
        resolve_project_folder(p).unwrap().folder
    }

    #[test]
    fn climbs_to_same_name_folder_on_windows() {
        let r = resolve_project_folder(r"C:\Dev\Foo\bin\Release\Foo.exe").unwrap();
        assert_eq!(r.folder, r"C:\Dev\Foo");
        assert_eq!(r.how, Match::SameName { levels_up: 2 });
    }

    #[test]
    fn climbs_to_same_name_folder_on_linux() {
        let r = resolve_project_folder("/home/ann/src/foo/target/release/foo").unwrap();
        assert_eq!(r.folder, "/home/ann/src/foo");
        assert_eq!(r.how, Match::SameName { levels_up: 2 });
    }

    #[test]
    fn program_directly_inside_same_name_folder() {
        let r = resolve_project_folder(r"C:\Tools\Foo\Foo.exe").unwrap();
        assert_eq!(r.folder, r"C:\Tools\Foo");
        assert_eq!(r.how, Match::SameName { levels_up: 0 });
    }

    #[test]
    fn match_ignores_case_and_extension() {
        assert_eq!(folder(r"C:\Apps\FOO\x\foo.EXE"), r"C:\Apps\FOO");
    }

    #[test]
    fn nearest_match_wins() {
        assert_eq!(folder(r"C:\foo\a\foo\b\foo.exe"), r"C:\foo\a\foo");
    }

    #[test]
    fn falls_back_to_the_programs_own_folder() {
        let r = resolve_project_folder(r"C:\Program Files\Tool\bin\run.exe").unwrap();
        assert_eq!(r.folder, r"C:\Program Files\Tool\bin");
        assert_eq!(r.how, Match::ParentFallback);
    }

    #[test]
    fn fallback_directly_under_drive_keeps_a_usable_root() {
        assert_eq!(folder(r"C:\run.exe"), r"C:\");
    }

    #[test]
    fn drive_letter_is_never_a_candidate() {
        // The program is called "C" and sits on drive C: the drive must not match.
        let r = resolve_project_folder(r"C:\tools\c.exe").unwrap();
        assert_eq!(r.how, Match::ParentFallback);
    }

    #[test]
    fn forward_slashes_on_windows_input_are_kept() {
        assert_eq!(folder("C:/Dev/Foo/bin/Foo.exe"), "C:/Dev/Foo");
    }

    #[test]
    fn unc_paths_keep_their_prefix() {
        assert_eq!(
            folder(r"\\server\share\Foo\bin\Foo.exe"),
            r"\\server\share\Foo"
        );
    }

    #[test]
    fn dotted_names_use_the_last_extension_only() {
        assert_eq!(folder("/opt/my.app/bin/my.app.sh"), "/opt/my.app");
        assert_eq!(folder("/opt/tool/.hidden"), "/opt/tool");
    }

    #[test]
    fn no_folder_part_returns_none() {
        assert!(resolve_project_folder("foo.exe").is_none());
        assert!(resolve_project_folder("").is_none());
        assert!(resolve_project_folder("/").is_none());
    }

    #[test]
    fn extensionless_unix_binary_in_root_folder() {
        assert_eq!(folder("/usr/bin/foo"), "/usr/bin");
        assert_eq!(folder("/foo/foo"), "/foo");
    }
}
