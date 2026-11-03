//! Chat ids, links and the handoff plan.

/// A validated claude.ai chat id (lowercase UUID). A file with a bad id fails to load.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChatId(String);

impl TryFrom<String> for ChatId {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        Self::parse(&s).ok_or_else(|| format!("not a chat id: {s}"))
    }
}

impl From<ChatId> for String {
    fn from(id: ChatId) -> String {
        id.0
    }
}

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

/// The link that opens the chat. Nothing else can travel in a link: an existing chat takes no
/// text, and a new one takes only a short prompt, so the text and files are pasted afterwards.
pub fn open_url(destination: &Destination, scheme: Scheme) -> String {
    let base = scheme.base();
    match destination {
        Destination::NewChat => format!("{base}/new"),
        Destination::ExistingChat(id) => format!("{base}/chat/{}", id.as_str()),
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
    fn links_open_the_chat_and_nothing_else() {
        let id = ChatId::parse(ID).unwrap();
        let existing = Destination::ExistingChat(id);
        assert_eq!(
            open_url(&existing, Scheme::App),
            format!("claude://claude.ai/chat/{ID}")
        );
        assert_eq!(
            open_url(&existing, Scheme::Web),
            format!("https://claude.ai/chat/{ID}")
        );
        assert_eq!(
            open_url(&Destination::NewChat, Scheme::App),
            "claude://claude.ai/new"
        );
        assert_eq!(
            open_url(&Destination::NewChat, Scheme::Web),
            "https://claude.ai/new"
        );
    }
}
