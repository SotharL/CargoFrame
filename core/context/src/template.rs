//! `{name}` and `{{name}}` substitution for text blocks, prompts, tags and info-type values.
//!
//! * Names use letters, digits, `_`, `-` and `.` (for example `{{rule.build_number}}`).
//!   Spaces around the name are ignored inside double braces: `{{ rule.x }}`.
//! * The short form `{name}` (0.5.0) works the same: `{gpu}`, `{app.exe}`. So that braces written
//!   for other reasons stay as they are, it takes no spaces, no name made only of digits (`{0}`),
//!   none of the names Jira's formatting uses (`{code}`, `{quote}`…) and, with a dot, only the
//!   capture's own groups (`app.`, `project.`, `capture.`, `rule.`). [`render_strict`] reads only
//!   the double form, for texts where single braces mean something else (a custom API's body).
//! * A name without a dot is a value read from files: `{gpu}` and `{{gpu}}` are `{{rule.gpu}}`.
//! * `\{{` produces a literal `{{`.
//! * Anything that does not look like a variable (`{{}}`, `{{a b}}`, an unclosed `{{`) stays
//!   as plain text.
//! * A name the lookup cannot resolve is left in the output as written and reported in
//!   [`Rendered::unresolved`], so the window can show it with the debug colour.
//! * Substituted values are never expanded again, so a value that contains `{{x}}` cannot
//!   pull in another variable.

/// Result of [`render`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    /// Names that could not be resolved, in order of first appearance, without duplicates.
    pub unresolved: Vec<String>,
}

enum Token<'a> {
    Text(String),
    Var { name: &'a str, raw: &'a str },
}

/// Names Jira's own formatting writes in single braces: never values.
pub const RESERVED: [&str; 9] = [
    "code", "noformat", "quote", "color", "panel", "anchor", "toc", "section", "column",
];

/// The groups a dotted short name may belong to.
const GROUPS: [&str; 4] = ["app.", "project.", "capture.", "rule."];

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Whether `{name}` is meant as a value (see the module's notes).
pub fn short_name_ok(name: &str) -> bool {
    valid_name(name)
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.chars().all(|c| c.is_ascii_digit())
        && !RESERVED.contains(&name.to_ascii_lowercase().as_str())
        && (!name.contains('.')
            || GROUPS
                .iter()
                .any(|g| name.starts_with(g) && name.len() > g.len()))
}

/// The key of a value read from files that `name` stands for: `rule.gpu` and `gpu` are `gpu`.
pub fn rule_key(name: &str) -> Option<&str> {
    match name.strip_prefix("rule.") {
        Some(key) => Some(key),
        None if !name.contains('.') => Some(name),
        None => None,
    }
}

fn tokens(t: &str, short: bool) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    let push = |out: &mut Vec<Token<'_>>, text: &mut String| {
        if !text.is_empty() {
            out.push(Token::Text(std::mem::take(text)));
        }
    };
    while i < t.len() {
        let rest = &t[i..];
        if rest.starts_with("\\{{") {
            text.push_str("{{");
            i += 3;
        } else if let Some(after) = rest.strip_prefix("{{") {
            let var = after.find("}}").and_then(|end| {
                let name = after[..end].trim();
                valid_name(name).then_some((name, &rest[..end + 4]))
            });
            match var {
                Some((name, raw)) => {
                    push(&mut out, &mut text);
                    out.push(Token::Var { name, raw });
                    i += raw.len();
                }
                None => {
                    text.push_str("{{");
                    i += 2;
                }
            }
        } else if let Some(after) = rest.strip_prefix('{').filter(|_| short) {
            let var = after
                .find('}')
                .filter(|end| *end <= 64)
                .map(|end| &after[..end])
                .filter(|name| short_name_ok(name));
            match var {
                Some(name) => {
                    push(&mut out, &mut text);
                    out.push(Token::Var {
                        name,
                        raw: &rest[..name.len() + 2],
                    });
                    i += name.len() + 2;
                }
                None => {
                    text.push('{');
                    i += 1;
                }
            }
        } else {
            let ch = rest.chars().next().unwrap_or_default();
            text.push(ch);
            i += ch.len_utf8().max(1);
        }
    }
    push(&mut out, &mut text);
    out
}

