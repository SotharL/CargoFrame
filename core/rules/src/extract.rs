//! Apply a rule to the text of a file.

use crate::model::{Format, Multiple, Rule};
use crate::path;
use serde_json::Value;

/// Why a rule gave no value. The messages are shown to the person as they are, so they say what
/// to change.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("no file matches \"{0}\" in the project folder")]
    FileNotFound(String),
    #[error("the file is larger than the {0} byte limit for this rule")]
    TooLarge(u64),
    #[error("the file could not be read: {0}")]
    Unreadable(String),
    #[error("the file is not valid {format}: {detail}")]
    Parse {
        format: &'static str,
        detail: String,
    },
    #[error("the query is not valid: {0}")]
    BadQuery(String),
    #[error("the query found no value")]
    NoValue,
    #[error("there is no project folder to read from")]
    NoProject,
}

const MAX_VALUES: usize = 1000;

/// Run the rule's query over `text`. Returns every value found (at most 1000), in order.
pub fn all_values(rule: &Rule, text: &str) -> Result<Vec<String>, RuleError> {
    let found = match rule.format {
        Format::Json => {
            let v: Value = serde_json::from_str(text).map_err(|e| parse_err("JSON", e))?;
            by_path(&v, &rule.query)?
        }
        Format::Yaml => {
            let v: Value = serde_yaml_ng::from_str(text).map_err(|e| parse_err("YAML", e))?;
            by_path(&v, &rule.query)?
        }
        Format::Toml => {
            let v: Value = toml::from_str(text).map_err(|e| parse_err("TOML", e))?;
            by_path(&v, &rule.query)?
        }
        Format::Ini => ini(text, &rule.query)?,
        Format::Csv => csv_column(text, &rule.query)?,
        Format::Xml => xml(text, &rule.query)?,
        Format::Regex => regex_values(text, &rule.query)?,
    };
    Ok(found.into_iter().take(MAX_VALUES).collect())
}

fn parse_err(format: &'static str, e: impl std::fmt::Display) -> RuleError {
    RuleError::Parse {
        format,
        detail: e.to_string(),
    }
}

/// The rule's value: what the query found, reduced to one text by `multiple`.
pub fn extract(rule: &Rule, text: &str) -> Result<String, RuleError> {
    let mut values = all_values(rule, text)?;
    if values.is_empty() {
        return Err(RuleError::NoValue);
    }
    Ok(match rule.multiple {
        Multiple::First => values.swap_remove(0),
        Multiple::Last => values.pop().unwrap_or_default(),
        Multiple::Join => values.join(&rule.separator),
    })
}

fn by_path(root: &Value, query: &str) -> Result<Vec<String>, RuleError> {
    let segs = path::parse(query).map_err(RuleError::BadQuery)?;
    Ok(path::select(root, &segs)
        .into_iter()
        .filter_map(path::text_of)
        .collect())
}

fn ini(text: &str, query: &str) -> Result<Vec<String>, RuleError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(RuleError::BadQuery("write section.key, or just key".into()));
    }
    let (want_section, want_key) = match query.rsplit_once('.') {
        Some((s, k)) => (s.trim().to_lowercase(), k.trim().to_lowercase()),
        None => (String::new(), query.to_lowercase()),
    };
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_lowercase();
            continue;
        }
        let Some((k, v)) = line.split_once('=').or_else(|| line.split_once(':')) else {
            continue;
        };
        if section == want_section && k.trim().to_lowercase() == want_key {
            out.push(unquote(v.trim()).to_string());
        }
    }
    Ok(out)
}

fn unquote(v: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = v.strip_prefix(q).and_then(|r| r.strip_suffix(q)) {
            return inner;
        }
    }
    v
}

fn csv_column(text: &str, query: &str) -> Result<Vec<String>, RuleError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(RuleError::BadQuery(
            "write a column name from the header row, or a column number".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());
    let headers = reader.headers().map_err(|e| parse_err("CSV", e))?.clone();
    let column = match headers
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case(query))
    {
        Some(i) => i,
        None => match query.parse::<usize>() {
            Ok(n) if n >= 1 => n - 1,
            _ => {
                return Err(RuleError::BadQuery(format!(
                    "the header row has no column \"{query}\""
                )))
            }
        },
    };
    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| parse_err("CSV", e))?;
        if let Some(cell) = record.get(column) {
            let cell = cell.trim();
            if !cell.is_empty() {
                out.push(cell.to_string());
            }
        }
        if out.len() >= MAX_VALUES {
            break;
        }
    }
    Ok(out)
}

