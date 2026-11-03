//! File names for fallback zips: `CargoFrame_<date>_<time>_<service>.zip`, or a name the person
//! typed in a Zip name block.

/// Lowercase, `a-z0-9` and single dashes, at most 32 characters. Empty input becomes `share`.
pub fn sanitize_service(service: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // suppresses a leading dash
    for c in service.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(32);
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "share".to_string()
    } else {
        out
    }
}

/// `CargoFrame_2026-10-23_153045_jira.zip`
pub fn zip_file_name(date: (u16, u8, u8), time: (u8, u8, u8), service: &str) -> String {
    format!(
        "CargoFrame_{:04}-{:02}-{:02}_{:02}{:02}{:02}_{}.zip",
        date.0,
        date.1,
        date.2,
        time.0,
        time.1,
        time.2,
        sanitize_service(service)
    )
}

/// Longest name (before `.zip`) taken from a Zip name block.
const MAX_CUSTOM_STEM: usize = 80;

/// Names Windows refuses for a file whatever the extension.
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A zip file name made from what the person typed: `Bug in login` becomes `Bug in login.zip`.
/// Anything that could leave the folder or that a file system refuses is replaced, so the result
/// is always a plain file name. `None` when nothing usable is left (the default name is used).
pub fn custom_zip_file_name(typed: &str) -> Option<String> {
    let mut stem = String::new();
    let mut last_blank = false;
    for c in typed.chars() {
        let c = match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        };
        if c.is_whitespace() {
            if !last_blank {
                stem.push(' ');
            }
            last_blank = true;
        } else {
            stem.push(c);
            last_blank = false;
        }
    }
    let mut stem = stem.trim().to_string();
    // "report.zip" typed by hand: one extension is enough.
    if stem.len() >= 4
        && stem.is_char_boundary(stem.len() - 4)
        && stem[stem.len() - 4..].eq_ignore_ascii_case(".zip")
    {
        let cut = stem.len() - 4;
        stem.truncate(cut);
    }
    let mut stem = stem
        .trim_matches(|c: char| c == ' ' || c == '.' || c == '-')
        .to_string();
    if stem.chars().count() > MAX_CUSTOM_STEM {
        stem = stem.chars().take(MAX_CUSTOM_STEM).collect();
        stem = stem
            .trim_end_matches(|c: char| c == ' ' || c == '.')
            .to_string();
    }
    if stem.is_empty() {
        return None;
    }
    let first_word = stem
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if RESERVED.contains(&first_word.as_str()) {
        stem.insert(0, '_');
    }
    Some(format!("{stem}.zip"))
}

/// The temporary name used while writing; renamed to the final name when complete.
pub fn part_name(final_name: &str) -> String {
    format!("{final_name}.part")
}

/// First free name: `x.zip`, `x-2.zip`, `x-3.zip`, ... `exists` tells whether a name is taken.
pub fn unique_name(name: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2u32..)
        .map(|n| format!("{stem}-{n}{ext}"))
        .find(|candidate| !exists(candidate))
        .unwrap_or_else(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_documented_name() {
        assert_eq!(
            zip_file_name((2026, 10, 23), (15, 30, 45), "Jira"),
            "CargoFrame_2026-10-23_153045_jira.zip"
        );
        assert_eq!(
            zip_file_name((2026, 1, 2), (3, 4, 5), "Custom API"),
            "CargoFrame_2026-01-02_030405_custom-api.zip"
        );
    }

    #[test]
    fn sanitizes_service_names() {
        assert_eq!(sanitize_service("  Jira Cloud!! "), "jira-cloud");
        assert_eq!(sanitize_service("../../etc"), "etc");
        assert_eq!(sanitize_service(""), "share");
        assert_eq!(sanitize_service("***"), "share");
        assert_eq!(sanitize_service("é"), "share");
        let long = sanitize_service(&"a-".repeat(40));
        assert!(long.len() <= 32 && !long.ends_with('-'));
    }

    #[test]
    fn typed_names_become_plain_zip_names() {
        assert_eq!(
            custom_zip_file_name("Bug in login").as_deref(),
            Some("Bug in login.zip")
        );
        assert_eq!(
            custom_zip_file_name("  spaced   out  ").as_deref(),
            Some("spaced out.zip")
        );
        assert_eq!(
            custom_zip_file_name("report.zip").as_deref(),
            Some("report.zip")
        );
        assert_eq!(
            custom_zip_file_name("Report.ZIP").as_deref(),
            Some("Report.zip")
        );
        assert_eq!(
            custom_zip_file_name("v1.2 notes").as_deref(),
            Some("v1.2 notes.zip")
        );
        assert_eq!(
            custom_zip_file_name("café 2026").as_deref(),
            Some("café 2026.zip")
        );
    }

    #[test]
    fn typed_names_cannot_leave_the_folder() {
        assert_eq!(
            custom_zip_file_name("../../etc/passwd").as_deref(),
            Some("etc-passwd.zip")
        );
        assert_eq!(
            custom_zip_file_name("C:\\Windows\\x").as_deref(),
            Some("C--Windows-x.zip")
        );
        assert_eq!(
            custom_zip_file_name("a:b*c?d\"e<f>g|h").as_deref(),
            Some("a-b-c-d-e-f-g-h.zip")
        );
        assert_eq!(
            custom_zip_file_name("line\nbreak\tand\u{7}bell").as_deref(),
            Some("line break and bell.zip")
        );
    }

    #[test]
    fn typed_names_avoid_windows_reserved_names() {
        assert_eq!(custom_zip_file_name("con").as_deref(), Some("_con.zip"));
        assert_eq!(
            custom_zip_file_name("NUL.txt").as_deref(),
            Some("_NUL.txt.zip")
        );
        assert_eq!(custom_zip_file_name("COM1").as_deref(), Some("_COM1.zip"));
        assert_eq!(
            custom_zip_file_name("console").as_deref(),
            Some("console.zip")
        );
    }

    #[test]
    fn typed_names_that_are_empty_fall_back() {
        assert_eq!(custom_zip_file_name(""), None);
        assert_eq!(custom_zip_file_name("   "), None);
        assert_eq!(custom_zip_file_name("..."), None);
        assert_eq!(custom_zip_file_name(".zip"), None);
        assert_eq!(custom_zip_file_name("///"), None);
    }

    #[test]
    fn typed_names_are_cut_to_a_sane_length() {
        let name = custom_zip_file_name(&"ab ".repeat(100)).unwrap();
        assert!(name.ends_with(".zip") && name.chars().count() <= MAX_CUSTOM_STEM + 4);
        assert!(!name.trim_end_matches(".zip").ends_with(' '));
        let wide = custom_zip_file_name(&"é".repeat(200)).unwrap();
        assert_eq!(wide.chars().count(), MAX_CUSTOM_STEM + 4);
    }

    #[test]
    fn part_name_appends_suffix() {
        assert_eq!(part_name("a.zip"), "a.zip.part");
    }

    #[test]
    fn unique_name_counts_up() {
        let taken = ["a.zip", "a-2.zip"];
        assert_eq!(unique_name("a.zip", |n| taken.contains(&n)), "a-3.zip");
        assert_eq!(unique_name("b.zip", |n| taken.contains(&n)), "b.zip");
        assert_eq!(unique_name("noext", |n| n == "noext"), "noext-2");
    }
}
