//! Claude: a one-way hand-off. There is no way to post into a claude.ai chat from outside, so a
//! share puts the screenshot on the clipboard and opens a chat (a new one, or one the person
//! pinned) in the Claude app or the browser. Nothing leaves this computer except the link that is
//! opened, and a prompt is required first.

use crate::ctx::Ctx;
use crate::error::invalid;
use crate::model::{self, OpenWith};
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_handoff::{plan, ChatId, ClipboardPlan, Destination, Scheme};
use cargoframe_safe::{FailureKind, SendError};

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

impl Connector for ClaudeConnector {
    fn describe(&self) -> Describe {
        Describe {
            fields: vec![FieldSpec {
                key: "chat",
                label: "Chat",
                hint: "A chat id, or \"new\". Use the Claude chats dropdown",
                list: None,
            }],
            lists: vec![],
            summary: "Copies the screenshot, then opens Claude. Paste it into the message box. The prompt is required.".into(),
        }
    }

    fn status(&self, _ctx: &Ctx) -> Status {
        Status::Connected
    }

    fn disconnect(&self, _ctx: &Ctx) {}

    fn test(&self, _ctx: &Ctx) -> Result<String, SendError> {
        Ok(match self.scheme() {
            Scheme::App => "Shares open the Claude app. Nothing is sent over the network by CargoFrame.",
            Scheme::Web => "Shares open claude.ai in your browser. Nothing is sent over the network by CargoFrame.",
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
        let mut handoff = plan(
            &destination,
            prompt,
            self.scheme(),
            self.settings.image_and_text_together,
        );

        let mut copy_next = None;
        let mut notes = Vec::new();
        let local = |e: String| {
            SendError::new(
                FailureKind::Rejected(0),
                format!("The clipboard refused: {e}"),
            )
        };
        match (&payload.image, handoff.clipboard) {
            (Some(img), ClipboardPlan::Image) => {
                ctx.clipboard.copy_image(&img.data).map_err(local)?;
            }
            (Some(img), ClipboardPlan::ImageWithText) => {
                if ctx
                    .clipboard
                    .copy_image_and_text(&img.data, prompt)
                    .is_err()
                {
                    // this system cannot hold both: paste the picture, then the prompt
                    ctx.clipboard.copy_image(&img.data).map_err(local)?;
                    copy_next = Some(prompt.to_string());
                    handoff.clipboard = ClipboardPlan::ImageThenPromptStep;
                }
            }
            (Some(img), ClipboardPlan::ImageThenPromptStep) => {
                ctx.clipboard.copy_image(&img.data).map_err(local)?;
                copy_next = Some(prompt.to_string());
            }
            (None, ClipboardPlan::Image) => {
                notes.push("There was no screenshot to copy".to_string());
            }
            (None, _) => {
                ctx.clipboard.copy_text(prompt).map_err(local)?;
                notes
                    .push("There was no screenshot, so the prompt is on the clipboard".to_string());
            }
        }
        if payload.files_zip.is_some() {
            notes.push(
                "Claude chats cannot take files from here, so the ticked files were not sent"
                    .to_string(),
            );
        }
        ctx.browser.open(&handoff.open_url).map_err(|e| {
            SendError::new(
                FailureKind::Rejected(0),
                format!("Could not open Claude: {e}"),
            )
        })?;

        let summary = match (&copy_next, &payload.image) {
            (Some(_), _) => {
                "Claude is open: paste the screenshot, then copy the prompt".to_string()
            }
            (None, Some(_)) => {
                "Claude is open: paste the screenshot into the message box".to_string()
            }
            (None, None) => "Claude is open".to_string(),
        };
        Ok(SendOutcome {
            summary,
            url: None,
            notes,
            copy_next,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Claude as ClaudeSettings;
    use crate::payload::{Attachment, Field};
    use crate::testkit::{FakeClipboard, World};
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
            image: Some(Attachment {
                name: "screenshot.png".into(),
                mime: "image/png".into(),
                data: vec![1, 2, 3],
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
        clip: &FakeClipboard,
    ) -> (Result<SendOutcome, SendError>, Vec<String>) {
        let rec = Recorder(Mutex::new(vec![]));
        let w = World::new();
        let r = w.with_all("claude", &rec, clip, |ctx| c.send(ctx, p));
        (r, rec.0.into_inner().unwrap())
    }

    #[test]
    fn a_prompt_is_required_before_anything_happens() {
        let clip = FakeClipboard::default();
        for prompt in [None, Some(""), Some("   \n")] {
            let (r, opened) = run(
                &ClaudeConnector::new(ClaudeSettings::default()),
                &payload(prompt, None),
                &clip,
            );
            assert!(r.unwrap_err().message.contains("Write a prompt"));
            assert!(opened.is_empty());
        }
        assert!(clip.images.lock().unwrap().is_empty());
    }

    #[test]
    fn a_new_chat_carries_a_short_prompt_in_the_link_and_the_picture_on_the_clipboard() {
        let clip = FakeClipboard::default();
        let c = ClaudeConnector::new(ClaudeSettings::default()).on_windows(false);
        let (r, opened) = run(&c, &payload(Some("What is this?"), None), &clip);
        let out = r.unwrap();
        assert_eq!(
            opened,
            vec!["https://claude.ai/new?q=What%20is%20this%3F".to_string()]
        );
        assert_eq!(clip.images.lock().unwrap().as_slice(), &[vec![1, 2, 3]]);
        assert_eq!(out.copy_next, None);
        assert!(out.summary.contains("paste the screenshot"));
    }

    #[test]
    fn windows_opens_the_app_and_linux_the_browser_unless_told_otherwise() {
        let clip = FakeClipboard::default();
        let open = |open_with, windows| {
            let c = ClaudeConnector::new(ClaudeSettings {
                open_with,
                ..Default::default()
            })
            .on_windows(windows);
            run(&c, &payload(Some("hi"), None), &clip).1.remove(0)
        };
        assert!(open(OpenWith::Auto, true).starts_with("claude://claude.ai/new"));
        assert!(open(OpenWith::Auto, false).starts_with("https://claude.ai/new"));
        assert!(open(OpenWith::App, false).starts_with("claude://"));
        assert!(open(OpenWith::Browser, true).starts_with("https://"));
    }

    #[test]
    fn an_existing_chat_needs_the_prompt_as_a_second_paste() {
        let clip = FakeClipboard::default();
        let c = ClaudeConnector::new(ClaudeSettings::default()).on_windows(false);
        let (r, opened) = run(&c, &payload(Some("Explain"), Some(CHAT)), &clip);
        let out = r.unwrap();
        assert_eq!(opened, vec![format!("https://claude.ai/chat/{CHAT}")]);
        assert_eq!(out.copy_next.as_deref(), Some("Explain"));
        assert!(out.summary.contains("then copy the prompt"));
        assert_eq!(clip.images.lock().unwrap().len(), 1);
        assert!(
            clip.texts.lock().unwrap().is_empty(),
            "the prompt waits for the second step"
        );
    }

    #[test]
    fn picture_and_text_together_when_the_composer_and_clipboard_allow() {
        let both = FakeClipboard {
            can_hold_both: true,
            ..Default::default()
        };
        let c = ClaudeConnector::new(ClaudeSettings {
            image_and_text_together: true,
            ..Default::default()
        })
        .on_windows(false);
        let (r, _) = run(&c, &payload(Some("Explain"), Some(CHAT)), &both);
        assert_eq!(r.unwrap().copy_next, None);
        assert_eq!(both.both.lock().unwrap().len(), 1);

        // the setting says yes but this clipboard cannot: fall back to two pastes
        let one = FakeClipboard::default();
        let (r, _) = run(&c, &payload(Some("Explain"), Some(CHAT)), &one);
        assert_eq!(r.unwrap().copy_next.as_deref(), Some("Explain"));
        assert_eq!(one.images.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_bad_chat_id_a_broken_clipboard_and_no_screenshot() {
        let c = ClaudeConnector::new(ClaudeSettings::default()).on_windows(false);
        let clip = FakeClipboard::default();
        let (r, opened) = run(&c, &payload(Some("x"), Some("../evil")), &clip);
        assert!(r.unwrap_err().message.contains("not valid"));
        assert!(opened.is_empty());

        let broken = FakeClipboard {
            fail: true,
            ..Default::default()
        };
        let (r, opened) = run(&c, &payload(Some("x"), None), &broken);
        assert!(r.unwrap_err().message.contains("clipboard refused"));
        assert!(
            opened.is_empty(),
            "Claude is not opened if the picture could not be copied"
        );

        let mut p = payload(Some("x"), Some(CHAT));
        p.image = None;
        let clip = FakeClipboard::default();
        let (r, _) = run(&c, &p, &clip);
        let out = r.unwrap();
        assert!(out.notes.iter().any(|n| n.contains("no screenshot")));
        assert_eq!(clip.texts.lock().unwrap().as_slice(), &["x".to_string()]);
    }

    #[test]
    fn ticked_files_are_reported_as_not_sent() {
        let c = ClaudeConnector::new(ClaudeSettings::default()).on_windows(false);
        let mut p = payload(Some("x"), None);
        p.files_zip = Some(Attachment {
            name: "f.zip".into(),
            mime: "application/zip".into(),
            data: vec![1],
        });
        let (r, _) = run(&c, &p, &FakeClipboard::default());
        assert!(r
            .unwrap()
            .notes
            .iter()
            .any(|n| n.contains("ticked files were not sent")));
    }

    #[test]
    fn it_is_always_ready_and_has_no_lists() {
        let c = ClaudeConnector::new(ClaudeSettings::default());
        let w = World::new();
        assert_eq!(w.with("claude", |ctx| c.status(ctx)), Status::Connected);
        assert!(w
            .with("claude", |ctx| c.test(ctx))
            .unwrap()
            .contains("Nothing is sent"));
        assert!(w.with("claude", |ctx| c.options(ctx, "x")).is_err());
    }
}
