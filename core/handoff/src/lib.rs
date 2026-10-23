//! One-way handoff to Claude.
//!
//! CargoFrame cannot post into claude.ai chats: there is no user-level API for it. Instead a
//! share to Claude puts the screenshot on the clipboard and opens a chat in the Claude app (or
//! the browser). This crate holds the pure parts: parsing a chat link the person pastes to pin
//! a chat, building the links to open, deciding what travels on the clipboard, and keeping the
//! pinned and recent chat list.
//!
//! Which links the Claude desktop app honours, and how its composer treats a pasted image plus
//! text, are settled by the Phase 0 spike (`docs/spikes.md`); the constants and flags here are
//! the knobs that spike tunes.

pub mod chats;
pub mod link;

pub use chats::{ChatEntry, ChatList};
pub use link::{parse_chat_link, plan, ChatId, ClipboardPlan, Destination, HandoffPlan, Scheme};
