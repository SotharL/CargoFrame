//! Values made from other values. A rule's value can be a list of choices instead of something to
//! find in a file: "if `{{rule.shelf}}` is found, `{{rule.stream}}_{{rule.change}}_{{rule.shelf}}`,
//! else `{{rule.stream}}_{{rule.change}}`". The choices are tried from the top and the first that
//! fits gives the value. A choice with no conditions fits when every value its text names is
//! found, so most combined values need no condition at all.
//!
//! They run after every file was read, with every value the capture knows (the program, the
//! project, the date and the values read from files), and in the order they are listed, so a
//! combined value can use one listed before it.

use crate::model::{Choice, Cond, CondTest, Find, Rule};
use crate::run::{Outcome, Report};
use cargoframe_context::template;
use std::collections::BTreeMap;

/// `{{rule.shelf}}` or `rule.shelf` → `rule.shelf`.
fn name_of(raw: &str) -> &str {
    raw.trim()
        .trim_start_matches("{{")
        .trim_end_matches("}}")
        .trim()
}

/// Whether one condition holds. A value that is not known counts as empty.
pub fn holds(c: &Cond, known: &BTreeMap<String, String>) -> bool {
    let v = known
        .get(name_of(&c.value))
        .map(|v| v.trim().to_lowercase())
        .unwrap_or_default();
    let t = c.text.trim().to_lowercase();
    match c.test {
        CondTest::Found => !v.is_empty(),
        CondTest::Missing => v.is_empty(),
        CondTest::Is => v == t,
        CondTest::IsNot => v != t,
        CondTest::Contains => !t.is_empty() && v.contains(&t),
    }
}

