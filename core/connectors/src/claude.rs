//! Claude: open a chat and paste the capture into it.
//!
//! There is no way to post into a claude.ai chat from outside, and a link can open a chat but
//! cannot fill it. So a share does what a person would do, through `cargoframe-deliver`: it
//! opens the chat (a new one, or one the person pinned), waits until the Claude window is really
//! in front, pastes the screenshot and the ticked files as files, pastes the message, and
//! presses Enter. Nothing leaves this computer except through the Claude app or browser the
//! person already uses.

use crate::ctx::Ctx;
use crate::error::invalid;
use crate::model::{self, OpenWith};
use crate::payload::{InlineFile, OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_deliver::{DeliverError, Delivery, Target, Timing};
use cargoframe_handoff::{open_url, ChatId, Destination, Scheme};
use cargoframe_safe::{FailureKind, SendError};
use std::path::PathBuf;
use std::time::Duration;

/// Claude takes at most this many files in one message (the screenshot is one of them) ...
pub const MAX_FILES: usize = 20;
/// ... and none larger than this.
pub const MAX_FILE_BYTES: u64 = 30 * 1024 * 1024;

pub struct ClaudeConnector {
    settings: model::Claude,
    windows: bool,
}

impl ClaudeConnector {
    pub fn new(settings: model::Claude) -> Self {
        Self {
            settings,
            windows: cfg!(windows),
        }
    }

    /// For tests: pretend to be on another system.
    #[cfg(test)]
    pub fn on_windows(mut self, windows: bool) -> Self {
        self.windows = windows;
        self
    }

    fn scheme(&self) -> Scheme {
        match self.settings.open_with {
            OpenWith::App => Scheme::App,
            OpenWith::Browser => Scheme::Web,
            // The desktop app registers its link on Windows; on Linux the browser is the sure way.
            OpenWith::Auto if self.windows => Scheme::App,
            OpenWith::Auto => Scheme::Web,
        }
    }
}

/// A fence for a block of text: longer than any run of backticks inside it, so the text cannot
/// close it early.
fn fence_for(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

/// The message that is pasted: the title (if there is one), the prompt, the values the button
/// maps, the window's name, the names of the attached files, and then the text files the person
/// chose to write into the message.
pub fn compose_message(
    title: Option<&str>,
    prompt: &str,
    window: &str,
    attached: &[&str],
    values: &[(&str, &str)],
    inline: &[InlineFile],
) -> String {
    let mut out = String::new();
    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        out.push_str("Title: ");
        out.push_str(title);
        out.push_str("\n\n");
    }
    out.push_str(prompt.trim());
    let mut facts = Vec::new();
    for (label, value) in values {
        facts.push(format!("{}: {}", label.trim(), value.trim()));
    }
    if !window.trim().is_empty() {
        facts.push(format!("Captured window: {}", window.trim()));
    }
    if !attached.is_empty() {
        facts.push(format!("Attached files: {}", attached.join(", ")));
    }
    if !facts.is_empty() {
        out.push_str("\n\n");
        out.push_str(&facts.join("\n"));
    }
    for file in inline {
        let body = file.text.trim_end_matches(['\n', '\r']);
        let fence = fence_for(body);
        out.push_str(&format!(
            "\n\nFile {}:\n{fence}\n{body}\n{fence}",
            file.name
        ));
    }
    out
}

fn undelivered(e: DeliverError) -> SendError {
    SendError::new(FailureKind::NotDelivered, e.to_string())
}

impl Connector for ClaudeConnector {
    fn describe(&self) -> Describe {
        Describe {
            fields: vec![
                FieldSpec {
                    key: "chat",
                    label: "Chat",
                    hint: "A chat id, or \"new\". Use the Claude chats dropdown",
                    list: None,
                },
                FieldSpec {
                    key: "title",
                    label: "Title",
                    hint: "Goes at the top of the message. Any other field you add is written under the prompt as \"Name: value\"",
                    list: None,
                },
            ],
            lists: vec![],
            summary: "Opens the chat, then pastes the screenshot, the files and the message into it and presses Enter. Leave Claude in front for a few seconds while it works. The prompt is required.".into(),
        }
    }

    fn status(&self, _ctx: &Ctx) -> Status {
        Status::Connected
    }

    fn disconnect(&self, _ctx: &Ctx) {}

    fn test(&self, _ctx: &Ctx) -> Result<String, SendError> {
        Ok(match self.scheme() {
            Scheme::App => "Shares open the Claude app and paste into it. Nothing is sent over the network by CargoFrame.",
            Scheme::Web => "Shares open claude.ai in your browser and paste into it. Nothing is sent over the network by CargoFrame.",
        }
        .into())
    }

    fn options(&self, _ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        Err(invalid(format!(
            "Claude has no list called \"{list}\": use the Claude chats dropdown source"
        )))
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let prompt = payload.prompt.as_deref().map(str::trim).unwrap_or("");
        if prompt.is_empty() {
            return Err(invalid(
                "Write a prompt first: Claude needs to know what to do with the screenshot",
            ));
        }
        let destination = match payload.value("chat") {
            None | Some("new") => Destination::NewChat,
            Some(id) => Destination::ExistingChat(
                ChatId::parse(id)
                    .ok_or_else(|| invalid("That chat id is not valid: pin the chat again"))?,
            ),
        };
        let scheme = self.scheme();
        let target = match scheme {
            Scheme::App => Target::App,
            Scheme::Web => Target::Browser,
        };

        // The screenshot first, then the ticked files.
        let mut files: Vec<PathBuf> = Vec::new();
        // The names of the ticked files that travel with the message (the screenshot is not one).
        let mut names: Vec<&str> = Vec::new();
        let mut notes = Vec::new();
        let local = payload.local.as_ref();
        match local.and_then(|l| l.screenshot.clone()) {
            Some(shot) => files.push(shot),
            // Left out on purpose (the button's rules) is not a problem; losing it is.
            None if payload.image.is_some() => {
                notes.push("There was no screenshot to attach".to_string())
            }
            None => {}
        }
        for (name, path) in local.map(|l| l.files.as_slice()).unwrap_or_default() {
            files.push(path.clone());
            names.push(name);
        }

        // Every field but the chat and the title is a value to show under the prompt.
        let values: Vec<(&str, &str)> = payload
            .fields
            .iter()
            .filter(|f| f.key != "chat" && f.key != "title" && !f.value.trim().is_empty())
            .map(|f| (f.key.as_str(), f.value.as_str()))
            .collect();
        let text = compose_message(
            payload.value("title"),
            prompt,
            &payload.title,
            &names,
            &values,
            &payload.inline_files,
        );
        let delivery = Delivery {
            open_url: open_url(&destination, scheme),
            target,
            files,
            text,
            submit: self.settings.send_automatically,
            timing: Timing::for_target(
                target,
                Duration::from_secs(u64::from(self.settings.extra_wait_seconds)),
            ),
        };
        let done = ctx
            .deliver
            .deliver(&delivery, &|url| ctx.browser.open(url), ctx.cancelled)
            .map_err(undelivered)?;

        if !payload.inline_files.is_empty() {
            let n = payload.inline_files.len();
            notes.push(format!(
                "{n} file{} written into the message as text",
                if n == 1 { " was" } else { "s were" }
            ));
        }
        if done.skipped_files > 0 {
            notes.push(format!(
                "This system cannot attach files from CargoFrame yet, so {} file{} not attached",
                done.skipped_files,
                if done.skipped_files == 1 {
                    " was"
                } else {
                    "s were"
                }
            ));
        }
        let what = match (done.files, done.text) {
            (0, _) => "the message".to_string(),
            (n, true) => format!("{n} file{} and the message", if n == 1 { "" } else { "s" }),
            (n, false) => format!("{n} file{}", if n == 1 { "" } else { "s" }),
        };
        let summary = if done.submitted {
            format!("Sent {what} to Claude")
        } else {
            format!("Pasted {what} into Claude: press Enter there to send")
        };
        if done.submitted {
            notes.push(
                "CargoFrame cannot see inside Claude: check the chat that the message went through"
                    .to_string(),
            );
        }
        Ok(SendOutcome {
            summary,
            url: None,
            notes,
            place: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Claude as ClaudeSettings;
    use crate::payload::{Field, LocalCopies};
    use crate::testkit::{FakeDeliver, World};
    use std::sync::Mutex;

    struct Recorder(Mutex<Vec<String>>);

    impl crate::Browser for Recorder {
        fn open(&self, url: &str) -> Result<(), String> {
            self.0.lock().unwrap().push(url.to_string());
            Ok(())
        }
    }

    const CHAT: &str = "0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0";

    fn payload(prompt: Option<&str>, chat: Option<&str>) -> Payload {
        Payload {
            title: "Foo - Editor".into(),
            local: Some(LocalCopies {
                screenshot: Some("/c/screenshot.png".into()),
                files: vec![
                    ("logs/app.log".into(), "/c/files/logs/app.log".into()),
                    ("config.json".into(), "/c/files/config.json".into()),
                ],
            }),
            image: Some(crate::payload::Attachment {
                name: "screenshot.png".into(),
                mime: "image/png".into(),
                data: vec![1],
            }),
            prompt: prompt.map(String::from),
            fields: chat
                .map(|c| vec![Field::text("chat", c)])
                .unwrap_or_default(),
            ..Payload::default()
        }
    }

    fn run(
        c: &ClaudeConnector,
        p: &Payload,
        deliver: &FakeDeliver,
    ) -> (Result<SendOutcome, SendError>, Vec<String>) {
        let rec = Recorder(Mutex::new(vec![]));
        let w = World::new();
        let r = w.with_all("claude", &rec, deliver, |ctx| c.send(ctx, p));
        (r, rec.0.into_inner().unwrap())
    }

    fn connector(windows: bool) -> ClaudeConnector {
        ClaudeConnector::new(ClaudeSettings::default()).on_windows(windows)
    }

    #[test]
    fn a_prompt_is_required_before_anything_happens() {
        let d = FakeDeliver::default();
        for prompt in [None, Some(""), Some("   \n")] {
            let (r, opened) = run(&connector(true), &payload(prompt, None), &d);
            assert!(r.unwrap_err().message.contains("Write a prompt"));
            assert!(opened.is_empty());
        }
        assert!(d.deliveries.lock().unwrap().is_empty());
    }

    #[test]
    fn one_click_sends_the_screenshot_the_files_and_the_message_into_the_chat() {
        let d = FakeDeliver::default();
        let (r, opened) = run(
            &connector(true),
            &payload(Some("What is wrong?"), Some(CHAT)),
            &d,
        );
        let out = r.unwrap();
        // The chat is opened by link, and nothing else travels in the link.
        assert_eq!(opened, vec![format!("claude://claude.ai/chat/{CHAT}")]);
        let sent = d.deliveries.lock().unwrap().clone();
        assert_eq!(sent.len(), 1);
        let s = &sent[0];
        assert_eq!(s.target, Target::App);
        assert!(s.submit);
        // Screenshot first, then the files, each as a file on disk.
        assert_eq!(
            s.files,
            vec![
                PathBuf::from("/c/screenshot.png"),
                PathBuf::from("/c/files/logs/app.log"),
                PathBuf::from("/c/files/config.json"),
            ]
        );
        assert_eq!(
            s.text,
            "What is wrong?\n\nCaptured window: Foo - Editor\nAttached files: logs/app.log, config.json"
        );
        assert_eq!(out.summary, "Sent 3 files and the message to Claude");
        assert!(out.notes.iter().any(|n| n.contains("check the chat")));
    }

    #[test]
    fn the_title_goes_at_the_top_when_there_is_one() {
        let d = FakeDeliver::default();
        let mut p = payload(Some("Why?"), None);
        p.fields
            .push(Field::text("title", "Login fails on staging"));
        p.local = None;
        let (r, _) = run(&connector(true), &p, &d);
        let out = r.unwrap();
        let s = d.deliveries.lock().unwrap()[0].clone();
        assert_eq!(
            s.text,
            "Title: Login fails on staging\n\nWhy?\n\nCaptured window: Foo - Editor"
        );
        assert!(s.files.is_empty());
        // The payload carries a picture that was not staged: that is a loss worth saying.
        assert!(out.notes.iter().any(|n| n.contains("no screenshot")));
        assert!(compose_message(None, "p", "", &[], &[], &[]) == "p");
    }

    #[test]
    fn leaving_the_screenshot_out_on_purpose_is_not_a_problem() {
        let d = FakeDeliver::default();
        let mut p = payload(Some("Why?"), None);
        p.image = None;
        p.local = Some(LocalCopies {
            screenshot: None,
            files: vec![("a.log".into(), "/c/files/a.log".into())],
        });
        let (r, _) = run(&connector(true), &p, &d);
        let out = r.unwrap();
        let s = d.deliveries.lock().unwrap()[0].clone();
        assert_eq!(s.files, vec![PathBuf::from("/c/files/a.log")]);
        // The first file is still named: nothing is skipped over for a missing picture.
        assert!(s.text.ends_with("Attached files: a.log"), "{}", s.text);
        assert!(!out.notes.iter().any(|n| n.contains("no screenshot")));
    }

    #[test]
    fn mapped_values_are_written_under_the_prompt_as_name_and_value() {
        let d = FakeDeliver::default();
        let mut p = payload(Some("Why?"), Some(CHAT));
        p.local = None;
        p.fields.push(Field::text("Build", " 1.2.3 "));
        p.fields.push(Field::text("Branch", "main"));
        p.fields.push(Field::text("Empty", "  "));
        p.fields.push(Field::text("title", "T"));
        let (r, _) = run(&connector(true), &p, &d);
        r.unwrap();
        let s = d.deliveries.lock().unwrap()[0].clone();
        assert_eq!(
            s.text,
            "Title: T\n\nWhy?\n\nBuild: 1.2.3\nBranch: main\nCaptured window: Foo - Editor"
        );
    }

    #[test]
    fn text_files_are_written_into_the_message_in_a_fence_that_cannot_be_closed_early() {
        let inline = [
            InlineFile {
                name: "logs/app.log".into(),
                text: "one\ntwo\n".into(),
            },
            InlineFile {
                name: "README.md".into(),
                text: "```rust\nfn main() {}\n```\n".into(),
            },
        ];
        let text = compose_message(None, "Why?", "", &[], &[], &inline);
        assert_eq!(
            text,
            "Why?\n\nFile logs/app.log:\n```\none\ntwo\n```\n\nFile README.md:\n````\n```rust\nfn main() {}\n```\n````"
        );
    }

    #[test]
    fn inline_files_travel_in_the_message_and_not_as_files() {
        let d = FakeDeliver::default();
        let mut p = payload(Some("Why?"), None);
        p.local = Some(LocalCopies {
            screenshot: Some("/c/screenshot.png".into()),
            files: vec![],
        });
        p.inline_files = vec![InlineFile {
            name: "a.log".into(),
            text: "boom".into(),
        }];
        let (r, _) = run(&connector(true), &p, &d);
        let out = r.unwrap();
        let s = d.deliveries.lock().unwrap()[0].clone();
        assert_eq!(s.files, vec![PathBuf::from("/c/screenshot.png")]);
        assert!(s.text.contains("File a.log:\n```\nboom\n```"), "{}", s.text);
        assert!(out
            .notes
            .iter()
            .any(|n| n.contains("1 file was written into the message as text")));
    }

    #[test]
    fn a_new_chat_opens_the_empty_chat_and_pastes_everything() {
        let d = FakeDeliver::default();
        let (_, opened) = run(&connector(false), &payload(Some("Hi"), Some("new")), &d);
        assert_eq!(opened, vec!["https://claude.ai/new".to_string()]);
        let (_, opened) = run(&connector(false), &payload(Some("Hi"), None), &d);
        assert_eq!(opened, vec!["https://claude.ai/new".to_string()]);
    }

    #[test]
    fn windows_opens_the_app_and_linux_the_browser_unless_told_otherwise() {
        let d = FakeDeliver::default();
        let open = |open_with, windows| {
            let c = ClaudeConnector::new(ClaudeSettings {
                open_with,
                ..Default::default()
            })
            .on_windows(windows);
            let (_, opened) = run(&c, &payload(Some("hi"), None), &d);
            let target = d.deliveries.lock().unwrap().last().unwrap().target;
            (opened.remove_first(), target)
        };
        trait First {
            fn remove_first(self) -> String;
        }
        impl First for Vec<String> {
            fn remove_first(mut self) -> String {
                self.remove(0)
            }
        }
        let (url, target) = open(OpenWith::Auto, true);
        assert!(url.starts_with("claude://claude.ai/new") && target == Target::App);
        let (url, target) = open(OpenWith::Auto, false);
        assert!(url.starts_with("https://claude.ai/new") && target == Target::Browser);
        assert!(open(OpenWith::App, false).0.starts_with("claude://"));
        assert!(open(OpenWith::Browser, true).0.starts_with("https://"));
    }

    #[test]
    fn the_settings_choose_enter_and_the_extra_wait() {
        let d = FakeDeliver::default();
        let c = ClaudeConnector::new(ClaudeSettings {
            send_automatically: false,
            extra_wait_seconds: 4,
            ..Default::default()
        })
        .on_windows(true);
        let (r, _) = run(&c, &payload(Some("hi"), None), &d);
        let out = r.unwrap();
        let s = d.deliveries.lock().unwrap()[0].clone();
        assert!(!s.submit);
        assert_eq!(
            s.timing.settle,
            Timing::for_target(Target::App, Duration::ZERO).settle + Duration::from_secs(4)
        );
        assert!(
            out.summary.contains("press Enter there to send"),
            "{}",
            out.summary
        );
        assert!(!out.notes.iter().any(|n| n.contains("check the chat")));
        // The defaults send for you.
        assert!(ClaudeSettings::default().send_automatically);
    }

    #[test]
    fn a_bad_chat_id_opens_nothing() {
        let d = FakeDeliver::default();
        let (r, opened) = run(&connector(true), &payload(Some("x"), Some("../evil")), &d);
        assert!(r.unwrap_err().message.contains("not valid"));
        assert!(opened.is_empty());
        assert!(d.deliveries.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failed_hand_over_is_reported_plainly_and_is_not_retried() {
        let d = FakeDeliver::default();
        *d.fail_with.lock().unwrap() = Some(DeliverError::LostFocus {
            stage: "send",
            now: Some("Notes".into()),
        });
        let (r, _) = run(&connector(true), &payload(Some("x"), None), &d);
        let e = r.unwrap_err();
        assert_eq!(e.kind, FailureKind::NotDelivered);
        assert!(!e.kind.is_retryable());
        assert!(e.message.contains("lost the focus"), "{}", e.message);
        assert_eq!(e.kind.label(), "Not delivered");
    }

    #[test]
    fn files_this_system_cannot_attach_are_reported() {
        struct NoFiles;
        impl cargoframe_deliver::Deliver for NoFiles {
            fn deliver(
                &self,
                d: &Delivery,
                _: &dyn Fn(&str) -> Result<(), String>,
                _: &dyn Fn() -> bool,
            ) -> Result<cargoframe_deliver::Delivered, DeliverError> {
                Ok(cargoframe_deliver::Delivered {
                    window: "Claude".into(),
                    files: 0,
                    skipped_files: d.files.len(),
                    text: true,
                    submitted: true,
                })
            }
        }
        let rec = Recorder(Mutex::new(vec![]));
        let w = World::new();
        let p = payload(Some("x"), None);
        let c = connector(false);
        let out = w
            .with_all("claude", &rec, &NoFiles, |ctx| c.send(ctx, &p))
            .unwrap();
        assert_eq!(out.summary, "Sent the message to Claude");
        assert!(out
            .notes
            .iter()
            .any(|n| n.contains("3 files were not attached")));
    }

    #[test]
    fn it_is_always_ready_and_has_no_lists() {
        let c = connector(true);
        let w = World::new();
        assert_eq!(w.with("claude", |ctx| c.status(ctx)), Status::Connected);
        assert!(w
            .with("claude", |ctx| c.test(ctx))
            .unwrap()
            .contains("Nothing is sent"));
        assert!(w.with("claude", |ctx| c.options(ctx, "x")).is_err());
        let fields: Vec<_> = c.describe().fields.iter().map(|f| f.key).collect();
        assert_eq!(fields, ["chat", "title"]);
    }
}
