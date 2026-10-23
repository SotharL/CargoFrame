//! Chat ids, links and the handoff plan.

/// Longest percent-encoded prompt put in a `?q=` link. Longer prompts travel on the clipboard
/// instead. The real limit is a spike result; 6000 is a conservative starting point.
pub const MAX_QUERY_BYTES: usize = 6000;

/// A validated claude.ai chat id (lowercase UUID).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChatId(String);

impl ChatId {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let groups: Vec<&str> = s.split('-').collect();
        let lens = [8usize, 4, 4, 4, 12];
        let ok = groups.len() == 5
            && groups
                .iter()
                .zip(lens)
                .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()));
        ok.then(|| Self(s.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Accept what people paste to pin a chat: `https://claude.ai/chat/<id>` (with an optional
/// query, fragment or trailing slash), `claude://claude.ai/chat/<id>`, `claude.ai/chat/<id>`
/// or the bare id. Any other host is refused.
pub fn parse_chat_link(input: &str) -> Option<ChatId> {
    let s = input.trim();
    if let Some(id) = ChatId::parse(s) {
        return Some(id);
    }
    let rest = ["https://", "claude://"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        .unwrap_or(s);
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (host, path) = rest.split_once('/')?;
    if !host.eq_ignore_ascii_case("claude.ai") {
        return None;
    }
    let id = path.trim_end_matches('/').strip_prefix("chat/")?;
    ChatId::parse(id)
}

/// Where the share goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    NewChat,
    ExistingChat(ChatId),
}

/// Which kind of link to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// `claude://claude.ai/...`: opens the desktop app (Windows, macOS).
    App,
    /// `https://claude.ai/...`: the browser. Used on Linux until the spike shows otherwise.
    Web,
}

impl Scheme {
    fn base(self) -> &'static str {
        match self {
            Scheme::App => "claude://claude.ai",
            Scheme::Web => "https://claude.ai",
        }
    }
}

/// What the clipboard holds when the chat opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardPlan {
    /// Screenshot only. The prompt is already in the link.
    Image,
    /// Screenshot and prompt text together (the composer must accept both).
    ImageWithText,
    /// Screenshot first; after the person pastes it, CargoFrame offers "Copy prompt".
    ImageThenPromptStep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffPlan {
    pub open_url: String,
    pub clipboard: ClipboardPlan,
}

/// Percent-encode for a query value: unreserved characters stay, everything else becomes
/// `%XX` over the UTF-8 bytes.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decide the link and the clipboard contents.
///
/// * New chat with a short prompt: the prompt travels in `?q=`; the clipboard has the image.
/// * New chat with a long prompt, or any existing chat (documented links do not prefill
///   existing chats): the prompt travels on the clipboard. `composer_takes_image_and_text`
///   (a spike result) chooses between one combined paste and a second "Copy prompt" step.
pub fn plan(
    destination: &Destination,
    prompt: &str,
    scheme: Scheme,
    composer_takes_image_and_text: bool,
) -> HandoffPlan {
    let base = scheme.base();
    let prompt_on_clipboard = if composer_takes_image_and_text {
        ClipboardPlan::ImageWithText
    } else {
        ClipboardPlan::ImageThenPromptStep
    };
    let has_prompt = !prompt.trim().is_empty();

    match destination {
        Destination::ExistingChat(id) => HandoffPlan {
            open_url: format!("{base}/chat/{}", id.as_str()),
            clipboard: if has_prompt {
                prompt_on_clipboard
            } else {
                ClipboardPlan::Image
            },
        },
        Destination::NewChat => {
            let encoded = percent_encode(prompt);
            if !has_prompt {
                HandoffPlan {
                    open_url: format!("{base}/new"),
                    clipboard: ClipboardPlan::Image,
                }
            } else if encoded.len() <= MAX_QUERY_BYTES {
                HandoffPlan {
                    open_url: format!("{base}/new?q={encoded}"),
                    clipboard: ClipboardPlan::Image,
                }
            } else {
                HandoffPlan {
                    open_url: format!("{base}/new"),
                    clipboard: prompt_on_clipboard,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0";

    #[test]
    fn parses_pasted_links() {
        for input in [
            format!("https://claude.ai/chat/{ID}"),
            format!("  https://claude.ai/chat/{ID}/  "),
            format!("https://claude.ai/chat/{ID}?utm=x#top"),
            format!("claude://claude.ai/chat/{ID}"),
            format!("claude.ai/chat/{ID}"),
            format!("https://claude.ai/chat/{}", ID.to_uppercase()),
            ID.to_string(),
        ] {
            let id = parse_chat_link(&input).unwrap_or_else(|| panic!("failed: {input}"));
            assert_eq!(id.as_str(), ID);
        }
    }

    #[test]
    fn rejects_other_hosts_and_shapes() {
        for input in [
            format!("https://claude.ai.evil.com/chat/{ID}"),
            format!("https://evil.com/chat/{ID}"),
            format!("https://claude.ai@evil.com/chat/{ID}"),
            format!("https://claude.ai:8080/chat/{ID}"),
            format!("http://claude.ai/chat/{ID}"),
            "https://claude.ai/chat/not-a-uuid".to_string(),
            "https://claude.ai/new".to_string(),
            format!("https://claude.ai/project/{ID}"),
            String::new(),
            "hello".to_string(),
        ] {
            assert!(parse_chat_link(&input).is_none(), "accepted: {input}");
        }
    }

    #[test]
    fn percent_encoding() {
        assert_eq!(percent_encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(percent_encode("é\n"), "%C3%A9%0A");
        assert_eq!(percent_encode("A-z_0.9~"), "A-z_0.9~");
    }

    #[test]
    fn new_chat_with_short_prompt_uses_the_link() {
        let p = plan(&Destination::NewChat, "Look at this", Scheme::App, true);
        assert_eq!(p.open_url, "claude://claude.ai/new?q=Look%20at%20this");
        assert_eq!(p.clipboard, ClipboardPlan::Image);
    }

    #[test]
    fn new_chat_without_prompt_opens_empty() {
        let p = plan(&Destination::NewChat, "  ", Scheme::Web, true);
        assert_eq!(p.open_url, "https://claude.ai/new");
        assert_eq!(p.clipboard, ClipboardPlan::Image);
    }

    #[test]
    fn long_prompt_moves_to_the_clipboard() {
        let long = "x".repeat(MAX_QUERY_BYTES + 1);
        let combined = plan(&Destination::NewChat, &long, Scheme::App, true);
        assert_eq!(combined.open_url, "claude://claude.ai/new");
        assert_eq!(combined.clipboard, ClipboardPlan::ImageWithText);
        let stepped = plan(&Destination::NewChat, &long, Scheme::App, false);
        assert_eq!(stepped.clipboard, ClipboardPlan::ImageThenPromptStep);
    }

    #[test]
    fn encoded_length_decides_not_character_count() {
        // Each "é" is 6 bytes once encoded ("%C3%A9"), so this exceeds the limit.
        let text = "é".repeat(MAX_QUERY_BYTES / 6 + 1);
        let p = plan(&Destination::NewChat, &text, Scheme::App, true);
        assert_eq!(p.open_url, "claude://claude.ai/new");
    }

    #[test]
    fn existing_chat_carries_the_prompt_on_the_clipboard() {
        let id = ChatId::parse(ID).unwrap();
        let dest = Destination::ExistingChat(id);
        let p = plan(&dest, "Hello", Scheme::App, true);
        assert_eq!(p.open_url, format!("claude://claude.ai/chat/{ID}"));
        assert_eq!(p.clipboard, ClipboardPlan::ImageWithText);
        let p = plan(&dest, "Hello", Scheme::Web, false);
        assert_eq!(p.open_url, format!("https://claude.ai/chat/{ID}"));
        assert_eq!(p.clipboard, ClipboardPlan::ImageThenPromptStep);
        let p = plan(&dest, "", Scheme::Web, true);
        assert_eq!(p.clipboard, ClipboardPlan::Image);
    }
}