fn xml(text: &str, query: &str) -> Result<Vec<String>, RuleError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(RuleError::BadQuery(
            "write element names such as a/b/c".into(),
        ));
    }
    let doc = roxmltree::Document::parse(text).map_err(|e| parse_err("XML", e))?;
    let (deep, rest) = match query.strip_prefix("//") {
        Some(r) => (true, r),
        None => (false, query.strip_prefix('/').unwrap_or(query)),
    };
    let mut names: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    let attr = match names.last() {
        Some(last) if last.starts_with('@') => names.pop().map(|a| &a[1..]),
        _ => None,
    };
    if names.is_empty() {
        return Err(RuleError::BadQuery("name at least one element".into()));
    }
    let root = doc.root();
    // Without `//` the first name must be the root element itself; with it, any element.
    let mut level: Vec<roxmltree::Node> = if deep {
        root.descendants()
            .filter(|n| n.is_element() && matches_name(n, names[0]))
            .collect()
    } else {
        root.children()
            .filter(|n| n.is_element() && matches_name(n, names[0]))
            .collect()
    };
    for name in &names[1..] {
        level = level
            .iter()
            .flat_map(|n| n.children())
            .filter(|n| n.is_element() && matches_name(n, name))
            .collect();
    }
    Ok(level
        .into_iter()
        .filter_map(|n| match attr {
            Some(a) => n.attribute(a).map(str::to_string),
            None => {
                let t: String = n
                    .descendants()
                    .filter(|d| d.is_text())
                    .filter_map(|d| d.text())
                    .collect();
                let t = t.trim().to_string();
                (!t.is_empty()).then_some(t)
            }
        })
        .collect())
}

fn matches_name(n: &roxmltree::Node, name: &str) -> bool {
    name == "*" || n.tag_name().name() == name
}

fn regex_values(text: &str, query: &str) -> Result<Vec<String>, RuleError> {
    if query.trim().is_empty() {
        return Err(RuleError::BadQuery("write a regular expression".into()));
    }
    let re = regex::RegexBuilder::new(query)
        .multi_line(true)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 22)
        .build()
        .map_err(|e| {
            RuleError::BadQuery(
                e.to_string()
                    .lines()
                    .last()
                    .unwrap_or("invalid")
                    .to_string(),
            )
        })?;
    let group = usize::from(re.captures_len() > 1);
    Ok(re
        .captures_iter(text)
        .filter_map(|c| c.get(group).map(|m| m.as_str().to_string()))
        .take(MAX_VALUES)
        .collect())
}

