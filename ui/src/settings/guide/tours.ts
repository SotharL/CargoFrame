// The in-app guides. Each step points at one thing on a Settings page, named by its `data-tour`
// attribute in the page's code, and says one short thing about it. To change a guide, change
// this file; to rename a field, change its `data-tour` too. tours.test.ts fails when a step
// points at something that no longer exists, so a guide cannot silently go stale.
import type { PageId } from '../nav';

export interface Step {
  /** The `data-tour` name of what to point at. */
  target: string;
  /** One short sentence, shown under the box. */
  say: string;
  /** Go to this page first. */
  page?: PageId;
  /** Next presses the target itself (the button that opens a form, a Save button). */
  click?: boolean;
}

export interface Tour {
  id: string;
  title: string;
  steps: Step[];
}

/** The longest a step may say, in characters. A guide is a nudge, not a manual. */
export const MAX_SAY = 90;

const SETUP: PageId = 'setup';

/** The same ending for the services that need a sign-in in the browser. */
const signIn = (service: string): Step => ({
  target: 'conn.signin',
  say: `Sign in: your browser opens ${service}'s own page.`,
});

export const TOURS: Tour[] = [
  {
    id: 'connect-jira',
    title: 'Connect Jira',
    steps: [
      { target: 'add-jira', say: 'Start here: add a Jira connection.', page: SETUP, click: true },
      { target: 'jira.address', say: 'Your Jira address, like https://acme.atlassian.net.' },
      { target: 'jira.auth', say: 'Jira Cloud uses e-mail and a token. Server uses a personal token.' },
      { target: 'jira.email', say: 'The e-mail you sign in to Jira with.' },
      { target: 'conn.save', say: 'Save. The token box appears under the form.', click: true },
      { target: 'conn.secret', say: 'Paste the token. The button beside it opens the page that makes one.' },
      { target: 'conn.done', say: 'Press Done, then Test on the card.' },
    ],
  },
  {
    id: 'connect-google',
    title: 'Connect Google Drive and Sheets',
    steps: [
      { target: 'add-google', say: 'Start here: add a Google connection.', page: SETUP, click: true },
      { target: 'google.client_id', say: 'Paste your Client ID. The button opens the page where you make one.' },
      { target: 'conn.save', say: 'Save. A box for the client secret appears.', click: true },
      { target: 'conn.secret', say: 'Paste the client secret from the same page.' },
      { target: 'conn.done', say: 'Press Done.' },
      signIn('Google'),
    ],
  },
  {
    id: 'connect-reddit',
    title: 'Connect Reddit',
    steps: [
      { target: 'add-reddit', say: 'Start here: add a Reddit connection.', page: SETUP, click: true },
      { target: 'reddit.client_id', say: 'Paste the app id. The button opens the page where you register one.' },
      { target: 'conn.save', say: 'Save.', click: true },
      signIn('Reddit'),
    ],
  },
  {
    id: 'connect-claude',
    title: 'Connect Claude',
    steps: [
      { target: 'add-claude', say: 'Start here: add a Claude connection.', page: SETUP, click: true },
      { target: 'claude.open_with', say: 'Where chats open. The desktop app is the smoother way.' },
      { target: 'conn.save', say: 'Save. There is no token to paste.', click: true },
    ],
  },
  {
    id: 'connect-slack',
    title: 'Connect Slack',
    steps: [
      { target: 'add-slack', say: 'Start here: add a Slack connection.', page: SETUP, click: true },
      { target: 'slack.steps', say: 'These steps make the bot token you need.' },
      { target: 'conn.save', say: 'Save. The token box appears under the form.', click: true },
      { target: 'conn.secret', say: 'Paste the bot token (it starts with xoxb-).' },
      { target: 'conn.done', say: 'Press Done. In each channel, type /invite @ and the app name.' },
    ],
  },
  {
    id: 'connect-discord',
    title: 'Connect Discord',
    steps: [
      { target: 'add-discord', say: 'Start here: add a Discord connection.', page: SETUP, click: true },
      { target: 'discord.mode', say: 'Webhook is the easy way: no bot needed.' },
      { target: 'discord.add-channel', say: 'Add a channel to post in.', click: true },
      { target: 'discord.channels', say: 'Give it a name, like bugs. Add one for each channel you use.' },
      { target: 'conn.save', say: 'Save. A link box appears for each channel.', click: true },
      { target: 'conn.secret', say: "Paste the channel's webhook link. The button shows how to make one." },
      { target: 'conn.done', say: 'Press Done.' },
      { target: 'layouts.templates', say: 'Then start a layout from a template.', page: 'layouts', click: true },
      { target: 'template-discord-message', say: 'This one lists only threads named like your Title.' },
    ],
  },
  {
    id: 'connect-teams',
    title: 'Connect Microsoft Teams',
    steps: [
      { target: 'add-teams', say: 'Start here: add a Teams connection.', page: SETUP, click: true },
      { target: 'teams.steps', say: 'These steps register the small app Teams needs.' },
      { target: 'teams.client_id', say: 'Paste its Application (client) ID. The button opens the page.' },
      { target: 'conn.save', say: 'Save.', click: true },
      signIn('Microsoft'),
    ],
  },
  {
    id: 'connect-x',
    title: 'Connect X',
    steps: [
      { target: 'add-x', say: 'Start here: add an X connection.', page: SETUP, click: true },
      { target: 'x.steps', say: 'These steps make the small app X needs.' },
      { target: 'x.client_id', say: 'Paste its OAuth 2.0 Client ID. The button opens the portal.' },
      { target: 'conn.save', say: 'Save.', click: true },
      signIn('X'),
    ],
  },
  {
    id: 'connect-bluesky',
    title: 'Connect Bluesky',
    steps: [
      { target: 'add-bluesky', say: 'Start here: add a Bluesky connection.', page: SETUP, click: true },
      { target: 'bluesky.handle', say: 'Your handle, like jane.bsky.social.' },
      { target: 'conn.save', say: 'Save. A box for the app password appears.', click: true },
      { target: 'conn.secret', say: 'Paste an app password. The button opens the page that makes one.' },
      { target: 'conn.done', say: 'Press Done, then Test on the card.' },
    ],
  },
  {
    id: 'connect-instagram',
    title: 'Connect Instagram',
    steps: [
      { target: 'add-instagram', say: 'Start here: add an Instagram connection.', page: SETUP, click: true },
      { target: 'instagram.upload', say: 'Where CargoFrame puts the picture first, on your own server.' },
      { target: 'instagram.public', say: 'The public address of that file. Instagram fetches it there.' },
      { target: 'conn.save', say: 'Save. The token box appears under the form.', click: true },
      { target: 'conn.secret', say: 'Paste the token made in your Meta app (Generate token).' },
      { target: 'conn.done', say: 'Press Done, then Test on the card.' },
    ],
  },
  {
    id: 'connect-custom',
    title: 'Connect your own API',
    steps: [
      { target: 'add-custom', say: 'Start here: add a custom connection.', page: SETUP, click: true },
      { target: 'custom.url', say: 'The address your API lives at. It must start with https://.' },
      { target: 'custom.auth', say: 'How your API wants you to sign in.' },
      { target: 'conn.save', say: 'Save.', click: true },
    ],
  },
  {
    id: 'first-capture',
    title: 'Make your first capture',
    steps: [
      { target: 'layouts.templates', say: 'A layout is the window a capture opens. Start from a template.', page: 'layouts', click: true },
      { target: 'template-local-capture', say: 'This one needs no account: it saves a zip on this computer.', click: true },
      { target: 'layouts.save', say: 'Save it as your own layout.', click: true },
      { target: 'triggers.add', say: 'Now a trigger: a shortcut that opens the layout.', page: 'triggers', click: true },
      { target: 'triggers.shortcut', say: 'Click the box, then press the keys you want.' },
      { target: 'triggers.layout', say: 'Pick the layout you just saved.' },
      { target: 'triggers.save', say: 'Save. Press the shortcut anywhere to try it.' },
    ],
  },
  {
    id: 'data-from-files',
    title: 'Add a value from a file',
    steps: [
      { target: 'rules.add', say: 'Read a value, like a version, from a file in the project.', page: 'rules', click: true },
      { target: 'rules.name', say: 'Its name. Write {{rule.name}} in a text block to use it.' },
      { target: 'rules.file', say: 'The file, from the project folder: package.json, say.' },
      { target: 'rules.format', say: 'How to read it: JSON, text, a pattern.' },
      { target: 'rules.query', say: 'Where the value is: a JSON key, or the text around it.' },
      { target: 'rules.save', say: 'Save. The preview shows what it finds.' },
    ],
  },
  {
    id: 'destination-fields',
    title: 'Fill a field on the service (like Priority)',
    steps: [
      { target: 'infotypes.add', say: 'An extra field is a box on the service beyond where to post, like Jira Priority.', page: 'infotypes', click: true },
      { target: 'infotypes.name', say: 'What you call it.' },
      { target: 'infotypes.connection', say: 'The connection it belongs to.' },
      { target: 'infotypes.key', say: "Its key on the service. Search the ones it offers." },
      { target: 'infotypes.kind', say: 'Text, or a choice from a list.' },
      { target: 'infotypes.save', say: 'Save. Then map it to a block in your layout.' },
    ],
  },
];

export const tourById = (id: string): Tour | undefined => TOURS.find((t) => t.id === id);

/** The CSS selector for a step's target. */
export const selectorOf = (target: string): string => `[data-tour="${target}"]`;
