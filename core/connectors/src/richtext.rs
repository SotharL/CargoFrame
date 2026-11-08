//! Text written once, shown right on every service.
//!
//! A text block can carry a little formatting, written the way most people already know:
//!
//! | Written | Means |
//! |---------|-------|
//! | `**bold**` | bold |
//! | `*italic*` | italic |
//! | `__underline__` | underline |
//! | `` `code` `` | code |
//! | `> text` | a quote (a line) |
//! | `- text` | a bullet (a line) |
//! | `1. text` | a numbered item (a line) |
//!
//! Each service has its own way of writing these (Jira's wiki, Slack's `_italic_`, Teams' HTML),
//! and some cannot show some of them (Slack and Reddit have no underline): [`render`] writes
//! the text the way one service wants it, and leaves out what it cannot show. Text without any of
//! these marks comes out as it went in.
//!
//! A mark counts only at the edge of a word, so `2*3*4` and `snake__case__name` stay as written, and
//! one that is never closed stays as it was typed.

/// The services that take formatted text, and how each writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Discord's markdown, which is the way of writing above.
    Discord,
    /// Reddit's markdown: no underline, and a list needs an empty line before it.
    Reddit,
    /// Slack's `mrkdwn`: `*bold*`, `_italic_`, no underline, text escaped.
    Slack,
    /// Jira's wiki markup: `*bold*`, `_italic_`, `+underline+`, `{{code}}`.
    Jira,
    /// Teams' HTML, with every piece of text escaped.
    Html,
    /// No formatting at all (X, Bluesky, Instagram): the marks go, the words stay, a bullet is "• ".
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Span {
    Text(String),
    Bold(Vec<Span>),
    Italic(Vec<Span>),
    Underline(Vec<Span>),
    Code(String),
}

/// A mark is looked for this far ahead and no further, so a long text full of lone marks is read in
/// a moment: nobody bolds more than this in one go.
const REACH: usize = 2000;

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// Where the next backtick after `from` is, within reach.
fn next_tick(chars: &[char], from: usize) -> Option<usize> {
    chars[from.min(chars.len())..]
        .iter()
        .take(REACH)
        .position(|c| *c == '`')
        .map(|p| from + p)
}

/// Where the mark `marker` closes, from `from`, skipping what is inside code. The closer is not
/// preceded by a space and not followed by a letter or digit; a single `*` is not half of `**`.
fn find_close(chars: &[char], from: usize, marker: &[char]) -> Option<usize> {
    let mut i = from;
    while i < chars.len() && i - from <= REACH {
        if chars[i] == '`' {
            // a code span hides what it holds
            i = next_tick(chars, i + 1)? + 1;
            continue;
        }
        if chars[i..].starts_with(marker) && i > from {
            let after = chars.get(i + marker.len()).copied();
            let before = chars[i - 1];
            let part_of_longer =
                marker.len() == 1 && (after == Some(marker[0]) || before == marker[0]);
            if !part_of_longer && !before.is_whitespace() && !after.is_some_and(is_word) {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn spans(text: &str) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    parse(&chars)
}

fn parse(chars: &[char]) -> Vec<Span> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span::Text(std::mem::take(plain)));
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let at_edge = i == 0 || !is_word(chars[i - 1]);
        let next = chars.get(i + 1).copied();
        if c == '`' {
            if let Some(close) = next_tick(chars, i + 1) {
                if close > i + 1 {
                    flush(&mut plain, &mut out);
                    out.push(Span::Code(chars[i + 1..close].iter().collect()));
                    i = close + 1;
                    continue;
                }
            }
        } else if at_edge && (c == '*' || c == '_') && next == Some(c) {
            // `**` bold, `__` underline
            let marker = [c, c];
            let inner = i + 2;
            if chars.get(inner).is_some_and(|x| !x.is_whitespace()) {
                if let Some(end) = find_close(chars, inner, &marker) {
                    flush(&mut plain, &mut out);
                    let body = parse(&chars[inner..end]);
                    out.push(if c == '*' {
                        Span::Bold(body)
                    } else {
                        Span::Underline(body)
                    });
                    i = end + 2;
                    continue;
                }
            }
        } else if at_edge && c == '*' && next.is_some_and(|x| !x.is_whitespace() && x != '*') {
            if let Some(end) = find_close(chars, i + 1, &['*']) {
                flush(&mut plain, &mut out);
                out.push(Span::Italic(parse(&chars[i + 1..end])));
                i = end + 1;
                continue;
            }
        }
        plain.push(c);
        i += 1;
    }
    flush(&mut plain, &mut out);
    out
}