/// Check a rule as it is saved, without reading any file: the file pattern is usable and the
/// query is valid for the format. Returns what to change, in words.
pub fn check(rule: &Rule) -> Result<(), String> {
    if rule.name.trim().is_empty() {
        return Err("Give the rule a name".into());
    }
    if !crate::run::pattern_ok(&rule.file) {
        return Err(
            "The file must be a path inside the project folder, like package.json or config/*.toml"
                .into(),
        );
    }
    let q = rule.query.trim();
    if q.is_empty() && !matches!(rule.format, Format::Json | Format::Yaml | Format::Toml) {
        return Err("Write what to look for in the file".into());
    }
    match rule.format {
        Format::Json | Format::Yaml | Format::Toml => path::parse(&rule.query).map(|_| ()),
        Format::Regex => regex::RegexBuilder::new(&rule.query)
            .size_limit(1 << 20)
            .build()
            .map(|_| ())
            .map_err(|e| {
                e.to_string()
                    .lines()
                    .last()
                    .unwrap_or("invalid pattern")
                    .to_string()
            }),
        Format::Ini | Format::Csv | Format::Xml => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(format: Format, query: &str) -> Rule {
        Rule {
            format,
            query: query.into(),
            ..Rule::default()
        }
    }

    #[test]
    fn json_yaml_and_toml_share_one_path_language() {
        let json = r#"{"app": {"version": "1.2.3", "tags": ["a", "b"]}}"#;
        let yaml = "app:\n  version: 1.2.3\n  tags: [a, b]\n";
        let toml = "[app]\nversion = \"1.2.3\"\ntags = [\"a\", \"b\"]\n";
        for (f, text) in [
            (Format::Json, json),
            (Format::Yaml, yaml),
            (Format::Toml, toml),
        ] {
            assert_eq!(
                extract(&rule(f, "app.version"), text).unwrap(),
                "1.2.3",
                "{f:?}"
            );
            assert_eq!(
                extract(&rule(f, "app.tags[1]"), text).unwrap(),
                "b",
                "{f:?}"
            );
            let mut all = rule(f, "app.tags[*]");
            all.multiple = Multiple::Join;
            all.separator = " + ".into();
            assert_eq!(extract(&all, text).unwrap(), "a + b", "{f:?}");
        }
    }

    #[test]
    fn first_last_and_join() {
        let text = r#"{"v": [3, 5, 8]}"#;
        let mut r = rule(Format::Json, "v[*]");
        assert_eq!(extract(&r, text).unwrap(), "3");
        r.multiple = Multiple::Last;
        assert_eq!(extract(&r, text).unwrap(), "8");
        r.multiple = Multiple::Join;
        assert_eq!(extract(&r, text).unwrap(), "3, 5, 8");
    }

    #[test]
    fn a_missing_value_or_a_broken_file_is_a_clear_error() {
        assert_eq!(
            extract(&rule(Format::Json, "nope"), "{}").unwrap_err(),
            RuleError::NoValue
        );
        assert!(matches!(
            extract(&rule(Format::Json, "a"), "{not json").unwrap_err(),
            RuleError::Parse { format: "JSON", .. }
        ));
        assert!(matches!(
            extract(&rule(Format::Json, "a["), "{}").unwrap_err(),
            RuleError::BadQuery(_)
        ));
    }

    #[test]
    fn ini_sections_comments_quotes_and_case() {
        let text = "; comment\nname = top\n[Build]\nNumber = 7\nlabel: \"rc 1\"\n# more\n[other]\nnumber = 9\n";
        assert_eq!(extract(&rule(Format::Ini, "name"), text).unwrap(), "top");
        assert_eq!(
            extract(&rule(Format::Ini, "build.number"), text).unwrap(),
            "7"
        );
        assert_eq!(
            extract(&rule(Format::Ini, "BUILD.label"), text).unwrap(),
            "rc 1"
        );
        assert_eq!(
            extract(&rule(Format::Ini, "other.number"), text).unwrap(),
            "9"
        );
        assert_eq!(
            extract(&rule(Format::Ini, "build.missing"), text).unwrap_err(),
            RuleError::NoValue
        );
    }

    #[test]
    fn csv_by_name_or_number_with_quoted_cells() {
        let text = "id,name,note\n1,Ann,\"hi, there\"\n2,Bob,\n3,Cy,ok\n";
        let mut r = rule(Format::Csv, "name");
        r.multiple = Multiple::Join;
        assert_eq!(extract(&r, text).unwrap(), "Ann, Bob, Cy");
        let mut last = rule(Format::Csv, "3");
        last.multiple = Multiple::Last;
        assert_eq!(extract(&last, text).unwrap(), "ok");
        assert_eq!(
            extract(&rule(Format::Csv, "NOTE"), text).unwrap(),
            "hi, there"
        );
        assert!(matches!(
            extract(&rule(Format::Csv, "missing"), text).unwrap_err(),
            RuleError::BadQuery(_)
        ));
    }

    #[test]
    fn xml_paths_attributes_and_any_depth() {
        let text = r#"<project><version>2.0</version><deps><dep name="a"/><dep name="b"/></deps></project>"#;
        assert_eq!(
            extract(&rule(Format::Xml, "project/version"), text).unwrap(),
            "2.0"
        );
        assert_eq!(
            extract(&rule(Format::Xml, "//version"), text).unwrap(),
            "2.0"
        );
        let mut names = rule(Format::Xml, "project/deps/dep/@name");
        names.multiple = Multiple::Join;
        assert_eq!(extract(&names, text).unwrap(), "a, b");
        assert_eq!(
            extract(&rule(Format::Xml, "version"), text).unwrap_err(),
            RuleError::NoValue,
            "without // the first name must be the root"
        );
        assert!(matches!(
            extract(&rule(Format::Xml, "a"), "<a>").unwrap_err(),
            RuleError::Parse { format: "XML", .. }
        ));
    }

    #[test]
    fn regex_uses_group_one_and_all_matches() {
        let text = "build 41\nbuild 42\nother 9\n";
        let mut r = rule(Format::Regex, r"build (\d+)");
        assert_eq!(extract(&r, text).unwrap(), "41");
        r.multiple = Multiple::Last;
        assert_eq!(extract(&r, text).unwrap(), "42");
        assert_eq!(
            extract(&rule(Format::Regex, r"other \d"), text).unwrap(),
            "other 9"
        );
        assert!(matches!(
            extract(&rule(Format::Regex, "(unclosed"), text).unwrap_err(),
            RuleError::BadQuery(_)
        ));
    }

    #[test]
    fn a_hostile_regex_is_fast_and_a_huge_one_is_refused() {
        // Classic catastrophic pattern; the regex crate runs it in linear time.
        let text = "a".repeat(50_000) + "!";
        let started = std::time::Instant::now();
        let _ = extract(&rule(Format::Regex, r"(a+)+$"), &text);
        assert!(started.elapsed().as_secs() < 5);
        // A very large pattern is either refused or runs in bounded time; never a hang.
        let huge = format!("({}){{1000}}", "a|b|c|d|e|f".repeat(200));
        let started = std::time::Instant::now();
        assert!(extract(&rule(Format::Regex, &huge), "abc").is_err());
        assert!(started.elapsed().as_secs() < 5);
    }

    #[test]
    fn a_yaml_bomb_is_refused_not_expanded() {
        // Nine levels of nine aliases: about 387 million values if it were expanded.
        let mut text = String::from("a: &a [x, x, x, x, x, x, x, x, x]\n");
        for (prev, next) in "abcdefgh".chars().zip("bcdefghi".chars()) {
            let refs = format!("*{prev}, ").repeat(9);
            text.push_str(&format!(
                "{next}: &{next} [{}]\n",
                refs.trim_end_matches(", ")
            ));
        }
        let started = std::time::Instant::now();
        let result = extract(&rule(Format::Yaml, "a[0]"), &text);
        assert!(started.elapsed().as_secs() < 5, "took too long");
        // Either refused outright or answered without expanding everything.
        assert!(result.is_ok() || matches!(result, Err(RuleError::Parse { .. })));
    }

    #[test]
    fn empty_queries_are_rejected_with_advice() {
        for f in [Format::Ini, Format::Csv, Format::Xml, Format::Regex] {
            assert!(matches!(
                extract(&rule(f, " "), "x").unwrap_err(),
                RuleError::BadQuery(_)
            ));
        }
    }
}

#[cfg(test)]
mod check_tests {
    use super::*;

    fn rule(file: &str, format: Format, query: &str) -> Rule {
        Rule {
            name: "R".into(),
            file: file.into(),
            format,
            query: query.into(),
            ..Rule::default()
        }
    }

    #[test]
    fn a_good_rule_passes() {
        assert!(check(&rule("package.json", Format::Json, "version")).is_ok());
        assert!(
            check(&rule("**/*.toml", Format::Toml, "")).is_ok(),
            "the whole document"
        );
        assert!(check(&rule("a.ini", Format::Ini, "s.k")).is_ok());
    }

    #[test]
    fn each_mistake_says_what_to_change() {
        let mut unnamed = rule("a.json", Format::Json, "x");
        unnamed.name = " ".into();
        assert!(check(&unnamed).unwrap_err().contains("name"));
        assert!(check(&rule("../a.json", Format::Json, "x"))
            .unwrap_err()
            .contains("inside the project"));
        assert!(check(&rule("", Format::Json, "x")).is_err());
        assert!(check(&rule("a.json", Format::Json, "a[")).is_err());
        assert!(check(&rule("a.txt", Format::Regex, "(")).is_err());
        assert!(check(&rule("a.txt", Format::Regex, " "))
            .unwrap_err()
            .contains("look for"));
        assert!(check(&rule("a.csv", Format::Csv, "")).is_err());
    }
}
