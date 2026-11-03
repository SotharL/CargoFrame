use crate::Target;
use std::time::Duration;

/// Why a delivery stopped. The messages are for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeliverError {
    #[error("There was nothing to paste")]
    NothingToSend,
    #[error("A file to attach is missing: {0}")]
    MissingFile(String),
    #[error("Could not open Claude: {0}")]
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
    #[error("Cancelled")]
    Cancelled,
    #[error("The clipboard refused: {0}")]
    Clipboard(String),
    #[error("Could not press the keys: {0}")]
    Keys(String),
    #[error("Cannot tell which window is in front: {0}")]
    Desk(String),
}

fn not_in_front(target: Target, waited: Duration, seen: Option<&str>) -> String {
    let what = match target {
        Target::App => "The Claude app did not come to the front",
        Target::Browser => "A browser showing Claude did not come to the front",
    };
    let seen = seen
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!(" (\u{201c}{}\u{201d} was in front)", s.trim()))
        .unwrap_or_default();
    format!(
        "{what} within {} seconds{seen}, so nothing was pasted. Click the Claude window, then share again",
        waited.as_secs()
    )
}

fn lost_focus(stage: &str, now: Option<&str>) -> String {
    let doing = match stage {
        "attach" => "before the files were pasted",
        "write" => "before the text was pasted",
        "upload" => "while the files were uploading, so Enter was not pressed",
        "send" => "before Enter was pressed",
        _ => "while CargoFrame was waiting for the chat to load, so nothing was pasted",
    };
    let now = now
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!(" (\u{201c}{}\u{201d} is in front now)", s.trim()))
        .unwrap_or_default();
    let left = if stage == "wait" {
        ""
    } else {
        ". Whatever was already pasted is still in the chat's message box"
    };
    format!("Stopped: the Claude window lost the focus {doing}{now}{left}")
}