fn escape_slack(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn inline(spans: &[Span], style: Style) -> String {
    let mut out = String::new();
    for span in spans {
        match span {
            Span::Text(t) => out.push_str(&match style {
                Style::Slack => escape_slack(t),
                Style::Html => escape_html(t),
                _ => t.clone(),
            }),
            Span::Bold(inner) => {
                let body = inline(inner, style);
                out.push_str(&match style {
                    Style::Discord | Style::Reddit => format!("**{body}**"),
                    Style::Slack | Style::Jira => format!("*{body}*"),
                    Style::Html => format!("<b>{body}</b>"),
                    Style::Plain => body,
                });
            }
            Span::Italic(inner) => {
                let body = inline(inner, style);
                out.push_str(&match style {
                    Style::Discord | Style::Reddit => format!("*{body}*"),
                    Style::Slack | Style::Jira => format!("_{body}_"),
                    Style::Html => format!("<i>{body}</i>"),
                    Style::Plain => body,
                });
            }
            Span::Underline(inner) => {
                let body = inline(inner, style);
                out.push_str(&match style {
                    Style::Discord => format!("__{body}__"),
                    Style::Jira => format!("+{body}+"),
                    Style::Html => format!("<u>{body}</u>"),
                    // no underline on these: the words stay, the mark goes
                    Style::Reddit | Style::Slack | Style::Plain => body,
                });
            }
            Span::Code(code) => out.push_str(&match style {
                Style::Discord | Style::Reddit | Style::Slack => format!("`{code}`"),
                Style::Jira => format!("{{{{{code}}}}}"),
                Style::Html => format!("<code>{}</code>", escape_html(code)),
                Style::Plain => code.clone(),
            }),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Quote,
    Bullet,
    Number,
}

/// What a line is, and what is left of it without the mark at its start.
fn classify(line: &str) -> (Kind, &str) {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("> ") {
        return (Kind::Quote, rest);
    }
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        if !rest.trim().is_empty() {
            return (Kind::Bullet, rest);
        }
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if (1..=9).contains(&digits) {
        if let Some(rest) = t[digits..]
            .strip_prefix(". ")
            .or_else(|| t[digits..].strip_prefix(") "))
        {
            if !rest.trim().is_empty() {
                return (Kind::Number, rest);
            }
        }
    }
    (Kind::Plain, line)
}

/// Write `text` the way `style` wants it.
pub fn render(text: &str, style: Style) -> String {
    let lines: Vec<(Kind, String)> = text
        .trim_end()
        .lines()
        .map(|l| {
            let (kind, rest) = classify(l.trim_end());
            (kind, inline(&spans(rest), style))
        })
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let kind = lines[i].0;
        // a run of lines of one kind
        let end = lines[i..]
            .iter()
            .position(|(k, _)| *k != kind)
            .map_or(lines.len(), |p| i + p);
        let run = &lines[i..end];
        let block_starts = kind != Kind::Plain;
        if style == Style::Reddit && block_starts && out.last().is_some_and(|l| !l.is_empty()) {
            out.push(String::new());
        }
        match (style, kind) {
            (Style::Html, Kind::Plain) => {
                let body: Vec<&str> = run.iter().map(|(_, l)| l.as_str()).collect();
                out.push(format!("<p>{}</p>", body.join("<br>")));
            }
            (Style::Html, Kind::Quote) => {
                let body: Vec<&str> = run.iter().map(|(_, l)| l.as_str()).collect();
                out.push(format!("<blockquote>{}</blockquote>", body.join("<br>")));
            }
            (Style::Html, Kind::Bullet | Kind::Number) => {
                let tag = if kind == Kind::Bullet { "ul" } else { "ol" };
                let items: String = run.iter().map(|(_, l)| format!("<li>{l}</li>")).collect();
                out.push(format!("<{tag}>{items}</{tag}>"));
            }
            (Style::Jira, Kind::Quote) => {
                out.push("{quote}".into());
                out.extend(run.iter().map(|(_, l)| l.clone()));
                out.push("{quote}".into());
            }
            (_, Kind::Plain) => out.extend(run.iter().map(|(_, l)| l.clone())),
            (Style::Plain, Kind::Quote) => out.extend(run.iter().map(|(_, l)| format!("“{l}”"))),
            (_, Kind::Quote) => out.extend(run.iter().map(|(_, l)| format!("> {l}"))),
            (style, Kind::Bullet) => out.extend(run.iter().map(|(_, l)| match style {
                Style::Slack | Style::Plain => format!("• {l}"),
                Style::Jira => format!("* {l}"),
                _ => format!("- {l}"),
            })),
            (style, Kind::Number) => {
                out.extend(run.iter().enumerate().map(|(n, (_, l))| match style {
                    Style::Jira => format!("# {l}"),
                    _ => format!("{}. {l}", n + 1),
                }))
            }
        }
        i = end;
    }
    out.join(if style == Style::Html { "" } else { "\n" })
}

#[cfg(test)]
#[path = "richtext_tests.rs"]
mod tests;
