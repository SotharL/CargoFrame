# Changelog

What each version of CargoFrame changed, newest first. The release notes on GitHub say the same for the
people who use it; why each change was made is in [decisions.md](docs/decisions.md).

## 0.7.3 (in progress)

- *Check this PC* in debug mode: the real tests run on the PC itself, in a report the bug report attaches
- An overflow check of every page in a real browser, in the checks and CI
- Tests that compare the window's copies of the program's lists, and value-finding cases both sides run
- The key steps logged with their time (capture, recording, share), a panic with its thread and backtrace
- This changelog, and a status page that says what is true today
- The windows no longer load the demo's in-memory backend (`memoryApi.ts`): 12 KB less to load, gzipped
- No more `ERROR tauri::manager: asset not found: favicon.ico` each time a window opens on Windows: the pages name an empty icon
- A second sign-in entry that starts CargoFrame again no longer opens Settings at every sign-in; the log names the other program
- The Settings timing is no longer left running when Settings is only brought to the front (a 99-second "usable" was logged on a reload)
- The log says which program started (its path), so two copies on one PC can be told apart
- Jira: the window names the fields Jira requires for the issue type picked that the button does not send, and waits for them
- The builder warns about a block no sharing button sends (what is put in it goes nowhere)
- *Check this PC* shows whenever debug mode is on, with its own heading, and is found by the Settings search
- The settings types and their defaults come from Rust (`ts-rs`, test-only): the window keeps no copy of its own

## 0.7.2 (25 November 2026)

- An update no longer undone at sign-in by an older copy in the machine's startup list
- Debug mode, for reporting a problem in CargoFrame itself
- A value of several parts and grips on a selection
- The Tags list over the blocks below it
- The Columns control, the first tab's corner, the Fill fields form and the New value panel fixed
- Jira people and teams searched by name, a new ticket assigned to you
- The formatted text box fixed for Enter and a paste in the window
- A recording of several screens kept to about the main screen's size
- Tags picked from the service's list
- Copy, paste, duplicate and delete in the layout builder, and locked tabs
- The computer's details as values
- Deleting several items in Files & backup

## 0.7.1 (20 November 2026)

- Updates found again on Windows
- A followed window recorded on its own screen without redrawing its outline as it moves
- Shortcuts of any keys, watched by CargoFrame when the system cannot
- ffmpeg downloaded and updated from System
- Templates that record the window in focus beside its screenshot, with starting texts that use the values and a recording/screenshot table
- Jira and GitHub issues created or updated, each field as its mapping says, with a comment and a vote
- Blocks that must be filled before sharing
- File tabs in Data parsing that close and stay until Settings closes

## 0.7.0

- Settings regrouped: Connections
- Layouts, Shortcuts, Data parsing, Recording
- Preferences, Files & backup, System, each page with an icon
- A tour that starts after an install or an update
- Recording made lighter (the graphics card's capture and a hardware encoder where they work, ffmpeg below normal priority, one conversion)
- The log viewer reads a time selected in a line and follows the clip as it plays
- A file dropped in Data parsing opens, and a long line no longer pushes the panel out of sight
- The wording of the four languages brought to the terms technical tools use

## 0.6.0

- French, Spanish, Italian and German besides English: the computer's language by default and a setting in General, the packs checked on every push

## 0.5.1

- The ready-made layouts brought up to date: one-line titles, `{app.title}`, hashtags for X, Bluesky and Instagram, the time over a recording and its logs from the recording

## 0.5.0

- Values made by selecting them in a file of a capture, read in several files with the newest winning, written `{map}`
- Data & fields as Values and Files, Fill fields a tab of Layouts, `{` and a values preview in the layout builder

## 0.4.1

- The time shown over a recording, the log lines written during it, read with time formats people write or data rules set
- A Tabs block
- The layout round: rows that hold one line, sharing dots, the recording's tools above it as icons, the log viewer opened large in the middle

## 0.4.0

- A Tags block, and where tags go per service
- A log viewer block
- Files added by hand
- Values made from other values
- Continuous recording, held to start and stop
- The keys pressed shown over a recording
- A recording that follows a moved window
- A search in Settings
- Text blocks that keep one line
- The sharing boxes in the block's corner

## 0.3.1 (a fix)

- A Text block's settings did not open in the layout builder once a data rule read a value

## 0.3.0 (the 10 November round)

- A sign while recording, and the focus followed
- Tables and code blocks in the formatted box
- The screenshot and the recording placed in the text for Jira, GitHub and Teams
- The values list beside the starting text
- *Read text* on the screenshot
- Wider layouts, with up to 24 columns
- The video crop and marks reworked
- Text on screenshots with bold, italic and a box
- Several subfolders in a Files block
- The layout builder split in half
- The guides grouped

## Before the numbered versions: 2026-11-09

- Data & fields: read values from files, then fill the service's fields with them by steps; formatting in the layout builder's starting text; a GitHub connector; Data from files as a flow: if a file is found, add it and read values, no format to pick; links to web pages in the guides; a formatted box that shows its formatting, with *Message of* on the Text block
- A shortcut records or takes a screenshot from its layout; fixed: a window whose layout had a dropdown stayed on Loading; X, Bluesky and Instagram against mock servers, one build switch per service, `cargoframe open`; the flow review: Settings grouped by content, General split into General, Your data and About, the duplicate video editor removed from Settings, Linux trusts the system's certificates

## Before the numbered versions: 2026-11-07

- The second 7 November round after the installer test: one path to post with threads, sound recorded from Windows itself, Your data and reset, notifications switch, text formatting, Setup checks, Data from files as a sentence
- The twelve changes of the first 7 November feedback round: Discord webhooks, dependent dropdowns, last choices, get-token buttons, Claude app install, screenshot with video, in-app guides, Connections under Setup
- Phases 8 to 12, including video, merged to `main` in pull request #19
- The settings update (General and Safety, Save locally, layout templates, text and log rules, export and import), pull request #12

## V1 (23 October 2026)

- Merged to `main` from `dev/v1` in pull request #1