/// The value a choice gives, if it fits.
pub fn try_choice(choice: &Choice, known: &BTreeMap<String, String>) -> Option<String> {
    if !choice.when.iter().all(|c| holds(c, known)) {
        return None;
    }
    let lookup = |name: &str| known.get(name).filter(|v| !v.trim().is_empty()).cloned();
    let rendered = template::render(&choice.text, lookup);
    // with no condition, every value it names must be there
    if choice.when.is_empty() && !rendered.unresolved.is_empty() {
        return None;
    }
    let text = if rendered.unresolved.is_empty() {
        rendered.text
    } else {
        // a value the conditions did not ask for is left out, not shown as {{name}}
        template::render(&choice.text, |name| Some(lookup(name).unwrap_or_default())).text
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The value of a list of choices: the first that fits.
pub fn pick(choices: &[Choice], known: &BTreeMap<String, String>) -> Option<String> {
    choices.iter().find_map(|c| try_choice(c, known))
}

/// Make every combined value of `rules`, in order, from `known` (the capture's values, the
/// values read from files): each is added to `report` and to `known` as `rule.<key>`, so a later
/// one can use it.
pub fn combine_values<'a>(
    rules: impl IntoIterator<Item = (&'a str, &'a Rule)>,
    report: &mut Report,
    known: &mut BTreeMap<String, String>,
) {
    for (id, rule) in rules {
        for step in &rule.values {
            let Find::Combine { choices } = &step.find else {
                continue;
            };
            let value = pick(choices, known);
            let outcome = match value {
                Some(v) => Outcome {
                    value: Some(v),
                    problem: None,
                    used_fallback: false,
                },
                None => Outcome {
                    value: step.fallback.clone(),
                    problem: Some(
                        "none of its choices fits: a value it needs was not found".into(),
                    ),
                    used_fallback: step.fallback.is_some(),
                },
            };
            if let Some(v) = &outcome.value {
                known.insert(format!("rule.{}", step.key), v.clone());
            }
            report.owners.insert(step.key.clone(), id.to_string());
            report.values.insert(step.key.clone(), outcome);
        }
    }
}

/// Check the choices a person wrote.
pub fn check(choices: &[Choice]) -> Result<(), String> {
    if choices.is_empty() {
        return Err("Add a choice: the text the value is made of".into());
    }
    for c in choices {
        if c.text.trim().is_empty() {
            return Err(
                "Write what each choice is made of, like {{rule.stream}}_{{rule.config}}".into(),
            );
        }
        for w in &c.when {
            if name_of(&w.value).is_empty() {
                return Err("Choose the value each condition looks at".into());
            }
            if matches!(w.test, CondTest::Is | CondTest::IsNot | CondTest::Contains)
                && w.text.trim().is_empty()
            {
                return Err("Write the text a condition looks for".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ValueStep, Which};

    fn known(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn choice(text: &str, when: Vec<Cond>) -> Choice {
        Choice {
            when,
            text: text.into(),
        }
    }

    fn found(v: &str) -> Cond {
        Cond {
            value: v.into(),
            test: CondTest::Found,
            text: String::new(),
        }
    }

    /// The build id of the request: with a shelf, it goes before the config.
    fn build_id() -> Vec<Choice> {
        vec![
            choice(
                "{{rule.stream}}_{{rule.change}}_{{rule.shelf}}_{{rule.config}}",
                vec![],
            ),
            choice("{{rule.stream}}_{{rule.change}}_{{rule.config}}", vec![]),
        ]
    }

    #[test]
    fn the_first_choice_whose_values_are_all_found_gives_the_value() {
        let with_shelf = known(&[
            ("rule.stream", "main"),
            ("rule.change", "4521"),
            ("rule.shelf", "88"),
            ("rule.config", "Debug"),
        ]);
        assert_eq!(
            pick(&build_id(), &with_shelf).as_deref(),
            Some("main_4521_88_Debug")
        );
        let no_shelf = known(&[
            ("rule.stream", "main"),
            ("rule.change", "4521"),
            ("rule.shelf", "  "),
            ("rule.config", "Debug"),
        ]);
        assert_eq!(
            pick(&build_id(), &no_shelf).as_deref(),
            Some("main_4521_Debug")
        );
        // no config: nothing fits
        assert_eq!(pick(&build_id(), &known(&[("rule.stream", "main")])), None);
    }

    #[test]
    fn conditions_choose_and_a_value_they_did_not_ask_for_is_left_out() {
        let choices = vec![
            choice(
                "{{rule.stream}}-{{rule.shelf}}",
                vec![
                    found("{{rule.stream}}"),
                    Cond {
                        value: "rule.config".into(),
                        test: CondTest::Is,
                        text: "release".into(),
                    },
                ],
            ),
            choice("dev", vec![]),
        ];
        let k = known(&[("rule.stream", "main"), ("rule.config", "Release")]);
        assert_eq!(pick(&choices, &k).as_deref(), Some("main-"));
        let k = known(&[("rule.stream", "main"), ("rule.config", "Debug")]);
        assert_eq!(pick(&choices, &k).as_deref(), Some("dev"));
        let contains = Cond {
            value: "app.exe".into(),
            test: CondTest::Contains,
            text: "EDITOR".into(),
        };
        assert!(holds(&contains, &known(&[("app.exe", "UnrealEditor.exe")])));
        let missing = Cond {
            value: "rule.shelf".into(),
            test: CondTest::Missing,
            text: String::new(),
        };
        assert!(holds(&missing, &known(&[])));
    }

    #[test]
    fn combined_values_run_in_order_and_use_the_ones_before_them() {
        let step = |key: &str, choices: Vec<Choice>, fallback: Option<&str>| ValueStep {
            key: key.into(),
            name: key.into(),
            find: Find::Combine { choices },
            which: Which::First,
            fallback: fallback.map(str::to_string),
        };
        let rule = Rule {
            name: "Build".into(),
            values: vec![
                step("build_id", build_id(), None),
                step(
                    "title",
                    vec![choice("[{{rule.build_id}}] {{app.title}}", vec![])],
                    None,
                ),
                step(
                    "missing",
                    vec![choice("{{rule.nope}}", vec![])],
                    Some("none"),
                ),
            ],
            ..Rule::default()
        };
        let mut report = Report::default();
        let mut k = known(&[
            ("rule.stream", "main"),
            ("rule.change", "1"),
            ("rule.config", "Debug"),
            ("app.title", "Editor"),
        ]);
        combine_values([("build", &rule)], &mut report, &mut k);
        assert_eq!(
            report.values["build_id"].value.as_deref(),
            Some("main_1_Debug")
        );
        assert_eq!(
            report.values["title"].value.as_deref(),
            Some("[main_1_Debug] Editor")
        );
        assert!(report.values["missing"].used_fallback);
        assert_eq!(k["rule.title"], "[main_1_Debug] Editor");
        assert_eq!(report.owners["title"], "build");
        assert!(rule.only_combines());
    }

    #[test]
    fn what_a_person_wrote_is_checked() {
        assert!(check(&[]).is_err());
        assert!(check(&[choice(" ", vec![])]).is_err());
        let no_text = Cond {
            value: "rule.a".into(),
            test: CondTest::Is,
            text: String::new(),
        };
        assert!(check(&[choice("x", vec![no_text])]).is_err());
        assert!(check(&build_id()).is_ok());
    }
}
