# Connectors: how each one flows, and what to improve

_Version 0.3.0. Proposals for review; "Started" and "Done" items are in the program._

The goal for every service is the same: set up in a few clicks with a guide, pick *where to post* in one box that
goes as deep as the service does, and send without surprises (sizes, required fields, formatting handled for you).

## For every connector

| # | Improvement | Why | State |
|---|-------------|-----|-------|
| 1 | One build switch per service | A company can ship an installer without the services it does not use; their code is not in the program | Started |
| 2 | Pictures made to fit each service's size limit | No failed share because a 4K screenshot is too big | Started (X, Bluesky, Instagram) |
| 3 | A "where to post" path for every service with places (the Discord path, extended) | One box instead of several dropdowns; the last place is remembered | Discord done; others below |
| 4 | Extra fields reworked as *Fill fields* (Settings, Data & fields): steps that fill a field on each share, from values found in files; a block is sent in a field from the sharing button only | One place for each: what a block sends, what fills a field by itself | Done (0.3.0) |
| 5 | Required fields asked by the service shown in the layout before the first send | No "field X is required" error after pressing the button | Proposed (Jira first) |
| 6 | Company sign-in (OAuth with the company's identity provider) | No personal tokens to paste in a large company | Proposed (Jira, Custom API) |
| 7 | Other programs fill a layout (`cargoframe open`) | A company tool can open CargoFrame with its own data in the blocks | Started (text boxes and dropdowns) |
| 8 | Tables and code blocks written the service's way | A repro table or a stack trace reads well in Jira, Teams, GitHub and Reddit; aligned lines in a code block where there are no tables | Done (0.3.0) |
| 9 | The screenshot and the recording placed in the text (`{{capture.screenshot}}`, `{{capture.recording}}`) | A *Repro / Highlight* table with the video and the picture side by side, not two attachments at the end | Done (0.3.0): Jira, GitHub, Teams (picture); proposed: Slack and Discord cannot place files in text |

## Per connector

| Service | How it flows today | Proposed next |
|---------|--------------------|---------------|
| **GitHub** (new) | A fine-grained token; repository, labels, assignees, milestone and open-issue lists; a new issue or a comment (with labels); pictures and files in a branch of the repository, shown in the issue | Issue templates and forms of the repository; projects (boards); sign in with a GitHub App so no token is pasted |
| **Jira** | Address, e-mail and API token (or a personal access token); project, issue type, priority lists; new issue or comment | Path: project, then *New issue* or an existing issue searched by summary (the Discord thread step); required fields of the issue type from the service shown in the layout; OAuth sign-in for Jira Cloud and Data Center |
| **Google** | Your own OAuth client; Drive folder list; optional Sheets row | Path: Drive folder, then sub-folder; for Sheets, spreadsheet then tab; a short guide for making the OAuth client |
| **Slack** | Bot token from an app you make; channel list; thread by pasted link | A *Create the app* link with the permissions filled in (Slack app manifest), so setup is one click and a paste; path: channel, then recent threads |
| **Discord** | Webhook per channel, or a bot; one path down to threads | Name a channel from its webhook by itself; forum tags as a list |
| **Reddit** | Your own app; subreddit and flair lists | Say when a subreddit requires a flair, before sending; title length checked as you type |
| **Teams** | Your own Entra app; chats and channels; thread by link | Path: team, channel, then recent posts; pictures and files through OneDrive so videos and files can go; an admin-consent link to hand to IT |
| **Claude** | Opens a chat and pastes | Pick a Claude project for new chats |
| **Custom API** | Address, sign-in, request template | OAuth 2.0 with your own provider; a *Show the request* preview before sending; import from a `curl` line |
| **X** (new) | Your own X app (Native App, no secret), sign in in the browser; text (280 characters) and the screenshot, picture description | Video through the chunked upload; reply to one of your recent posts (path step) |
| **Bluesky** (new) | Handle and an app password, nothing to register; text (300 characters), the screenshot, picture description, links marked | Video; reply in one of your threads; sign in with Bluesky's OAuth when it is common |
| **Instagram** (new) | Professional account; token from the Meta app dashboard; the picture is put on your own picture server first, then published | A list of common picture servers (S3-compatible, Azure, Bunny) with their address shapes filled in; Reels through the resumable upload; several pictures (carousel) |

## What the new connectors cannot do, and why

- **Instagram takes pictures only from a public web address.** Meta's API has no upload for pictures. CargoFrame
  will not run a public server on people's computers, so the picture goes to a server the person or company
  already owns. Without one, Instagram cannot be used from CargoFrame.
- **X posting needs your own developer app** and may be charged by X per post.
- **GitHub cannot attach a file to an issue** through its API. CargoFrame puts the picture and the zip in a branch of
  the repository (`cargoframe-attachments`, made from the main branch the first time) and shows them in the issue; the
  connection can also send text only.
- **Formatting** (bold, lists) is shown as plain text on X, Bluesky and Instagram: they have none.
- **Pictures in the text** need a service that shows an attached file inside its text: Jira (`!name!`), GitHub (a link
  from the attachments branch) and Teams (an inline picture). Slack, Discord, Reddit and Google attach the files after
  the text, so the two values are left out of their text.
- **Videos** are not sent to X, Bluesky or Instagram yet: the screenshot (or the still of a recording) is.
