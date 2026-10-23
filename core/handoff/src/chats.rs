//! The pinned and recent chat list shown in the Claude destination dropdown.
//!
//! Order in the dropdown: pinned chats (in the order they were pinned), then recent chats
//! (newest first), then "New chat", which the interface adds itself. Chats are added by
//! pasting a chat link once; CargoFrame keeps the list because Claude offers no way to read it.

use crate::link::ChatId;

/// How many recent (unpinned) chats are kept.
pub const MAX_RECENT: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEntry {
    pub id: ChatId,
    pub title: String,
    pub pinned: bool,
    /// Monotonic "last used" counter (or a timestamp). Larger is newer.
    pub last_used: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatList {
    entries: Vec<ChatEntry>, // pinned entries keep insertion order
}

impl ChatList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a chat (or update the title of a known one) and optionally pin it.
    pub fn add(&mut self, id: ChatId, title: &str, pin: bool, now: u64) {
        let title = title.trim();
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            if !title.is_empty() {
                e.title = title.to_string();
            }
            e.last_used = e.last_used.max(now);
            if pin {
                e.pinned = true;
            }
        } else {
            self.entries.push(ChatEntry {
                id,
                title: if title.is_empty() {
                    "Untitled chat".to_string()
                } else {
                    title.to_string()
                },
                pinned: pin,
                last_used: now,
            });
        }
        self.trim_recent();
    }

    /// Mark a chat as just used (after a share).
    pub fn touch(&mut self, id: &ChatId, now: u64) {
        if let Some(e) = self.entries.iter_mut().find(|e| &e.id == id) {
            e.last_used = e.last_used.max(now);
        }
    }

    pub fn set_pinned(&mut self, id: &ChatId, pinned: bool) -> bool {
        match self.entries.iter_mut().find(|e| &e.id == id) {
            Some(e) => {
                e.pinned = pinned;
                self.trim_recent();
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, id: &ChatId) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| &e.id != id);
        self.entries.len() != before
    }

    /// Pinned first, then recent newest first.
    pub fn ordered(&self) -> Vec<&ChatEntry> {
        let mut recent: Vec<&ChatEntry> = self.entries.iter().filter(|e| !e.pinned).collect();
        recent.sort_by_key(|e| std::cmp::Reverse(e.last_used));
        self.entries
            .iter()
            .filter(|e| e.pinned)
            .chain(recent)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn trim_recent(&mut self) {
        let mut recent: Vec<(usize, u64)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.pinned)
            .map(|(i, e)| (i, e.last_used))
            .collect();
        if recent.len() <= MAX_RECENT {
            return;
        }
        recent.sort_by_key(|&(_, used)| std::cmp::Reverse(used));
        let mut drop: Vec<usize> = recent[MAX_RECENT..].iter().map(|&(i, _)| i).collect();
        drop.sort_unstable_by(|a, b| b.cmp(a));
        for i in drop {
            self.entries.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u32) -> ChatId {
        ChatId::parse(&format!("00000000-0000-0000-0000-{n:012x}")).unwrap()
    }

    fn titles(list: &ChatList) -> Vec<String> {
        list.ordered().iter().map(|e| e.title.clone()).collect()
    }

    #[test]
    fn pinned_first_then_recent_newest_first() {
        let mut l = ChatList::new();
        l.add(id(1), "old", false, 1);
        l.add(id(2), "pinned-a", true, 2);
        l.add(id(3), "new", false, 3);
        l.add(id(4), "pinned-b", true, 4);
        assert_eq!(titles(&l), vec!["pinned-a", "pinned-b", "new", "old"]);
    }

    #[test]
    fn touching_moves_a_recent_chat_up() {
        let mut l = ChatList::new();
        l.add(id(1), "a", false, 1);
        l.add(id(2), "b", false, 2);
        l.touch(&id(1), 9);
        assert_eq!(titles(&l), vec!["a", "b"]);
    }

    #[test]
    fn re_adding_updates_instead_of_duplicating() {
        let mut l = ChatList::new();
        l.add(id(1), "first", false, 1);
        l.add(id(1), "renamed", true, 2);
        assert_eq!(l.len(), 1);
        let e = l.ordered()[0];
        assert_eq!(e.title, "renamed");
        assert!(e.pinned);
    }

    #[test]
    fn blank_title_gets_a_placeholder_and_keeps_old_title_on_update() {
        let mut l = ChatList::new();
        l.add(id(1), "  ", false, 1);
        assert_eq!(titles(&l), vec!["Untitled chat"]);
        l.add(id(1), "Real", false, 2);
        l.add(id(1), "", false, 3);
        assert_eq!(titles(&l), vec!["Real"]);
    }

    #[test]
    fn recent_list_is_capped_but_pins_are_kept() {
        let mut l = ChatList::new();
        l.add(id(1000), "pinned", true, 0);
        for n in 0..(MAX_RECENT as u32 + 5) {
            l.add(id(n), &format!("c{n}"), false, u64::from(n) + 1);
        }
        assert_eq!(l.len(), MAX_RECENT + 1);
        let t = titles(&l);
        assert_eq!(t[0], "pinned");
        assert_eq!(t[1], format!("c{}", MAX_RECENT + 4)); // newest first
        assert!(!t.contains(&"c0".to_string())); // oldest dropped
    }

    #[test]
    fn unpinning_and_removing() {
        let mut l = ChatList::new();
        l.add(id(1), "a", true, 1);
        assert!(l.set_pinned(&id(1), false));
        assert!(!l.set_pinned(&id(9), true));
        assert!(l.remove(&id(1)));
        assert!(!l.remove(&id(1)));
        assert!(l.is_empty());
    }
}
