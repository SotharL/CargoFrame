//! What each destination can take, in one table.
//!
//! The builder greys out what a button cannot send and says why, the capture window marks which
//! blocks each button sends, and the share itself skips what has no place at the destination.
//! All three read the table below, so they cannot disagree.
//!
//! Five things are described: free text (a message, a description, a post body), dropdown
//! choices (picked from the service's own lists), the screenshot, the ticked files, and links
//! written in the text. Every entry is `Yes`, `Limited` or `No`, with one line saying how or why,
//! in words for the person.

use crate::validate::ConnectionKind;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Yes,
    /// Works, with a catch the note explains.
    Limited,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Capability {
    pub support: Support,
    /// One short line: how it is used, or why it cannot be.
    pub note: &'static str,
}

impl Capability {
    pub const fn yes(note: &'static str) -> Self {
        Self {
            support: Support::Yes,
            note,
        }
    }
    pub const fn limited(note: &'static str) -> Self {
        Self {
            support: Support::Limited,
            note,
        }
    }
    pub const fn no(note: &'static str) -> Self {
        Self {
            support: Support::No,
            note,
        }
    }
    /// Can anything at all be sent?
    pub fn works(&self) -> bool {
        self.support != Support::No
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    pub text: Capability,
    pub dropdown: Capability,
    pub screenshot: Capability,
    pub files: Capability,
    pub link: Capability,
}

/// How a Custom API connection is set up, which decides what it can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CustomShape {
    /// The request has a body that can hold text.
    pub has_body: bool,
    /// The request is multipart and names a field for the screenshot.
    pub screenshot_field: bool,
    /// The request is multipart and names a field for the zip of files.
    pub files_field: bool,
    /// The connection defines at least one list.
    pub has_lists: bool,
}

impl Capabilities {
    /// The table for a kind of connection. `windows` matters for Claude only: pasting files into
    /// another program is done on Windows; elsewhere only the message goes.
    pub fn for_kind(kind: ConnectionKind, windows: bool) -> Self {
        match kind {
            ConnectionKind::Jira => Self {
                text: Capability::yes("Becomes the issue summary and description"),
                dropdown: Capability::yes("Priority, labels, components and more come from your Jira"),
                screenshot: Capability::yes("Attached to the issue"),
                files: Capability::yes("Attached to the issue as one zip"),
                link: Capability::yes("Links use Jira wiki style: [word|https://…]"),
            },
            ConnectionKind::Google => Self {
                text: Capability::limited("Goes into the spreadsheet columns you map, as plain text"),
                dropdown: Capability::limited("Only the Drive folder has a list to pick from"),
                screenshot: Capability::yes("Uploaded to Drive, with a link in the row"),
                files: Capability::yes("Uploaded to Drive as one zip, with a link in the row"),
                link: Capability::limited("Addresses stay plain text in a cell"),
            },
            ConnectionKind::Reddit => Self {
                text: Capability::yes("Becomes the post title and text"),
                dropdown: Capability::yes("The subreddit and flair come from your Reddit"),
                screenshot: Capability::yes("Posted as an image post"),
                files: Capability::no("Reddit cannot take files on a post"),
                link: Capability::yes("The post text understands Markdown links; the Link field makes a link post"),
            },
            ConnectionKind::Claude if windows => Self {
                text: Capability::yes("The prompt and title are typed into the chat"),
                dropdown: Capability::limited("Only the chat to open can be picked"),
                screenshot: Capability::yes("Pasted into the chat as a picture"),
                files: Capability::yes("Pasted into the chat as files (20 at most, 30 MB each), or as text"),
                link: Capability::yes("Addresses are typed into the message as written"),
            },
            ConnectionKind::Claude => Self {
                text: Capability::yes("The prompt and title are typed into the chat"),
                dropdown: Capability::limited("Only the chat to open can be picked"),
                screenshot: Capability::no("Pasting a picture into another program works on Windows only for now"),
                files: Capability::limited("Pasting files works on Windows only for now; text files can go inside the message"),
                link: Capability::yes("Addresses are typed into the message as written"),
            },
            ConnectionKind::Slack => Self {
                text: Capability::yes("Posted as the message (or the comment on the files)"),
                dropdown: Capability::limited("Only the channel has a list to pick from"),
                screenshot: Capability::yes("Uploaded to the channel as a picture"),
                files: Capability::yes("Uploaded to the channel as one zip"),
                link: Capability::yes("Addresses turn into links by themselves; <, > and & are shown as typed"),
            },
            ConnectionKind::Discord => Self {
                text: Capability::yes("Posted as the message, up to 2000 characters (a forum post's title is the name)"),
                dropdown: Capability::limited("The channel (or forum) and an existing thread can be picked"),
                screenshot: Capability::yes("Attached to the message"),
                files: Capability::limited("Attached as one zip, within the server's upload limit"),
                link: Capability::yes("Addresses and [word](https://…) links are shown as links"),
            },
            ConnectionKind::CustomApi => Self::custom(CustomShape::default()),
        }
    }

