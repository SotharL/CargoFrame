//! Info types: a field a destination understands, such as Jira's "Priority" or a custom field,
//! that a sharing button can fill from a block, a data rule or a fixed value.
//!
//! The person describes the field once (`infotypes/<id>.json`); the layout builder then offers
//! it as a target when mapping a sharing button, and the connector knows where to put the value.

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
    /// Starting value; may use `{{variables}}`.
    pub default_template: String,
}

/// Check an info type as it is saved. Returns what to change, in words.
pub fn check(t: &InfoType) -> Result<(), String> {
    if t.name.trim().is_empty() {
        return Err("Give the info type a name".into());
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