/// Names used by `template` (in either form, as written), in order of first appearance, without
/// duplicates.
pub fn variables(template: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for token in tokens(template, true) {
        if let Token::Var { name, .. } = token {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Replace every variable using `lookup`. A name without a dot is also looked up as a value read
/// from files (`gpu` as `rule.gpu`).
pub fn render<F>(template: &str, lookup: F) -> Rendered
where
    F: Fn(&str) -> Option<String>,
{
    render_with(tokens(template, true), lookup)
}

/// [`render`] reading only `{{name}}`: single braces stay as written.
pub fn render_strict<F>(template: &str, lookup: F) -> Rendered
where
    F: Fn(&str) -> Option<String>,
{
    render_with(tokens(template, false), lookup)
}

fn render_with<F>(tokens: Vec<Token<'_>>, lookup: F) -> Rendered
where
    F: Fn(&str) -> Option<String>,
{
    let mut text = String::new();
    let mut unresolved: Vec<String> = Vec::new();
    for token in tokens {
        match token {
            Token::Text(s) => text.push_str(&s),
            Token::Var { name, raw } => {
                let found = lookup(name).or_else(|| {
                    (!name.contains('.'))
                        .then(|| lookup(&format!("rule.{name}")))
                        .flatten()
                });
                match found {
                    Some(value) => text.push_str(&value),
                    None => {
                        text.push_str(raw);
                        if !unresolved.iter().any(|n| n == name) {
                            unresolved.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    Rendered { text, unresolved }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        match name {
            "rule.build" => Some("1042".into()),
            "who" => Some("Ann".into()),
            "tricky" => Some("{{who}}".into()),
            _ => None,
        }
    }

    #[test]
    fn replaces_variables() {
        let r = render("Build {{rule.build}} by {{ who }}", lookup);
        assert_eq!(r.text, "Build 1042 by Ann");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn reports_and_keeps_unresolved() {
        let r = render("{{nope}} and {{ nope }} and {{who}}", lookup);
        assert_eq!(r.text, "{{nope}} and {{ nope }} and Ann");
        assert_eq!(r.unresolved, vec!["nope".to_string()]);
    }

    #[test]
    fn values_are_not_expanded_again() {
        let r = render("{{tricky}}", lookup);
        assert_eq!(r.text, "{{who}}");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn backslash_escapes_the_opening_braces() {
        let r = render(r"literal \{{who}} here", lookup);
        assert_eq!(r.text, "literal {{who}} here");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn malformed_braces_stay_text() {
        let r = render("a {{}} b {{x y}} c {{who", lookup);
        assert_eq!(r.text, "a {{}} b {{x y}} c {{who");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn handles_non_ascii_text() {
        let r = render("héllo {{who}} — ✓ {{missing}}", lookup);
        assert_eq!(r.text, "héllo Ann — ✓ {{missing}}");
        assert_eq!(r.unresolved, vec!["missing".to_string()]);
    }

    #[test]
    fn variables_lists_unique_names_in_order() {
        assert_eq!(
            variables("{{b}} {{a}} {{b}} \\{{c}} {{ d.e }}"),
            vec!["b", "a", "d.e"]
        );
    }

    #[test]
    fn empty_and_plain_input() {
        assert_eq!(render("", lookup).text, "");
        assert_eq!(render("no variables", lookup).text, "no variables");
    }

    #[test]
    fn adjacent_variables() {
        assert_eq!(render("{{who}}{{who}}", lookup).text, "AnnAnn");
    }

    #[test]
    fn the_short_form_reads_the_same_values() {
        let r = render(
            "Build {build} on {app.exe} by {who}, {{build}}",
            |n| match n {
                "rule.build" => Some("1042".into()),
                "app.exe" => Some("Game.exe".into()),
                "who" => Some("Ann".into()),
                _ => None,
            },
        );
        assert_eq!(r.text, "Build 1042 on Game.exe by Ann, 1042");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn braces_written_for_other_reasons_stay_as_they_are() {
        // Jira's formatting, JSON, numbered placeholders, spaces, other dotted names
        let text = "{code}x{code} {color:red}a{color} {\"a\": 1} {0} {a b} {file.txt} {} {";
        let r = render(text, |_| Some("!".into()));
        assert_eq!(r.text, text);
        assert!(r.unresolved.is_empty());
        // a short name nothing fills stays as written and is reported, like the long form
        let r = render("on {map} {{gone}}", |_| None);
        assert_eq!(r.text, "on {map} {{gone}}");
        assert_eq!(r.unresolved, vec!["map".to_string(), "gone".to_string()]);
        // the strict form reads only double braces
        let r = render_strict("{\"t\": \"{{who}}\", \"id\": \"{id}\"}", lookup);
        assert_eq!(r.text, "{\"t\": \"Ann\", \"id\": \"{id}\"}");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn short_names_and_rule_keys() {
        assert!(short_name_ok("gpu") && short_name_ok("app.exe") && short_name_ok("rule.x"));
        assert!(
            !short_name_ok("Code")
                && !short_name_ok("12")
                && !short_name_ok("a.b")
                && !short_name_ok("rule.")
        );
        assert_eq!(rule_key("rule.gpu"), Some("gpu"));
        assert_eq!(rule_key("gpu"), Some("gpu"));
        assert_eq!(rule_key("app.exe"), None);
        assert_eq!(variables("{a} {{b}} {a} {code}"), vec!["a", "b"]);
    }
}
