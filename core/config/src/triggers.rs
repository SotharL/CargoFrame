//! Checks across all triggers: things one trigger file cannot know about the others.

use crate::models::Trigger;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The shortcut is not valid; the message says why.
    BadShortcut { trigger: String, message: String },
    /// Two enabled triggers use the same shortcut.
    DuplicateShortcut {
        shortcut: String,
        triggers: Vec<String>,
    },
    /// The layout the trigger points at does not exist.
    MissingLayout { trigger: String, layout: String },
    /// An enabled trigger with no layout shows an empty window; that is allowed but worth a hint.
    NoLayout { trigger: String },
}

/// `layouts` are the ids of the layout files that exist.
pub fn check(triggers: &[(String, Trigger)], layouts: &[String]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut by_shortcut: Vec<(String, Vec<String>)> = Vec::new();
    for (id, t) in triggers {
        if t.shortcut.trim().is_empty() {
            // Tray menu only: fine.
        } else {
            match crate::shortcut::normalize(&t.shortcut) {
                Ok(canon) if t.enabled => match by_shortcut.iter_mut().find(|(s, _)| *s == canon) {
                    Some((_, ids)) => ids.push(id.clone()),
                    None => by_shortcut.push((canon, vec![id.clone()])),
                },
                Ok(_) => {}
                Err(e) => problems.push(Problem::BadShortcut {
                    trigger: id.clone(),
                    message: e.to_string(),
                }),
            }
        }
        match &t.layout_id {
            Some(l) if !layouts.contains(l) => problems.push(Problem::MissingLayout {
                trigger: id.clone(),
                layout: l.clone(),
            }),
            None if t.enabled => problems.push(Problem::NoLayout {
                trigger: id.clone(),
            }),
            _ => {}
        }
    }
    for (shortcut, ids) in by_shortcut {
        if ids.len() > 1 {
            problems.push(Problem::DuplicateShortcut {
                shortcut,
                triggers: ids,
            });
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(shortcut: &str, enabled: bool, layout: Option<&str>) -> Trigger {
        Trigger {
            shortcut: shortcut.into(),
            enabled,
            layout_id: layout.map(String::from),
            ..Trigger::default()
        }
    }

    #[test]
    fn clean_setup_has_no_problems() {
        let triggers = vec![
            ("a".to_string(), t("Ctrl+Shift+F9", true, Some("l1"))),
            ("b".to_string(), t("", true, Some("l2"))),
        ];
        assert!(check(&triggers, &["l1".into(), "l2".into()]).is_empty());
    }

    #[test]
    fn duplicates_are_found_after_normalizing_and_ignore_disabled_triggers() {
        let triggers = vec![
            ("a".to_string(), t("ctrl+shift+f9", true, Some("l"))),
            ("b".to_string(), t("Shift+Ctrl+F9", true, Some("l"))),
            ("c".to_string(), t("Ctrl+Shift+F9", false, Some("l"))),
        ];
        let p = check(&triggers, &["l".into()]);
        assert_eq!(
            p,
            vec![Problem::DuplicateShortcut {
                shortcut: "Ctrl+Shift+F9".into(),
                triggers: vec!["a".into(), "b".into()]
            }]
        );
    }

    #[test]
    fn bad_shortcut_missing_layout_and_no_layout() {
        let triggers = vec![
            ("a".to_string(), t("Ctrl+Banana", true, Some("gone"))),
            ("b".to_string(), t("F9", true, None)),
            ("c".to_string(), t("F10", false, None)),
        ];
        let p = check(&triggers, &[]);
        assert!(matches!(p[0], Problem::BadShortcut { .. }));
        assert_eq!(
            p[1],
            Problem::MissingLayout {
                trigger: "a".into(),
                layout: "gone".into()
            }
        );
        assert_eq!(
            p[2],
            Problem::NoLayout {
                trigger: "b".into()
            }
        );
        assert_eq!(p.len(), 3);
    }
}
