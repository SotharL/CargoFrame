//! Field fills: one field of a service, such as Jira's "Labels", "Priority" or a custom field,
//! and the steps that fill it on every share to that service: "if {{rule.gpu}} is found, add
//! gpu-{{rule.gpu}}". Shown in Settings as *Data & fields → Fill fields*.
//!
//! The person writes it once (`infotypes/<id>.json`, the name from before 0.3.0 when it only
//! described the field); the layout builder also offers the field when a block is sent in it, and
//! the connector knows how to shape the value (its kind, its list).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfoKind {
    #[default]
    Text,
    /// One value from a list.
    Choice,
    /// Several values from a list.
    MultiChoice,
    Number,
    /// On or off.
    Flag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InfoOptions {
    /// Free input (text, number, flag).
    #[default]
    None,
    /// A list the person typed in.
    Fixed { options: Vec<InfoOption> },
    /// A list the connection fetches from its API (`options_id` names the call).
    Api { options_id: String },
}

/// What a step looks at before it acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Test {
    /// Has a value (a value read from a file was found, a box was filled in).
    #[default]
    Found,
    /// Has no value.
    Missing,
    /// Is this text (case does not matter).
    Is,
    IsNot,
    /// Has this text in it (case does not matter).
    Contains,
}

/// "If {{rule.gpu}} is found": `subject` is a variable's name (`rule.gpu`), or empty for the
/// field's own value so far.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct When {
    pub subject: String,
    pub test: Test,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    /// Add to what the field has: one more label, a line at the end of a description.
    #[default]
    Add,
    /// Replace what the field has.
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Action {
    #[serde(rename = "do")]
    pub act: Act,
    /// May use `{{variables}}`.
    pub text: String,
}

/// "If … then add … and add …". No `when`: always.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FillStep {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    pub then: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InfoType {
    /// What the person calls it ("Priority").
    pub name: String,
    /// The connection (`connections/<id>.json`) this field belongs to.
    pub connection_id: String,
    /// The field's key in that service's API (`priority`, `customfield_10042`, `subreddit`).
    pub field: String,
    pub kind: InfoKind,
    pub options: InfoOptions,
    /// The share is blocked until it has a value.
    pub required: bool,
    /// Starting value; may use `{{variables}}`. From before 0.3.0: now a step "if the field is
    /// empty, set it to …".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default_template: String,
    /// What fills the field, in order, on every share to the connection.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<FillStep>,
    /// The layouts it fills the field in; empty: all of them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub layouts: Vec<String>,
}

impl InfoType {
    /// The steps, with a starting value from before 0.3.0 as the first ("if empty, set").
    pub fn all_steps(&self) -> Vec<FillStep> {
        let mut out = Vec::new();
        if !self.default_template.trim().is_empty() {
            out.push(FillStep {
                when: Some(When {
                    subject: String::new(),
                    test: Test::Missing,
                    text: String::new(),
                }),
                then: vec![Action {
                    act: Act::Set,
                    text: self.default_template.clone(),
                }],
            });
        }
        out.extend(self.steps.iter().cloned());
        out
    }

    /// Whether it fills the field by itself, or only describes one a layout sends a block in.
    pub fn fills(&self) -> bool {
        !self.steps.is_empty()
    }

    /// Whether it applies to shares from this layout.
    pub fn applies_in(&self, layout_id: &str) -> bool {
        self.layouts.is_empty() || self.layouts.iter().any(|l| l == layout_id)
    }
}

/// Whether a step's condition holds. `value` is the subject's value ("" when it has none).
pub fn holds(when: &When, value: &str) -> bool {
    let v = value.trim().to_lowercase();
    let t = when.text.trim().to_lowercase();
    match when.test {
        Test::Found => !v.is_empty(),
        Test::Missing => v.is_empty(),
        Test::Is => v == t,
        Test::IsNot => v != t,
        Test::Contains => !t.is_empty() && v.contains(&t),
    }
}

/// The field's value after one action. `list`: a field of several values (labels), where adding
/// is one more value, given once; `lines`: a field of paragraphs (a description), where adding is
/// a new line; otherwise adding goes after a space.
pub fn act(current: &str, action: Act, text: &str, list: bool, lines: bool) -> String {
    let text = text.trim();
    match action {
        Act::Set => text.to_string(),
        Act::Add if text.is_empty() => current.to_string(),
        Act::Add if current.trim().is_empty() => text.to_string(),
        Act::Add if list => {
            let mut have: Vec<String> = current
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            for t in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                if !have.iter().any(|h| h.eq_ignore_ascii_case(t)) {
                    have.push(t.to_string());
                }
            }
            have.join(", ")
        }
        Act::Add if lines => format!("{}\n{}", current.trim_end(), text),
        Act::Add => format!("{} {}", current.trim_end(), text),
    }
}

