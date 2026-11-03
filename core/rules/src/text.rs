//! Plain text and log files (`.txt`, `.log`, and anything else that is just lines).
//!
//! The query is a short instruction, `what: argument`, so no regular expression is needed for
//! the common jobs:
//!
//! | Query | Gives |
//! |-------|-------|
//! | `line: 3` / `line: -1` | line 3, or the last line (`-2` is the one before) |
//! | `lines: 5-9` | lines 5 to 9 |
//! | `head: 20` / `tail: 20` | the first or the last 20 lines |
//! | `contains: timeout` | every line containing the text (any case) |
//! | `after: Version:` | what follows the text, on every line that has it |
//! | `between: build= \| ;` | what sits between two texts (here `build=` and `;`), even across lines |
//! | `level: error, warn` | every line that has one of these words (any case, whole word) |
//!
//! Colour codes that programs write into logs are removed from what comes back. `multiple`
//! decides what happens when several lines match (first, last, or all joined).

use crate::model::ReadMode;
use regex::{Regex, RegexBuilder};

/// Most values any query may ask for. The same limit as every other format.
pub const MAX_LINES: usize = 1000;
const MAX_NEEDLE: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// 1-based from the top, or negative from the bottom.
    Line(i64),
    /// 1-based, inclusive.
    Lines(usize, usize),
    Head(usize),
    Tail(usize),
    Contains(String),
    After(String),
    /// What is between the first text and the next occurrence of the second, every time.
    Between(String, String),
    Level(Vec<String>),
}

const HELP: &str =
    "start with line:, lines:, head:, tail:, contains:, after:, between: or level:, \
                    for example tail: 20 or after: Version:";

pub fn parse(query: &str) -> Result<Query, String> {
    let query = query.trim();
    let Some((key, arg)) = query.split_once(':') else {
        return Err(HELP.into());
    };
    let arg = arg.trim();
    match key.trim().to_ascii_lowercase().as_str() {
        "line" => {
            let n: i64 = arg
                .parse()
                .map_err(|_| "line: needs a number, such as line: 3 or line: -1".to_string())?;
            if n == 0 || n.unsigned_abs() > 10_000_000 {
                return Err("line numbers start at 1; use -1 for the last line".into());
            }
            Ok(Query::Line(n))
        }
        "lines" => {
            let (a, b) = arg
                .split_once('-')
                .ok_or_else(|| "lines: needs a range, such as lines: 5-9".to_string())?;
            let (a, b) = (count(a, "lines:")?, count(b, "lines:")?);
            if b < a {
                return Err("the range must go upward, such as lines: 5-9".into());
            }
            if b - a + 1 > MAX_LINES {
                return Err(format!("a range can hold at most {MAX_LINES} lines"));
            }
            Ok(Query::Lines(a, b))
        }
        "head" => Ok(Query::Head(limited(arg, "head:")?)),
        "tail" => Ok(Query::Tail(limited(arg, "tail:")?)),
        "contains" => Ok(Query::Contains(needle(arg, "contains:")?)),
        "after" => Ok(Query::After(needle(arg, "after:")?)),
        "between" => {
            // The two texts are told apart by a bar: between: build= | ;
            let (start, end) = arg.split_once('|').ok_or_else(|| {
                "between: needs two texts separated by a bar, such as between: build= | ;"
                    .to_string()
            })?;
            Ok(Query::Between(
                needle(start.trim(), "between: the first text")?,
                needle(end.trim(), "between: the second text")?,
            ))
        }
        "level" => {
            let words: Vec<String> = arg
                .split(',')
                .map(str::trim)
                .filter(|w| !w.is_empty())
                .map(str::to_string)
                .collect();
            if words.is_empty() || words.len() > 10 {
                return Err("level: needs one to ten words, such as level: error, warn".into());
            }
            if words.iter().any(|w| w.contains(char::is_whitespace)) {
                return Err("each level is one word, separated by commas".into());
            }
            for w in &words {
                needle(w, "level:")?;
            }
            Ok(Query::Level(words))
        }
        _ => Err(HELP.into()),
    }
}

fn count(s: &str, what: &str) -> Result<usize, String> {
    s.trim()
        .parse::<usize>()
        .ok()
        .filter(|n| *n >= 1)
        .ok_or_else(|| format!("{what} needs whole numbers from 1"))
}

fn limited(s: &str, what: &str) -> Result<usize, String> {
    let n = count(s, what)?;
    if n > MAX_LINES {
        return Err(format!("{what} can ask for at most {MAX_LINES} lines"));
    }
    Ok(n)
}

fn needle(s: &str, what: &str) -> Result<String, String> {
    if s.is_empty() {
        return Err(format!("{what} needs some text to look for"));
    }
    if s.chars().count() > MAX_NEEDLE {
        return Err(format!("{what} text is limited to {MAX_NEEDLE} characters"));
    }
    Ok(s.to_string())
}

