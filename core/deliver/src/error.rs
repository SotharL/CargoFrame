use crate::Target;
use cargoframe_i18n::{t, tf};
use std::time::Duration;

/// Why a delivery stopped. The messages are for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeliverError {
    #[error("{}", cargoframe_i18n::t("There was nothing to paste"))]
    NothingToSend,
    #[error("{}", cargoframe_i18n::tf("A file to attach is missing: {value}", &[("value", &.0)]))]
    MissingFile(String),
    #[error("{}", cargoframe_i18n::tf("Could not open Claude: {value}", &[("value", &.0)]))]
    Open(String),
    #[error("{}", not_in_front(*.target, *.waited, .seen.as_deref()))]
    NotInFront {
        target: Target,
        waited: Duration,
        seen: Option<String>,
    },
    #[error("{}", lost_focus(.stage, .now.as_deref()))]
    LostFocus {
        stage: &'static str,
        now: Option<String>,
    },
    #[error("{}", cargoframe_i18n::t("Cancelled"))]
    Cancelled,
    #[error("{}", cargoframe_i18n::tf("The clipboard refused: {value}", &[("value", &.0)]))]
    Clipboard(String),
    #[error("{}", cargoframe_i18n::tf("Could not press the keys: {value}", &[("value", &.0)]))]
    Keys(String),
    #[error("{}", cargoframe_i18n::tf("Cannot tell which window is in front: {value}", &[("value", &.0)]))]
    Desk(String),
}

fn not_in_front(target: Target, waited: Duration, seen: Option<&str>) -> String {
    let what = match target {
        Target::App => cargoframe_i18n::t("The Claude app did not come to the front"),
        Target::Browser => cargoframe_i18n::t("A browser showing Claude did not come to the front"),
    };
    let seen = seen
        .filter(|s| !s.trim().is_empty())
        .map(|s| tf(" (\u{201c}{s}\u{201d} was in front)", &[("s", &s.trim())]))
        .unwrap_or_default();
    tf("{what} within {waited} seconds{seen}, so nothing was pasted. Click the Claude window, then share again", &[("what", &what), ("waited", &waited.as_secs()), ("seen", &seen)])
}

fn lost_focus(stage: &str, now: Option<&str>) -> String {
    let doing = match stage {
        "attach" => cargoframe_i18n::t("before the files were pasted"),
        "write" => cargoframe_i18n::t("before the text was pasted"),
        "upload" => cargoframe_i18n::t("while the files were uploading, so Enter was not pressed"),
        "send" => cargoframe_i18n::t("before Enter was pressed"),
        _ => cargoframe_i18n::t(
            "while CargoFrame was waiting for the chat to load, so nothing was pasted",
        ),
    };
    let now = now
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            tf(
                " (\u{201c}{s}\u{201d} is in front now)",
                &[("s", &s.trim())],
            )
        })
        .unwrap_or_default();
    let left = if stage == "wait" {
        ""
    } else {
        t(". Whatever was already pasted is still in the chat's message box")
    };
    tf(
        "Stopped: the Claude window lost the focus {doing}{now}{left}",
        &[("doing", &doing), ("now", &now), ("left", &left)],
    )
}
