//! Glob matching and the read-only allow-list for files CargoFrame may read.
//!
//! Data rules and the Files block only ever read paths that match an allow-list pattern the
//! user wrote. Patterns use `/` as the separator (`\` in input is converted):
//!
//! * `?` matches one character except `/`
//! * `*` matches any run of characters except `/`
//! * `**` matches any run of characters including `/`
//! * `**/` matches zero or more whole folders
//!
//! Matching is a table walk, so a hostile pattern cannot take exponential time.

#[derive(Clone, Copy)]
enum Tok {
    Lit(char),
    One,
    Star,
    Deep,
    DeepSlash,
}

fn normalize(s: &str) -> String {
    s.replace('\\', "/")
}

fn compile(pattern: &str) -> Vec<Tok> {
    let chars: Vec<char> = normalize(pattern).chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if chars.get(i + 1) == Some(&'*') => {
                if chars.get(i + 2) == Some(&'/') {
                    toks.push(Tok::DeepSlash);
                    i += 3;
                } else {
                    toks.push(Tok::Deep);
                    i += 2;
                }
                // Further stars add nothing: treat `***` like `**`.
                while chars.get(i) == Some(&'*') {
                    i += 1;
                }
            }
            '*' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '?' => {
                toks.push(Tok::One);
                i += 1;
            }
            c => {
                toks.push(Tok::Lit(c));
                i += 1;
            }
        }
    }
    toks
}

fn same(a: char, b: char, ignore_case: bool) -> bool {
    a == b || (ignore_case && a.to_lowercase().eq(b.to_lowercase()))
}

/// Does `text` match `pattern`? Both may use `\` or `/`.
pub fn matches(pattern: &str, text: &str, ignore_case: bool) -> bool {
    let toks = compile(pattern);
    let text: Vec<char> = normalize(text).chars().collect();
    let (n, m) = (toks.len(), text.len());

    // dp[p][t] = does toks[p..] match text[t..]
    let mut dp = vec![vec![false; m + 1]; n + 1];
    dp[n][m] = true;
    for p in (0..n).rev() {
        for t in (0..=m).rev() {
            dp[p][t] = match toks[p] {
                Tok::Lit(c) => t < m && same(c, text[t], ignore_case) && dp[p + 1][t + 1],
                Tok::One => t < m && text[t] != '/' && dp[p + 1][t + 1],
                Tok::Star => dp[p + 1][t] || (t < m && text[t] != '/' && dp[p][t + 1]),
                Tok::Deep => dp[p + 1][t] || (t < m && dp[p][t + 1]),
                Tok::DeepSlash => {
                    dp[p + 1][t]
                        || (t < m && ((text[t] == '/' && dp[p + 1][t + 1]) || dp[p][t + 1]))
                }
            };
        }
    }
    dp[0][0]
}

/// Paths CargoFrame is allowed to read.
#[derive(Debug, Clone, Default)]
pub struct AllowList {
    patterns: Vec<String>,
    ignore_case: bool,
}

impl AllowList {
    /// `ignore_case` should be true on Windows.
    pub fn new<I, S>(patterns: I, ignore_case: bool) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            patterns: patterns.into_iter().map(Into::into).collect(),
            ignore_case,
        }
    }

    /// True when `path` matches a pattern and contains no `..` segment or NUL byte.
    pub fn allows(&self, path: &str) -> bool {
        if path.is_empty() || path.contains('\0') {
            return false;
        }
        if normalize(path).split('/').any(|seg| seg == "..") {
            return false;
        }
        self.patterns
            .iter()
            .any(|p| matches(p, path, self.ignore_case))
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_stays_inside_a_folder() {
        assert!(matches("logs/*.txt", "logs/a.txt", false));
        assert!(!matches("logs/*.txt", "logs/sub/a.txt", false));
        assert!(matches("*", "", false));
    }

    #[test]
    fn deep_star_crosses_folders() {
        assert!(matches("logs/**", "logs/a/b/c.txt", false));
        assert!(matches("logs/**.txt", "logs/a/b/c.txt", false));
        assert!(!matches("logs/**.txt", "logs/a/b/c.log", false));
    }

    #[test]
    fn deep_slash_matches_zero_or_more_folders() {
        assert!(matches("src/**/mod.rs", "src/mod.rs", false));
        assert!(matches("src/**/mod.rs", "src/a/mod.rs", false));
        assert!(matches("src/**/mod.rs", "src/a/b/mod.rs", false));
        assert!(!matches("src/**/mod.rs", "src/a/b/lib.rs", false));
        assert!(matches("**/Cargo.toml", "Cargo.toml", false));
        assert!(matches("**/Cargo.toml", "core/x/Cargo.toml", false));
    }

    #[test]
    fn question_mark_matches_one_character() {
        assert!(matches("a?c", "abc", false));
        assert!(!matches("a?c", "a/c", false));
        assert!(!matches("a?c", "ac", false));
    }

    #[test]
    fn backslashes_are_converted() {
        assert!(matches(
            r"C:\Dev\Foo\**\*.json",
            r"C:\Dev\Foo\cfg\a.json",
            true
        ));
        assert!(matches("C:/Dev/Foo/*.json", r"C:\Dev\Foo\a.json", true));
    }

    #[test]
    fn case_rule() {
        assert!(matches("C:/Dev/*.TXT", "c:/dev/a.txt", true));
        assert!(!matches("C:/Dev/*.TXT", "c:/dev/a.txt", false));
    }

    #[test]
    fn hostile_pattern_finishes() {
        let pattern = "*a".repeat(40) + "b";
        let text = "a".repeat(200);
        assert!(!matches(&pattern, &text, false));
    }

    #[test]
    fn allow_list_blocks_traversal_and_unlisted_paths() {
        let list = AllowList::new(["C:/Dev/Foo/**"], true);
        assert!(list.allows(r"C:\Dev\Foo\config\app.json"));
        assert!(!list.allows(r"C:\Dev\Foo\..\Secret\key.txt"));
        assert!(!list.allows(r"C:\Dev\Other\app.json"));
        assert!(!list.allows(""));
        assert!(!list.allows("C:/Dev/Foo/a\0b"));
    }

    #[test]
    fn empty_allow_list_allows_nothing() {
        let list = AllowList::default();
        assert!(list.is_empty());
        assert!(!list.allows("/etc/passwd"));
    }
}
