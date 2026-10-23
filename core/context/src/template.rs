//! `{{name}}` substitution for text blocks, prompts and info-type values.
//!
//! * Names use letters, digits, `_`, `-` and `.` (for example `{{rule.build_number}}`).
//!   Spaces around the name are ignored: `{{ rule.x }}`.
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

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn tokens(t: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;
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
                    if !text.is_empty() {
                        out.push(Token::Text(std::mem::take(&mut text)));
                    }
                    out.push(Token::Var { name, raw });
                    i += raw.len();
                }
                None => {
                    text.push_str("{{");
                    i += 2;
                }
            }
        } else {
            let ch = rest.chars().next().unwrap_or_default();
            text.push(ch);
            i += ch.len_utf8().max(1);
        }
    }
    if !text.is_empty() {
        out.push(Token::Text(text));
    }
    out
}

/// Names used by `template`, in order of first appearance, without duplicates.
pub fn variables(template: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for token in tokens(template) {
        if let Token::Var { name, .. } = token {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Replace every variable using `lookup`.
pub fn render<F>(template: &str, lookup: F) -> Rendered
where
    F: Fn(&str) -> Option<String>,
{
    let mut text = String::with_capacity(template.len());
    let mut unresolved: Vec<String> = Vec::new();
    for token in tokens(template) {
        match token {
            Token::Text(s) => text.push_str(&s),
            Token::Var { name, raw } => match lookup(name) {
                Some(value) => text.push_str(&value),
                None => {
                    text.push_str(raw);
                    if !unresolved.iter().any(|n| n == name) {
                        unresolved.push(name.to_string());
                    }
                }
            },
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
}