/// Check an info type as it is saved. Returns what to change, in words.
pub fn check(t: &InfoType) -> Result<(), String> {
    if t.name.trim().is_empty() {
        return Err("Give the field a name".into());
    }
    if t.connection_id.trim().is_empty() {
        return Err("Choose the connection this field belongs to".into());
    }
    let field = t.field.trim();
    if field.is_empty() {
        return Err("Write the field's key in the service's API".into());
    }
    if field.chars().any(char::is_control) {
        return Err("The field key contains characters that are not allowed".into());
    }
    let wants_list = matches!(t.kind, InfoKind::Choice | InfoKind::MultiChoice);
    match (&t.options, wants_list) {
        (InfoOptions::None, true) => {
            return Err("A choice needs a list: type the options or pick an API list".into())
        }
        (InfoOptions::Fixed { options }, _) => {
            if options.is_empty() {
                return Err("Add at least one option".into());
            }
            let mut seen = BTreeSet::new();
            for o in options {
                if o.value.trim().is_empty() {
                    return Err("Every option needs a value".into());
                }
                if !seen.insert(o.value.as_str()) {
                    return Err(format!("The value \"{}\" appears twice", o.value));
                }
            }
        }
        (InfoOptions::Api { options_id }, _) if options_id.trim().is_empty() => {
            return Err("Name the API list to use".into())
        }
        _ => {}
    }
    for s in &t.steps {
        if s.then.is_empty() {
            return Err("Each step needs something to add or set".into());
        }
        if let Some(w) = &s.when {
            if matches!(w.test, Test::Is | Test::IsNot | Test::Contains) && w.text.trim().is_empty()
            {
                return Err("Write the text the step looks for".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> InfoType {
        InfoType {
            name: "Priority".into(),
            connection_id: "jira".into(),
            field: "priority".into(),
            kind: InfoKind::Choice,
            options: InfoOptions::Fixed {
                options: vec![
                    InfoOption {
                        value: "1".into(),
                        label: "High".into(),
                    },
                    InfoOption {
                        value: "2".into(),
                        label: "Low".into(),
                    },
                ],
            },
            ..InfoType::default()
        }
    }

    #[test]
    fn a_complete_info_type_passes_and_round_trips() {
        let t = good();
        assert!(check(&t).is_ok());
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""type":"fixed""#));
        assert_eq!(serde_json::from_str::<InfoType>(&json).unwrap(), t);
    }

    #[test]
    fn steps_add_labels_once_and_lines_to_a_description() {
        assert_eq!(
            act("crash", Act::Add, "gpu-rtx, crash", true, false),
            "crash, gpu-rtx"
        );
        assert_eq!(act("", Act::Add, "a", true, false), "a");
        assert_eq!(
            act("Seen twice", Act::Add, "GPU: RTX", false, true),
            "Seen twice\nGPU: RTX"
        );
        assert_eq!(act("Bug", Act::Add, "in Maya", false, false), "Bug in Maya");
        assert_eq!(act("old", Act::Set, "new", true, false), "new");
        assert_eq!(act("keep", Act::Add, "  ", true, false), "keep");
    }

    #[test]
    fn conditions_look_at_a_value_without_minding_case() {
        let w = |test, text: &str| When {
            subject: "rule.gpu".into(),
            test,
            text: text.into(),
        };
        assert!(holds(&w(Test::Found, ""), "RTX"));
        assert!(!holds(&w(Test::Found, ""), "  "));
        assert!(holds(&w(Test::Missing, ""), ""));
        assert!(holds(&w(Test::Is, "rtx 4070"), "RTX 4070"));
        assert!(holds(&w(Test::IsNot, "AMD"), "RTX"));
        assert!(holds(&w(Test::Contains, "rtx"), "NVIDIA RTX 4070"));
        assert!(!holds(&w(Test::Contains, ""), "x"));
    }

    #[test]
    fn a_starting_value_from_before_is_the_first_step() {
        let t = InfoType {
            default_template: "High".into(),
            ..good()
        };
        let steps = t.all_steps();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].when.as_ref().unwrap().test, Test::Missing);
        assert!(!t.fills());
        assert!(t.applies_in("any"));
    }

    #[test]
    fn hand_written_minimal_files_load_with_defaults() {
        let t: InfoType =
            serde_json::from_str(r#"{"name":"Labels","connection_id":"jira","field":"labels"}"#)
                .unwrap();
        assert_eq!(t.kind, InfoKind::Text);
        assert_eq!(t.options, InfoOptions::None);
        assert!(check(&t).is_ok());
    }

    #[test]
    fn each_mistake_says_what_to_change() {
        let mut t = good();
        t.name = " ".into();
        assert!(check(&t).unwrap_err().contains("name"));
        let mut t = good();
        t.connection_id.clear();
        assert!(check(&t).unwrap_err().contains("connection"));
        let mut t = good();
        t.field = "a\u{0}b".into();
        assert!(check(&t).unwrap_err().contains("not allowed"));
        let mut t = good();
        t.options = InfoOptions::None;
        assert!(check(&t).unwrap_err().contains("needs a list"));
        let mut t = good();
        t.options = InfoOptions::Fixed {
            options: vec![
                InfoOption {
                    value: "x".into(),
                    label: "A".into(),
                },
                InfoOption {
                    value: "x".into(),
                    label: "B".into(),
                },
            ],
        };
        assert!(check(&t).unwrap_err().contains("twice"));
        let mut t = good();
        t.options = InfoOptions::Api {
            options_id: " ".into(),
        };
        assert!(check(&t).unwrap_err().contains("API list"));
    }
}
