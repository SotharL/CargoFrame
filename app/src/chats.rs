//! The list of Claude chats the person pinned, and the ones used recently. Claude offers no way
//! to read its own chat list, so a chat is added by pasting its link once.

use cargoframe_config::Store;
use cargoframe_handoff::{parse_chat_link, ChatId, ChatList};
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ChatView {
    pub id: String,
    pub title: String,
    pub pinned: bool,
}

fn load(store: &Store) -> Result<ChatList, String> {
    store
        .load_or_default::<ChatList>()
        .map(ChatList::normalized)
        .map_err(|e| e.to_string())
}

fn save(store: &Store, list: &ChatList) -> Result<(), String> {
    store.save(list).map_err(|e| e.to_string())
}

fn chat_id(id: &str) -> Result<ChatId, String> {
    ChatId::parse(id).ok_or_else(|| "That is not a chat id".to_string())
}

/// Pinned chats first, then recent ones, newest first.
pub fn list(store: &Store) -> Result<Vec<ChatView>, String> {
    Ok(load(store)?
        .ordered()
        .into_iter()
        .map(|e| ChatView {
            id: e.id.as_str().to_string(),
            title: e.title.clone(),
            pinned: e.pinned,
        })
        .collect())
}

/// Pin the chat a pasted link points to.
pub fn pin_link(store: &Store, link: &str, title: &str, now: u64) -> Result<(), String> {
    let id = parse_chat_link(link).ok_or_else(|| {
        "That does not look like a Claude chat link. Open the chat and copy its address \
         (https://claude.ai/chat/...)"
            .to_string()
    })?;
    let mut list = load(store)?;
    list.add(id, title, true, now);
    save(store, &list)
}

pub fn set_pinned(store: &Store, id: &str, pinned: bool) -> Result<(), String> {
    let id = chat_id(id)?;
    let mut list = load(store)?;
    if !list.set_pinned(&id, pinned) {
        return Err("That chat is not in the list".into());
    }
    save(store, &list)
}

pub fn remove(store: &Store, id: &str) -> Result<(), String> {
    let id = chat_id(id)?;
    let mut list = load(store)?;
    list.remove(&id);
    save(store, &list)
}

/// A share to an existing chat makes it the most recent one.
pub fn touch(store: &Store, id: &str, now: u64) {
    let Ok(id) = chat_id(id) else { return };
    if let Ok(mut list) = load(store) {
        list.touch(&id, now);
        if let Err(e) = save(store, &list) {
            tracing::warn!("could not update the chat list: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_config::Dirs;

    const A: &str = "0f1e2d3c-4b5a-6978-8a9b-0c1d2e3f4a5b";
    const B: &str = "11111111-2222-3333-4444-555555555555";

    fn store() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::new(Dirs::under(d.path()));
        (d, s)
    }

    #[test]
    fn pasting_a_link_pins_a_chat_and_bad_links_are_explained() {
        let (_d, s) = store();
        pin_link(
            &s,
            &format!("https://claude.ai/chat/{A}?x=1"),
            "Crash notes",
            10,
        )
        .unwrap();
        pin_link(&s, B, "", 11).unwrap();
        let l = list(&s).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].id.as_str(), l[0].title.as_str(), l[0].pinned),
            (A, "Crash notes", true)
        );
        assert_eq!(l[1].title, "Untitled chat");

        let err = pin_link(&s, "https://evil.example/chat/x", "", 12).unwrap_err();
        assert!(err.contains("Claude chat link"), "{err}");
        assert_eq!(list(&s).unwrap().len(), 2);
    }

    #[test]
    fn unpin_touch_and_remove() {
        let (_d, s) = store();
        pin_link(&s, A, "A", 1).unwrap();
        pin_link(&s, B, "B", 2).unwrap();
        set_pinned(&s, A, false).unwrap();
        set_pinned(&s, B, false).unwrap();
        // newest first among recent chats
        assert_eq!(list(&s).unwrap()[0].id, B);
        touch(&s, A, 99);
        assert_eq!(list(&s).unwrap()[0].id, A);
        assert!(set_pinned(&s, "not-an-id", true).is_err());
        remove(&s, A).unwrap();
        assert_eq!(list(&s).unwrap().len(), 1);
        // removing what is not there is fine
        remove(&s, A).unwrap();
    }
}
