//! Claude chats.
//!
//! CargoFrame cannot post into claude.ai chats: there is no user-level API for it, and a link can
//! open a chat but not fill it. A share to Claude opens the chat and then pastes into its window
//! (see `cargoframe-deliver`). This crate holds the pure parts: parsing a chat link the person
//! pastes to pin a chat, building the link that opens it, and keeping the pinned and recent chat
//! list.

pub mod chats;
pub mod link;

pub use chats::{ChatEntry, ChatList};
pub use link::{open_url, parse_chat_link, ChatId, Destination, Scheme};