impl Query {
    /// Which end of a file too big to read whole is worth reading.
    pub fn read_mode(&self) -> ReadMode {
        match self {
            Query::Head(_) | Query::Lines(..) => ReadMode::Head,
            Query::Line(n) if *n > 0 => ReadMode::Head,
            // The end of a log is where its latest entries are.
            _ => ReadMode::Tail,
        }
    }
}

fn matcher(pattern: &str) -> Regex {
    // Escaped text and a size limit: this cannot be a runaway pattern.
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .size_limit(1 << 20)
        .build()
        .expect("an escaped pattern is always valid")
}

/// Colour and cursor codes (`ESC [ … letter`) that programs write into logs.
fn ansi() -> Regex {
    Regex::new("\u{1b}\\[[0-9;?]*[ -/]*[@-~]").expect("a fixed pattern")
}

/// Every value the query finds in `text`, in file order, at most [`MAX_LINES`].
pub fn values(text: &str, query: &Query) -> Vec<String> {
    // Colour codes go first, so they cannot split a word the query is looking for.
    let strip = ansi();
    let cleaned: Vec<String> = text
        .lines()
        .map(|l| strip.replace_all(l, "").trim_end().to_string())
        .collect();
    let lines: Vec<&str> = cleaned.iter().map(String::as_str).collect();
    let keep = |s: &str| (!s.trim().is_empty()).then(|| s.to_string());
    let mut out: Vec<String> = match query {
        Query::Line(n) => {
            let index = if *n > 0 {
                usize::try_from(*n - 1).ok()
            } else {
                lines
                    .len()
                    .checked_sub(usize::try_from(n.unsigned_abs()).unwrap_or(usize::MAX))
            };
            index
                .and_then(|i| lines.get(i))
                .and_then(|l| keep(l))
                .into_iter()
                .collect()
        }
        Query::Lines(a, b) => lines
            .iter()
            .skip(a - 1)
            .take(b - a + 1)
            .map(|l| (*l).to_string())
            .collect(),
        Query::Head(n) => lines.iter().take(*n).map(|l| (*l).to_string()).collect(),
        Query::Tail(n) => lines[lines.len().saturating_sub(*n)..]
            .iter()
            .map(|l| (*l).to_string())
            .collect(),
        Query::Contains(t) => {
            let re = matcher(&regex::escape(t));
            lines
                .iter()
                .filter(|l| re.is_match(l))
                .filter_map(|l| keep(l))
                .collect()
        }
        Query::After(t) => {
            let re = matcher(&regex::escape(t));
            lines
                .iter()
                .filter_map(|l| {
                    let m = re.find(l)?;
                    let rest = l[m.end()..].trim_start_matches([' ', '\t']);
                    let rest = rest.strip_prefix([':', '=']).unwrap_or(rest);
                    keep(rest.trim())
                })
                .map(|v| v.trim().to_string())
                .collect()
        }
        Query::Between(start, end) => {
            // Across line ends too, so a block like BEGIN ... END can be read whole.
            let re = RegexBuilder::new(&format!(
                "(?s){}(.*?){}",
                regex::escape(start),
                regex::escape(end)
            ))
            .case_insensitive(true)
            .size_limit(1 << 20)
            .build()
            .expect("an escaped pattern is always valid");
            let whole = lines.join("\n");
            re.captures_iter(&whole)
                .filter_map(|c| keep(c.get(1)?.as_str().trim()))
                .collect()
        }
        Query::Level(words) => {
            let alternatives: Vec<String> = words.iter().map(|w| regex::escape(w)).collect();
            let re = matcher(&format!(r"\b(?:{})\b", alternatives.join("|")));
            lines
                .iter()
                .filter(|l| re.is_match(l))
                .filter_map(|l| keep(l))
                .collect()
        }
    };
    out.truncate(MAX_LINES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
2026-10-23 09:00:01 INFO  starting
2026-10-23 09:00:02 WARN  disk almost full
2026-10-23 09:00:03 ERROR connect timeout
Version: 1.4.2
2026-10-23 09:00:04 INFO  retrying
2026-10-23 09:00:05 error: gave up
";

    fn run(q: &str, text: &str) -> Vec<String> {
        values(text, &parse(q).unwrap())
    }

    #[test]
    fn single_lines_count_from_either_end() {
        assert_eq!(run("line: 1", LOG), ["2026-10-23 09:00:01 INFO  starting"]);
        assert_eq!(run("line: -1", LOG), ["2026-10-23 09:00:05 error: gave up"]);
        assert_eq!(run("line: -2", LOG), ["2026-10-23 09:00:04 INFO  retrying"]);
        assert!(run("line: 99", LOG).is_empty());
        assert!(run("line: -99", LOG).is_empty());
    }

    #[test]
    fn ranges_head_and_tail() {
        assert_eq!(run("lines: 2-3", LOG).len(), 2);
        assert_eq!(
            run("lines: 2-3", LOG)[1],
            "2026-10-23 09:00:03 ERROR connect timeout"
        );
        assert_eq!(run("head: 2", LOG).len(), 2);
        let tail = run("tail: 2", LOG);
        assert_eq!(
            tail,
            [
                "2026-10-23 09:00:04 INFO  retrying",
                "2026-10-23 09:00:05 error: gave up"
            ]
        );
        assert_eq!(
            run("tail: 500", LOG).len(),
            6,
            "asking for more than there is gives it all"
        );
    }

    #[test]
    fn contains_ignores_case_and_after_gives_what_follows() {
        assert_eq!(
            run("contains: TIMEOUT", LOG),
            ["2026-10-23 09:00:03 ERROR connect timeout"]
        );
        assert_eq!(run("after: version:", LOG), ["1.4.2"]);
        assert_eq!(run("after: ERROR", LOG), ["connect timeout", "gave up"]);
        assert_eq!(
            run("AFTER: retrying", LOG).len(),
            0,
            "nothing follows it, so no value"
        );
    }

    #[test]
    fn levels_match_whole_words_in_any_case() {
        assert_eq!(run("level: error", LOG).len(), 2);
        assert_eq!(run("level: warn, error", LOG).len(), 3);
        // "warning" is another word: say so if both are wanted.
        assert!(run("level: warn", "a WARNING here\n").is_empty());
        assert_eq!(run("level: warn, warning", "a WARNING here\n").len(), 1);
    }

    #[test]
    fn colour_codes_and_line_endings_are_removed() {
        let text = "\u{1b}[31mERROR\u{1b}[0m failed\r\nnext\r\n";
        assert_eq!(run("line: 1", text), ["ERROR failed"]);
        assert_eq!(run("after: error", text), ["failed"]);
    }

    #[test]
    fn special_characters_in_the_text_are_plain_text() {
        let text = "cost (est.) = $5 [ok]\nother\n";
        assert_eq!(run("contains: (est.)", text).len(), 1);
        assert_eq!(
            run("after: [ok", text),
            ["]"],
            "the bracket is text, not a pattern"
        );
        assert_eq!(run("after: $5", text), ["[ok]"]);
    }

    #[test]
    fn bad_queries_say_what_to_write() {
        for bad in [
            "",
            "tail",
            "tail: x",
            "tail: 0",
            "tail: 5000",
            "line: 0",
            "line: x",
            "lines: 9-2",
            "lines: 5",
            "lines: 1-5000",
            "contains:",
            "after:   ",
            "level:",
            "level: a b",
            "nope: 3",
        ] {
            let e = parse(bad).unwrap_err();
            assert!(!e.is_empty(), "{bad:?}");
        }
        assert!(parse("contains: ").unwrap_err().contains("text"));
        assert!(parse("what").unwrap_err().contains("tail:"));
    }

    #[test]
    fn big_files_are_read_from_the_end_that_matters() {
        assert_eq!(parse("tail: 3").unwrap().read_mode(), ReadMode::Tail);
        assert_eq!(parse("line: -1").unwrap().read_mode(), ReadMode::Tail);
        assert_eq!(parse("contains: x").unwrap().read_mode(), ReadMode::Tail);
        assert_eq!(parse("level: error").unwrap().read_mode(), ReadMode::Tail);
        assert_eq!(parse("head: 3").unwrap().read_mode(), ReadMode::Head);
        assert_eq!(parse("line: 4").unwrap().read_mode(), ReadMode::Head);
        assert_eq!(parse("lines: 2-4").unwrap().read_mode(), ReadMode::Head);
    }

    #[test]
    fn a_huge_answer_is_cut_to_the_limit() {
        let text = "x\n".repeat(5000);
        assert_eq!(run("contains: x", &text).len(), MAX_LINES);
    }

    #[test]
    fn between_gives_what_sits_between_two_texts() {
        let text = "build=1.2.3; date=today; build=1.2.4;";
        assert_eq!(run("between: build= | ;", text), ["1.2.3", "1.2.4"]);
        assert_eq!(run("between: [ | ]", "x [one] y [two] z"), ["one", "two"]);
    }

    #[test]
    fn between_ignores_case_and_can_span_lines() {
        let block = "noise\nBEGIN\nline a\nline b\nEND\nmore\nbegin\nsecond\nend\n";
        assert_eq!(
            run("between: begin | end", block),
            ["line a\nline b", "second"]
        );
    }

    #[test]
    fn between_skips_empty_gaps_and_unclosed_starts() {
        assert_eq!(run("between: [ | ]", "[] [ ] [ok] [open"), ["ok"]);
        assert!(run("between: [ | ]", "no brackets here").is_empty());
    }

    #[test]
    fn between_keeps_the_texts_literal() {
        // Regular-expression characters in the markers mean themselves.
        assert_eq!(run("between: (( | ))", "a ((b.c)) d"), ["b.c"]);
        assert_eq!(run("between: $ | .", "cost $5.00"), ["5"]);
    }

    #[test]
    fn between_needs_two_texts_separated_by_a_bar() {
        for bad in [
            "between: only one",
            "between: | end",
            "between: start |",
            "between:",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert!(parse("between: a | b").is_ok());
    }
}
