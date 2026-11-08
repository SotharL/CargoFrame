//! What each destination can take, in one table.
//!
//! The builder greys out what a button cannot send and says why, the capture window marks which
//! blocks each button sends, and the share itself skips what has no place at the destination.
//! All three read the table below, so they cannot disagree.
//!
//! Six things are described: free text (a message, a description, a post body), dropdown
//! choices (picked from the service's own lists), the screenshot, the video, the ticked files,
//! and links written in the text. Every entry is `Yes`, `Limited` or `No`, with one line saying how or why,
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
    /// The screen recording a video trigger makes.
    pub video: Capability,
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
                video: Capability::limited("Attached to the issue, within your Jira's attachment size limit"),
                files: Capability::yes("Attached to the issue as one zip"),
                link: Capability::yes("Links use Jira wiki style: [word|https://…]"),
            },
            ConnectionKind::Google => Self {
                text: Capability::limited("Goes into the spreadsheet columns you map, as plain text"),
                dropdown: Capability::limited("Only the Drive folder has a list to pick from"),
                screenshot: Capability::yes("Uploaded to Drive, with a link in the row"),
                video: Capability::yes("Uploaded to Drive, with a link in the row"),
                files: Capability::yes("Uploaded to Drive as one zip, with a link in the row"),
                link: Capability::limited("Addresses stay plain text in a cell"),
            },
            ConnectionKind::Reddit => Self {
                text: Capability::yes("Becomes the post title and text"),
                dropdown: Capability::yes("The subreddit and flair come from your Reddit"),
                screenshot: Capability::yes("Posted as an image post"),
                video: Capability::no("Reddit video posts are not supported yet"),
                files: Capability::no("Reddit cannot take files on a post"),
                link: Capability::yes("The post text understands Markdown links; the Link field makes a link post"),
            },
            ConnectionKind::Claude if windows => Self {
                text: Capability::yes("The prompt and title are typed into the chat"),
                dropdown: Capability::limited("Only the chat to open can be picked"),
                screenshot: Capability::yes("Pasted into the chat as a picture"),
                video: Capability::no("Claude cannot watch a video"),
                files: Capability::yes("Pasted into the chat as files (20 at most, 30 MB each), or as text"),
                link: Capability::yes("Addresses are typed into the message as written"),
            },
            ConnectionKind::Claude => Self {
                text: Capability::yes("The prompt and title are typed into the chat"),
                dropdown: Capability::limited("Only the chat to open can be picked"),
                screenshot: Capability::no("Pasting a picture into another program works on Windows only for now"),
                video: Capability::no("Claude cannot watch a video"),
                files: Capability::limited("Pasting files works on Windows only for now; text files can go inside the message"),
                link: Capability::yes("Addresses are typed into the message as written"),
            },
            ConnectionKind::Slack => Self {
                text: Capability::yes("Posted as the message (or the comment on the files)"),
                dropdown: Capability::limited("Only the channel has a list to pick from"),
                screenshot: Capability::yes("Uploaded to the channel as a picture"),
                video: Capability::yes("Uploaded to the channel as a video"),
                files: Capability::yes("Uploaded to the channel as one zip"),
                link: Capability::yes("Addresses turn into links by themselves; <, > and & are shown as typed"),
            },
            ConnectionKind::Discord => Self {
                text: Capability::yes("Posted as the message, up to 2000 characters (a forum post's title is the name)"),
                dropdown: Capability::limited("The channel (or forum) and an existing thread can be picked"),
                screenshot: Capability::yes("Attached to the message"),
                video: Capability::limited("Attached to the message, within the server's upload limit (often 10 MB)"),
                files: Capability::limited("Attached as one zip, within the server's upload limit"),
                link: Capability::yes("Addresses and [word](https://…) links are shown as links"),
            },
            ConnectionKind::Teams => Self {
                text: Capability::yes("Posted as the message (a new channel post uses the title as its headline)"),
                dropdown: Capability::limited("Your chats and channels can be picked; a thread is chosen by its link"),
                screenshot: Capability::yes("Shown inside the message"),
                video: Capability::no("Teams needs files in OneDrive, which CargoFrame does not use yet"),
                files: Capability::no("Teams needs files in OneDrive, which CargoFrame does not use yet"),
                link: Capability::yes("Addresses are shown as typed and Teams links them by itself; markup is shown as text"),
            },
            ConnectionKind::X => Self {
                text: Capability::limited("Posted as the post, at most 280 characters; formatting is shown as plain text"),
                dropdown: Capability::no("X has no lists to pick from"),
                screenshot: Capability::yes("Attached to the post, made smaller when over 5 MB"),
                video: Capability::no("Videos are not sent to X yet; the screenshot is"),
                files: Capability::no("X cannot take files"),
                link: Capability::yes("Addresses become links by themselves"),
            },
            ConnectionKind::Bluesky => Self {
                text: Capability::limited("Posted as the post, at most 300 characters; formatting is shown as plain text"),
                dropdown: Capability::no("Bluesky has no lists to pick from"),
                screenshot: Capability::yes("Attached to the post, made smaller when over 1 MB"),
                video: Capability::no("Videos are not sent to Bluesky yet; the screenshot is"),
                files: Capability::no("Bluesky cannot take files"),
                link: Capability::yes("Addresses in the text are marked as links"),
            },
            ConnectionKind::Instagram => Self {
                text: Capability::limited("The caption, at most 2,200 characters; formatting is shown as plain text"),
                dropdown: Capability::no("Instagram has no lists to pick from"),
                screenshot: Capability::yes("The post's picture, put on your picture server first, as a JPEG"),
                video: Capability::no("Videos are not sent to Instagram yet; the screenshot is"),
                files: Capability::no("Instagram cannot take files"),
                link: Capability::limited("Addresses are shown as text: Instagram captions have no links"),
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
            video: Capability::no("A Custom API connection cannot send a video yet"),
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
            video: Capability::yes("Kept as recording.mp4 inside the zip"),
            files: Capability::yes("Kept in the zip's files folder"),
            link: Capability::yes("Kept as written"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [ConnectionKind; 11] = [
        ConnectionKind::X,
        ConnectionKind::Bluesky,
        ConnectionKind::Instagram,
        ConnectionKind::Jira,
        ConnectionKind::Google,
        ConnectionKind::Reddit,
        ConnectionKind::Claude,
        ConnectionKind::Slack,
        ConnectionKind::Discord,
        ConnectionKind::Teams,
        ConnectionKind::CustomApi,
    ];

    #[test]
    fn every_entry_says_something() {
        for kind in KINDS {
            for windows in [true, false] {
                let c = Capabilities::for_kind(kind, windows);
                for cap in [c.text, c.dropdown, c.screenshot, c.video, c.files, c.link] {
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
        assert!([c.text, c.dropdown, c.screenshot, c.video, c.files, c.link]
            .iter()
            .all(|cap| cap.support == Support::Yes));
    }

    #[test]
    fn a_video_goes_where_a_file_can_be_uploaded_and_nowhere_else() {
        let video = |k| Capabilities::for_kind(k, true).video.support;
        assert_eq!(video(ConnectionKind::Slack), Support::Yes);
        assert_eq!(video(ConnectionKind::Google), Support::Yes);
        assert_eq!(video(ConnectionKind::Jira), Support::Limited);
        assert_eq!(video(ConnectionKind::Discord), Support::Limited);
        for no in [
            ConnectionKind::Reddit,
            ConnectionKind::Claude,
            ConnectionKind::Teams,
            ConnectionKind::CustomApi,
            ConnectionKind::X,
            ConnectionKind::Bluesky,
            ConnectionKind::Instagram,
        ] {
            assert_eq!(video(no), Support::No, "{no:?}");
        }
        // Claude cannot watch one, on Windows or elsewhere
        assert_eq!(
            Capabilities::for_kind(ConnectionKind::Claude, false)
                .video
                .support,
            Support::No
        );
    }

    #[test]
    fn serialises_in_snake_case_for_the_interface() {
        let json =
            serde_json::to_value(Capabilities::for_kind(ConnectionKind::Reddit, true)).unwrap();
        assert_eq!(json["files"]["support"], "no");
        assert_eq!(json["screenshot"]["support"], "yes");
        assert_eq!(json["video"]["support"], "no");
    }
}
