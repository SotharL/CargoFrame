//! File names for fallback zips: `CargoFrame_<date>_<time>_<service>.zip`.

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