    /// A Custom API connection can carry what its request describes, and nothing else.
    pub fn custom(shape: CustomShape) -> Self {
        Self {
            text: if shape.has_body {
                Capability::yes("Placed in the request where you write {{prompt}} or {{field}}")
            } else {
                Capability::no("This request has no body to put text in")
            },
            dropdown: if shape.has_lists {
                Capability::yes("The lists you set up on the connection")
            } else {
                Capability::no("This connection has no lists to pick from")
            },
            screenshot: if shape.screenshot_field {
                Capability::yes("Sent in the multipart field you named")
            } else {
                Capability::no("The request has no multipart field for the screenshot")
            },
            files: if shape.files_field {
                Capability::yes("Sent as one zip in the multipart field you named")
            } else {
                Capability::no("The request has no multipart field for files")
            },
            link: Capability::limited("Sent as written; the service decides what it shows"),
        }
    }

    /// Saving a zip on this computer keeps everything.
    pub fn local() -> Self {
        Self {
            text: Capability::yes("Kept in share.json inside the zip"),
            dropdown: Capability::yes("The choices are kept in share.json inside the zip"),
            screenshot: Capability::yes("Kept as screenshot.png inside the zip"),
            files: Capability::yes("Kept in the zip's files folder"),
            link: Capability::yes("Kept as written"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [ConnectionKind; 7] = [
        ConnectionKind::Jira,
        ConnectionKind::Google,
        ConnectionKind::Reddit,
        ConnectionKind::Claude,
        ConnectionKind::Slack,
        ConnectionKind::Discord,
        ConnectionKind::CustomApi,
    ];

    #[test]
    fn every_entry_says_something() {
        for kind in KINDS {
            for windows in [true, false] {
                let c = Capabilities::for_kind(kind, windows);
                for cap in [c.text, c.dropdown, c.screenshot, c.files, c.link] {
                    assert!(!cap.note.trim().is_empty(), "{kind:?}");
                }
            }
        }
    }

    #[test]
    fn reddit_cannot_take_files_and_jira_can() {
        assert!(!Capabilities::for_kind(ConnectionKind::Reddit, true)
            .files
            .works());
        assert!(Capabilities::for_kind(ConnectionKind::Jira, true)
            .files
            .works());
    }

    #[test]
    fn claude_pastes_pictures_and_files_on_windows_only() {
        let win = Capabilities::for_kind(ConnectionKind::Claude, true);
        assert_eq!(win.screenshot.support, Support::Yes);
        assert_eq!(win.files.support, Support::Yes);
        let other = Capabilities::for_kind(ConnectionKind::Claude, false);
        assert_eq!(other.screenshot.support, Support::No);
        // Files stay "limited": a text file can still go inside the message.
        assert_eq!(other.files.support, Support::Limited);
        assert!(other.text.works());
    }

    #[test]
    fn a_custom_api_carries_what_its_request_describes() {
        let bare = Capabilities::for_kind(ConnectionKind::CustomApi, true);
        assert!(!bare.text.works() && !bare.screenshot.works() && !bare.files.works());
        assert!(!bare.dropdown.works());

        let full = Capabilities::custom(CustomShape {
            has_body: true,
            screenshot_field: true,
            files_field: true,
            has_lists: true,
        });
        assert!(full.text.works() && full.screenshot.works() && full.files.works());
        assert!(full.dropdown.works());

        let json_only = Capabilities::custom(CustomShape {
            has_body: true,
            ..CustomShape::default()
        });
        assert!(json_only.text.works());
        assert!(!json_only.screenshot.works() && !json_only.files.works());
    }

    #[test]
    fn saving_locally_keeps_everything() {
        let c = Capabilities::local();
        assert!([c.text, c.dropdown, c.screenshot, c.files, c.link]
            .iter()
            .all(|cap| cap.support == Support::Yes));
    }

    #[test]
    fn serialises_in_snake_case_for_the_interface() {
        let json =
            serde_json::to_value(Capabilities::for_kind(ConnectionKind::Reddit, true)).unwrap();
        assert_eq!(json["files"]["support"], "no");
        assert_eq!(json["screenshot"]["support"], "yes");
    }
}
