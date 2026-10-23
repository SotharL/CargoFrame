//! A small path language for structured data: `a.b`, `$.a.b`, `a[0]`, `a["key.with.dots"]`,
//! `items[*].name`. It covers what people write for "the version in package.json" without the
//! weight of a full JSONPath engine.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
    Each,
}

/// Parse a path. An empty path (or just `$`) means the whole document.
pub fn parse(path: &str) -> Result<Vec<Seg>, String> {
    let chars: Vec<char> = path.trim().chars().collect();
    let mut segs = Vec::new();
    let mut i = 0;
    if chars.first() == Some(&'$') {
        i = 1;
    }
    while i < chars.len() {
        match chars[i] {
            '.' => {
                i += 1;
                if matches!(chars.get(i), None | Some('.') | Some('[')) {
                    return Err("empty name in the path".into());
                }
            }
            '[' => {
                let (seg, next) = bracket(&chars, i)?;
                segs.push(seg);
                i = next;
            }
            _ => {
                let start = i;
                while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
                    i += 1;
                }
                let key: String = chars[start..i].iter().collect();
                if key.is_empty() {
                    return Err("empty name in the path".into());
                }
                segs.push(if key == "*" { Seg::Each } else { Seg::Key(key) });
            }
        }
    }
    Ok(segs)
}

fn bracket(chars: &[char], open: usize) -> Result<(Seg, usize), String> {
    let mut i = open + 1;
    match chars.get(i) {
        Some('"') | Some('\'') => {
            let quote = chars[i];
            i += 1;
            let mut key = String::new();
            while i < chars.len() && chars[i] != quote {
                key.push(chars[i]);
                i += 1;
            }
            if chars.get(i) != Some(&quote) || chars.get(i + 1) != Some(&']') {
                return Err("a quoted name in [] must end with the quote and ]".into());
            }
            Ok((Seg::Key(key), i + 2))
        }
        Some('*') => {
            if chars.get(i + 1) != Some(&']') {
                return Err("expected ] after *".into());
            }
            Ok((Seg::Each, i + 2))
        }
        Some(c) if c.is_ascii_digit() => {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if chars.get(i) != Some(&']') {
                return Err("expected ] after the number".into());
            }
            let n: String = chars[start..i].iter().collect();
            let n = n
                .parse::<usize>()
                .map_err(|_| "index too large".to_string())?;
            Ok((Seg::Index(n), i + 1))
        }
        _ => Err("expected a number, * or a quoted name inside []".into()),
    }
}

/// Every value the path selects, in document order.
pub fn select<'a>(root: &'a Value, segs: &[Seg]) -> Vec<&'a Value> {
    let mut current = vec![root];
    for seg in segs {
        let mut next = Vec::new();
        for v in current {
            match (seg, v) {
                (Seg::Key(k), Value::Object(m)) => next.extend(m.get(k)),
                (Seg::Index(n), Value::Array(a)) => next.extend(a.get(*n)),
                (Seg::Each, Value::Array(a)) => next.extend(a.iter()),
                (Seg::Each, Value::Object(m)) => next.extend(m.values()),
                _ => {}
            }
        }
        current = next;
    }
    current
}

/// A value as the text people expect: strings as they are, scalars written out, `null` as
/// nothing, and objects or lists as compact JSON.
pub fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(doc: &Value, path: &str) -> Vec<String> {
        select(doc, &parse(path).unwrap())
            .into_iter()
            .filter_map(text_of)
            .collect()
    }

    #[test]
    fn dotted_and_dollar_paths_agree() {
        let d = json!({"build": {"number": 42}});
        assert_eq!(run(&d, "build.number"), ["42"]);
        assert_eq!(run(&d, "$.build.number"), ["42"]);
    }

    #[test]
    fn indexes_wildcards_and_quoted_keys() {
        let d = json!({"items": [{"id": "a"}, {"id": "b"}], "dotted.key": {"x": true}});
        assert_eq!(run(&d, "items[1].id"), ["b"]);
        assert_eq!(run(&d, "items[*].id"), ["a", "b"]);
        assert_eq!(run(&d, "items.*.id"), ["a", "b"]);
        assert_eq!(run(&d, r#"["dotted.key"].x"#), ["true"]);
    }

    #[test]
    fn missing_things_select_nothing() {
        let d = json!({"a": [1]});
        assert!(run(&d, "a[5]").is_empty());
        assert!(run(&d, "b").is_empty());
        assert!(run(&d, "a.b").is_empty());
        assert!(run(&json!({"a": null}), "a").is_empty());
    }

    #[test]
    fn objects_and_lists_become_compact_json() {
        let d = json!({"a": {"b": [1, 2]}});
        assert_eq!(run(&d, "a"), [r#"{"b":[1,2]}"#]);
    }

    #[test]
    fn the_empty_path_is_the_whole_document() {
        assert_eq!(run(&json!("hi"), ""), ["hi"]);
        assert_eq!(run(&json!("hi"), "$"), ["hi"]);
    }

    #[test]
    fn malformed_paths_say_why() {
        assert!(parse("a[").is_err());
        assert!(parse("a[x]").is_err());
        assert!(parse("a..b").is_err());
        assert!(parse(r#"a["b]"#).is_err());
        assert!(parse("a[99999999999999999999999]").is_err());
    }
}
