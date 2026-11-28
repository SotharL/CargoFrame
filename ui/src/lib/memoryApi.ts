/**
 * The in-memory backend: what answers the windows outside the app (`npm run dev`, the tests, the screenshots and
 * the overflow check), with the demo's data. Loaded only there: in the app, `getApi` never imports this file.
 */
import type {
  ChatView, Cleaned, Connection, ConnectionView, DebugSetup, DebugView, Describe, DropdownOption, FieldSpec, General,
  ImportPreview, ItemEntry, ItemKind, Layout, LayoutTemplate, LookFolder, Rule, Safe, FfmpegPhase, Reach,
  ShareRequest, ShareResult, UpdateLast, UpdateView, VariableInfo, Video, SetupReport, SetupItem, SetupInstall,
  ServiceKind, TextReading, FileEntry, LogText, PcCheckStep,
} from './types';
import type { Api } from './ipc';
import { LANGUAGES, resolve as resolveLanguage } from './i18n/index.svelte';
import { DEMO_SAMPLES, demoValues } from './demoSamples';
import { demoCapabilities, demoNeeds, demoReach, LOCAL_CAPABILITIES } from './demoReach';
import { KIND_NAME } from './connector';
import { valuesOf } from '../settings/ruleValues';
import { defaultGeneral, defaultSafe, defaultVideo } from './defaults';

/** Options for the in-memory stand-in: what a share and a list request should do. */
export type SetupScenario =
  | 'ready'
  | 'windows_ready'
  | 'windows_missing'
  | 'windows_no_winget'
  | 'windows_no_output'
  | 'linux_missing';

export interface MemoryOptions {
  general?: Partial<General>;
  /** Starting video settings. */
  video?: Partial<Video>;
  /** The stand-in finds no ffmpeg. */
  noFfmpeg?: boolean;
  /** The services the stand-in build has (all of them by default). */
  connectors?: ServiceKind[];
  /** The stand-in's test recording fails. */
  recordingFails?: boolean;
  /** The stand-in is a Windows PC (it can record one program's sound, and has no "Stereo Mix"). */
  windows?: boolean;
  /** The stand-in finds no sound devices at all. */
  noAudio?: boolean;
  /** What the Setup page finds: everything there (the default), or one of the ways it is not. */
  setup?: SetupScenario;
  /** What the computer looks like from the second check on (a device was plugged in, say). */
  setupThen?: SetupScenario;
  /** The stand-in's install does not work. */
  installFails?: boolean;
  /** Windows stand-in: whether the Claude desktop app is installed (default: it is). */
  claudeApp?: 'installed' | 'missing';
  /** How many times the stand-in's install is asked about before it is done. */
  installPolls?: number;
  /** Called by `share`; the default answers with a made-up success. */
  share?: (request: ShareRequest) => ShareResult | Promise<ShareResult>;
  /** Called by `addFiles`; the default adds one made-up file. */
  addFiles?: () => FileEntry[];
  /** Called by `readOfferedFile`; the default gives a short made-up log. */
  readOfferedFile?: (path: string) => LogText;
  /** Called by `readText`; the default reads one made-up line. */
  readText?: (attempt: number) => TextReading | Promise<TextReading>;
  /** Called by `fetchOptions` (as `<block>:<choice>` for the list inside a choice) and `previewOptions`. */
  options?: (connectionId: string, list: string) => DropdownOption[] | Promise<DropdownOption[]>;
  /** The GitHub connections debug mode's setup offers. Default: one. */
  github?: [string, string][];
  /** Collects what the windows report in debug mode, for tests. */
  reported?: DebugView[];
  /** Older copies Windows starts at sign-in, for Preferences. Default: none. */
  autostartOthers?: string[];
  /** What `openImport` finds in the "file" the person picks; the default is a cancelled dialog. */
  importFile?: ImportPreview;
  /** What "Check for updates" finds in the stand-in. Default: nothing newer. */
  update?: {
    /** A newer release. */
    newer?: { version: string; notes?: string; size?: number };
    /** The check fails with this message. */
    fails?: { message: string; needs_token?: boolean };
  };
}

// the same as core/layout/src/templates.rs (TEMPLATES), in the same order
const TEMPLATE_CATALOG: LayoutTemplate[] = [
  { key: 'local-capture', name: 'Save a capture locally', description: 'The screenshot, a note and the files you tick, saved as a zip in your folder for saved zips. A Logs tab beside the screenshot reads the logs. No account needed.', service: null },
  { key: 'local-video', name: 'Save a recording locally', description: 'The recording of the window in focus, the screenshot taken from it, a note and the files you tick, saved as a zip in your folder for saved zips. A shortcut that opens it records the window in focus. No account needed.', service: null },
  { key: 'jira-ticket', name: 'Jira ticket', description: 'Creates a ticket assigned to you (or to whoever you pick), or updates the one you pick in Ticket to update: the summary and the assignee stay, the description goes below the one there and the labels are added. An optional comment goes with either. The screenshot and the ticked files are attached, and a Logs tab beside the screenshot reads the logs.', service: 'jira' },
  { key: 'jira-video', name: 'Jira ticket with recording', description: 'Like the Jira ticket, with the recording of the window in focus attached; tabs hold the recording (with the time of each moment over it), the screenshot taken from it and the logs, starting on the lines written while it recorded. A shortcut that opens it records the window in focus.', service: 'jira' },
  { key: 'github-issue', name: 'GitHub issue', description: 'Opens an issue in a repository, or updates the one you pick in Issue to update: the title stays, the description goes below the one there and the labels are added. An optional comment goes with either. The screenshot and the ticked files are shown in it, and a Logs tab beside the screenshot reads the logs.', service: 'github' },
  { key: 'github-video', name: 'GitHub issue with recording', description: 'Like the GitHub issue, with the recording of the window in focus linked from it beside the screenshot taken from it; tabs hold the recording, the screenshot and the logs. A shortcut that opens it records the window in focus.', service: 'github' },
  { key: 'cargoframe-report', name: 'CargoFrame bug report', description: 'Debug mode: a GitHub issue about CargoFrame itself, from a capture of one of its windows. The title starts with where it was (window, page, tab, section), the description says what was in focus, and the layout on screen, the data rules, the settings and the end of the log are attached.', service: 'github' },
  { key: 'google-drive', name: 'Google Drive upload', description: 'Upload the screenshot and a zip of the ticked files to a Drive folder you pick. A Logs tab beside the screenshot reads the logs.', service: 'google' },
  { key: 'google-drive-video', name: 'Google Drive upload of a recording', description: 'Upload the recording of the window in focus, the screenshot taken from it and a zip of the ticked files to a Drive folder you pick. A shortcut that opens it records the window in focus.', service: 'google' },
  { key: 'google-sheet', name: 'Google Sheet row', description: 'Add a row to a spreadsheet: the date, two columns you fill, and links to the uploads.', service: 'google' },
  { key: 'reddit-post', name: 'Reddit post', description: 'Post the screenshot to a subreddit you follow, with a title and text.', service: 'reddit' },
  { key: 'claude-ask', name: 'Ask Claude about a capture', description: 'Write a prompt, choose a new or existing chat, and hand the screenshot and prompt to Claude. A Logs tab beside the screenshot reads the logs, to copy lines into the prompt.', service: 'claude' },
  { key: 'slack-message', name: 'Slack message', description: 'Post a message with the screenshot and a zip of the ticked files into a channel the app is in, or reply in a thread. A Logs tab beside the screenshot reads the logs.', service: 'slack' },
  { key: 'slack-video', name: 'Slack message with a recording', description: 'Post a message with the recording of the window in focus and a zip of the ticked files into a channel the app is in, or reply in a thread; tabs hold the recording (with the time of each moment over it), the screenshot taken from it and the logs, starting on the lines written while it recorded. A shortcut that opens it records the window in focus.', service: 'slack' },
  { key: 'discord-message', name: 'Discord message', description: 'Post a message with the screenshot and a zip of the ticked files into a channel, or start a post in a forum. Nobody is pinged. A Logs tab beside the screenshot reads the logs.', service: 'discord' },
  { key: 'discord-video', name: 'Discord message with a recording', description: 'Post a message with the recording of the window in focus and a zip of the ticked files into a channel, or start a post in a forum, within the server\'s upload limit; tabs hold the recording, the screenshot taken from it and the logs. A shortcut that opens it records the window in focus.', service: 'discord' },
  { key: 'teams-message', name: 'Teams message', description: 'Post a message with the screenshot inside it into a chat, a channel or a channel thread, as you.', service: 'teams' },
  { key: 'x-post', name: 'X post', description: 'Post the screenshot to your X account with a short text (280 characters), hashtags and a description of the picture.', service: 'x' },
  { key: 'bluesky-post', name: 'Bluesky post', description: 'Post the screenshot to your Bluesky account with a short text (300 characters), hashtags and a description of the picture.', service: 'bluesky' },
  { key: 'instagram-post', name: 'Instagram post', description: 'Post the screenshot to your Instagram professional account with a caption and hashtags.', service: 'instagram' },
  { key: 'custom-post', name: 'Custom API post', description: 'A title and a message with the screenshot and files, sent through the request you described.', service: 'custom' },
];

/** A small stand-in for the backend's template: a screenshot, a note and a button. */
function templateLayout(t: LayoutTemplate, connectionId: string | null): Layout {
  return {
    id: '',
    name: t.name,
    blocks: [
      t.key.endsWith('-video')
        ? { id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 7, h: 7 } }
        : { id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 7, h: 7 } },
      { id: 'note', type: 'text', rect: { x: 7, y: 0, w: 5, h: 3 }, title: 'Note', placement: 'above', role: 'plain', default_text: '{app.title}' },
      {
        id: 'send', type: 'sharing_button', rect: { x: 7, y: 6, w: 5, h: 1 },
        label: t.service ? 'Send' : 'Save locally', connection_id: connectionId,
        prompt_block_id: null, mappings: [], on_success: 'close_window',
      },
    ],
  };
}

const DEMO_LANGUAGE = 'cargoframe.demo.language';

export function memoryApi(opts: MemoryOptions = {}): Api {
  const initial = opts.general;
  let general: General = { ...defaultGeneral(), ...initial };
  // the preview in a browser keeps its language across reloads (the app keeps it in its settings)
  if (!initial?.language) {
    try {
      const saved = globalThis.localStorage?.getItem(DEMO_LANGUAGE);
      if (saved) general.language = saved;
    } catch {
      // no storage here: the computer's language
    }
  }
  // what the Your data section finds, and clears (sizes in bytes)
  let kept: Record<string, { files: number; bytes: number }> = {
    work: { files: 4, bytes: 5_300_000 }, kept: { files: 0, bytes: 0 }, logs: { files: 3, bytes: 120_000 }, updates: { files: 0, bytes: 0 },
  };
  let autostart = true;
  const reported: DebugView[] = (opts.reported ??= []);
  let debugSetup: DebugSetup = { github: opts.github ?? [['gh', 'CargoFrame on GitHub']], layout: false, shortcut: null };
  let autostartOthers: string[] = [...(opts.autostartOthers ?? [])];
  let safe: Safe = defaultSafe();
  let video: Video = { ...defaultVideo(), ...opts.video };
  let trialRan = false;
  // the Setup page's stand-in: what is installed changes only when the install button is pressed
  let scenario: SetupScenario = opts.setup ?? 'ready';
  let setupChecks = 0;
  const hasFfmpeg = (s: SetupScenario) => s === 'ready' || s === 'windows_ready' || s === 'windows_no_output';
  let installed = hasFfmpeg(scenario);
  let claudeInstalled = opts.claudeApp !== 'missing';
  let installLeft = 0;
  let installFailed = false;
  let soundSettingsOpened = 0;
  const setupReport = (): SetupReport => {
    const windows = scenario.startsWith('windows');
    const ffmpeg: SetupItem = installed
      ? { id: 'ffmpeg', name: 'ffmpeg', what_for: 'Recording video, and the sound in it. Screenshots and sharing do not need it.', status: 'ok', detail: windows ? 'Version 7.1, C:\\Tools\\ffmpeg\\bin\\ffmpeg.exe' : 'Version 6.1, /usr/bin/ffmpeg', action: null }
      : { id: 'ffmpeg', name: 'ffmpeg', what_for: 'Recording video, and the sound in it. Screenshots and sharing do not need it.', status: 'missing', detail: 'ffmpeg was not found.', action: null };
    const items: SetupItem[] = [ffmpeg];
    let install: SetupInstall = { type: 'none' };
    const output = (status: SetupItem['status'], detail: string, action: SetupItem['action'] = null): SetupItem => ({
      id: 'sound_output', name: 'Sound output',
      what_for: 'The speakers or headphones this computer plays on. A video records what it plays.',
      status, detail, action,
    });
    if (windows) {
      if (scenario === 'windows_no_output') {
        items.push(output('missing', 'Windows has no default playback device: no speakers or headphones are set, or none is plugged in. Plug one in, or choose one in the Windows sound settings (the button).', 'sound_settings'));
      } else {
        items.push(output('ok', 'Speakers (Realtek(R) Audio). Videos record this sound.', 'sound_settings'));
      }
      // a Claude connection that opens the desktop app makes the app a line of its own
      const opensApp = [...store.connection.values()].some(
        (c) => (c as Connection).kind === 'claude' && (c as Connection & { open_with: string }).open_with !== 'browser',
      );
      if (opensApp) {
        items.push({
          id: 'claude_app', name: 'Claude desktop app',
          what_for: 'Where a Claude connection opens your chat. Without it, set that connection to open in the browser.',
          status: claudeInstalled ? 'ok' : 'missing', detail: claudeInstalled ? 'Installed.' : 'Not installed.', action: null,
        });
      }
      const claudeMissing = opensApp && !claudeInstalled;
      if (!installed || claudeMissing) {
        install = scenario === 'windows_no_winget'
          ? { type: 'manual', text: 'Windows\' package manager (winget) was not found. Install "App Installer" from the Microsoft Store, then come back; or download ffmpeg yourself and choose its folder under Recording.' }
          : { type: 'run', label: !installed && claudeMissing ? 'Install what is missing' : installed ? 'Install the Claude app' : 'Install ffmpeg' };
      }
    } else {
      items.push(output('ok', 'Built-in Audio Analog Stereo. Videos record this sound.'));
      items.push({ id: 'xdotool', name: 'xdotool', what_for: 'Lets CargoFrame press Ctrl+V and Enter in a Claude chat for you (X11 only).', status: installed ? 'ok' : 'missing', detail: installed ? 'Installed.' : 'Not installed.', action: null });
      if (!installed) install = { type: 'copy', command: 'sudo apt install ffmpeg xdotool' };
    }
    if (installLeft > 0) install = { type: 'running' };
    else if (installFailed && !(installed && claudeInstalled)) install = { type: 'failed', message: 'winget said: No network connection.' };
    return { os: windows ? 'windows' : 'linux', items, missing: items.filter((i) => i.status === 'missing').length, install };
  };
  const store: Record<ItemKind, Map<string, unknown>> = {
    trigger: new Map(),
    layout: new Map(),
    rule: new Map(),
    infotype: new Map(),
    connection: new Map(),
  };
  // Credentials the in-memory stand-in "stores": which secrets are set, and signed-in ids.
  const secretsSet = new Set<string>();
  const signedIn = new Set<string>();
  let chats: ChatView[] = [];
  let updateLast: UpdateLast | null = null;
  let updatePhase: UpdateView['phase'] = { state: 'idle' };
  let updateToken = false;
  const updateListeners = new Set<() => void>();
  let ffmpegPhase: FfmpegPhase = { state: 'idle' };
  const ffmpegListeners = new Set<() => void>();
  const updateView = (): UpdateView => ({
    current: '0.1.0-dev',
    repository: 'SotharL/CargoFrame',
    supported: true,
    token_set: updateToken,
    token_problem: null,
    last: updateLast,
    phase: updatePhase,
  });
  const TYPED: Record<string, string[]> = {
    jira: ['token'], google: ['client_secret'], reddit: [], claude: [], slack: ['token'], teams: [], custom: ['secret'],
    x: [], bluesky: ['app_password'], instagram: ['token', 'host_key'], github: ['token'],
    discord: ['token', ...Array.from({ length: 10 }, (_, i) => `hook${i}`)],
  };
  const OAUTH = new Set(['google', 'reddit', 'teams', 'x']);
  /** The demo's description of a connection, each field saying whether the service needs it. */
  const describeFor = (c: Connection): Describe => {
    const d = describeRaw(c);
    const needs = demoNeeds(c.kind);
    return {
      ...d,
      fields: d.fields.map((f) => {
        const n = needs.find((x) => x.field === f.key);
        return { ...f, required: n !== undefined, unless: n?.unless ?? null };
      }),
    };
  };
  const describeRaw = (c: Connection): Omit<Describe, 'fields'> & { fields: Omit<FieldSpec, 'required' | 'unless'>[] } => {
    if (c.kind === 'jira') {
      return {
        summary: 'Creates an issue, or comments on one.',
        fields: [
          { key: 'project', label: 'Project', hint: 'Project key', list: 'projects' },
          { key: 'issuetype', label: 'Issue type', hint: '', list: 'issue_types' },
          { key: 'summary', label: 'Summary', hint: 'The issue title', list: null },
          { key: 'description', label: 'Description', hint: '', list: null },
          { key: 'priority', label: 'Priority', hint: '', list: 'priorities' },
          { key: 'issue', label: 'Existing issue', hint: 'Comment instead of creating', list: 'recent_issues' },
        ],
        lists: [
          { id: 'projects', label: 'Projects', param: null },
          { id: 'issue_types', label: 'Issue types', param: null },
          { id: 'priorities', label: 'Priorities', param: null },
          { id: 'components', label: 'Components of a project', param: 'project key' },
        ],
      };
    }
    if (c.kind === 'teams') {
      return {
        summary: 'Posts a message as you into a chat, a channel or a channel thread, with the screenshot inside it. The ticked files cannot be sent yet: Teams needs them in OneDrive.',
        fields: [
          { key: 'target', label: 'Chat or channel', hint: 'Where to post. You must be a member of it', list: 'targets' },
          { key: 'thread', label: 'Reply in a channel thread', hint: 'A message link (Copy link in Teams) in a channel', list: null },
          { key: 'message', label: 'Message', hint: 'Defaults to the prompt text', list: null },
          { key: 'title', label: 'Headline', hint: 'The headline of a new channel post, or a bold first line', list: null },
        ],
        lists: [{ id: 'targets', label: 'Your chats and the channels of your teams', param: null }],
      };
    }
    if (c.kind === 'discord') {
      return {
        summary: 'Posts a message into a channel or thread, or starts a forum post, with the screenshot and a zip of the ticked files attached. Nobody is pinged.',
        fields: [
          { key: 'channel', label: 'Channel', hint: 'A text channel, or a forum to start a post in', list: 'channels' },
          { key: 'thread', label: 'Thread or forum post', hint: 'A thread of the channel; leave empty to start a new one', list: 'threads' },
          { key: 'message', label: 'Message', hint: 'Defaults to the prompt text', list: null },
          { key: 'title', label: 'Title', hint: 'The name of a forum post', list: null },
        ],
        lists: [
          {
            id: 'channels', label: 'Channels and forums the bot can see', param: null,
            inside: { list: 'threads', field: 'thread', label: 'Thread or post' },
          },
          { id: 'threads', label: 'Active threads and forum posts', param: null },
        ],
      };
    }
    if (c.kind === 'slack') {
      return {
        summary: 'Posts a message into a channel or a thread, with the screenshot and a zip of the ticked files attached.',
        fields: [
          { key: 'channel', label: 'Channel', hint: 'The app must have been invited to it', list: 'channels' },
          { key: 'thread', label: 'Reply in a thread', hint: 'A message link or its timestamp', list: null },
          { key: 'message', label: 'Message', hint: 'Defaults to the prompt text', list: null },
          { key: 'title', label: 'Bold first line', hint: '', list: null },
        ],
        lists: [{ id: 'channels', label: 'Channels the app can see', param: null }],
      };
    }
    if (c.kind === 'github') {
      return {
        summary: `Opens an issue in ${c.repository || 'the repository'}, with the screenshot shown in it.`,
        fields: [
          { key: 'repo', label: 'Repository', hint: "owner/name. Defaults to the connection's", list: 'repositories' },
          { key: 'title', label: 'Title', hint: 'One line. Defaults to the window title', list: null },
          { key: 'body', label: 'Description', hint: 'Defaults to the prompt text', list: null },
          { key: 'labels', label: 'Labels', hint: 'Separate several with commas', list: 'labels' },
          { key: 'assignees', label: 'Assignees', hint: 'GitHub user names. Separate several with commas', list: 'assignees' },
          { key: 'milestone', label: 'Milestone', hint: '', list: 'milestones' },
          { key: 'issue', label: 'Add to an existing issue', hint: 'An issue number: adds a comment instead', list: 'recent_issues' },
        ],
        lists: [
          { id: 'repositories', label: 'My repositories', param: null },
          { id: 'labels', label: 'Labels of the repository', param: null },
          { id: 'assignees', label: 'People who can be assigned', param: null },
          { id: 'milestones', label: 'Open milestones', param: null },
          { id: 'recent_issues', label: 'Open issues, recently updated', param: null },
        ],
      };
    }
    if (c.kind === 'x' || c.kind === 'bluesky') {
      const most = c.kind === 'x' ? 280 : 300;
      return {
        summary: `Posts the text and the screenshot to your ${KIND_NAME[c.kind]} account.`,
        fields: [
          { key: 'text', label: 'Text', hint: `The post, at most ${most} characters`, list: null },
          { key: 'alt', label: 'Picture description', hint: 'Read out to people who cannot see the picture', list: null },
        ],
        lists: [],
      };
    }
    if (c.kind === 'instagram') {
      return {
        summary: 'Posts the screenshot with a caption to your Instagram professional account.',
        fields: [{ key: 'caption', label: 'Caption', hint: 'Up to 2,200 characters', list: null }],
        lists: [],
      };
    }
    if (c.kind === 'claude') {
      return {
        summary: 'Copies the screenshot, then opens Claude. The prompt is required.',
        fields: [{ key: 'chat', label: 'Chat', hint: 'A chat id, or "new"', list: null }],
        lists: [],
      };
    }
    return { summary: '', fields: [], lists: [] };
  };
  const stateOf = (id: string, c: Connection): ConnectionView['state'] => {
    if (!c.enabled) return 'disabled';
    if (c.kind === 'claude') return 'connected';
    if (c.kind === 'custom' && c.auth.type === 'none') return 'connected';
    if (OAUTH.has(c.kind)) return signedIn.has(id) ? 'connected' : 'disconnected';
    if (c.kind === 'discord' && c.mode === 'webhook') {
      return c.hooks.some((h) => secretsSet.has(`${id}/hook${h.slot}`)) ? 'connected' : 'disconnected';
    }
    return secretsSet.has(`${id}/${TYPED[c.kind]![0]}`) ? 'connected' : 'disconnected';
  };
  const DEMO_SYSTEM: Record<string, string> = {
    'system.os': 'Windows 11 Pro 24H2 (26100)',
    'system.cpu': 'AMD Ryzen 7 7800X3D 8-Core Processor (16 threads)',
    'system.ram': '32 GB',
    'system.gpu': 'NVIDIA GeForce RTX 4070',
    'system.gpu_driver': '32.0.15.6094',
    'system.screens': '3 screens: 2560×1440 (main), 1920×1080, 1920×1080',
    'system.computer': 'DESK-042',
  };
  const BUILTINS: VariableInfo[] = [
    ['app.exe', 'File name of the program in focus'],
    ['app.path', 'Full path of that program'],
    ['app.title', 'The title of its window'],
    ['project.folder', 'The project folder found from the program\'s path'],
    ['project.name', 'That folder\'s name'],
    ['capture.date', 'Date of the capture (YYYY-MM-DD)'],
    ['capture.time', 'Time of the capture (HH:MM:SS)'],
    ['capture.screenshot', 'The screenshot, shown here in the text (Jira, GitHub, Teams)'],
    ['capture.recording', 'The recording, shown here in the text (Jira, GitHub)'],
    ['system.os', 'The system and its version'],
    ['system.cpu', 'The processor'],
    ['system.ram', 'The memory'],
    ['system.gpu', 'The graphics cards'],
    ['system.gpu_driver', 'The versions of their drivers'],
    ['system.screens', 'The screens and their sizes, the main one first'],
    ['system.computer', 'The computer\'s name'],
  ].map(([name, description]) => ({ name: name!, description: description!, ...(DEMO_SYSTEM[name!] ? { value: DEMO_SYSTEM[name!] } : {}) }));
  return {
    appInfo: async () => ({
      version: '0.1.0-dev',
      os: 'linux',
      config_dir: '~/.config/CargoFrame',
      data_dir: '~/.local/share/CargoFrame',
      connectors: opts.connectors ?? ['jira', 'google', 'reddit', 'claude', 'slack', 'discord', 'teams', 'x', 'bluesky', 'instagram', 'github', 'custom'],
      tray_available: true,
    }),
    getGeneral: async () => ({ ...general }),
    getLanguage: async () => {
      const system = typeof navigator === 'undefined' ? null : navigator.language;
      return {
        code: resolveLanguage(general.language, system ?? undefined),
        system,
        languages: LANGUAGES.map((l) => [l.code, l.name] as [string, string]),
      };
    },
    setGeneral: async (g) => {
      general = { ...g };
      try {
        if (g.language) globalThis.localStorage?.setItem(DEMO_LANGUAGE, g.language);
      } catch {
        // the language lasts until the page is reloaded
      }
    },
    getAutostart: async () => autostart,
    reportView: async (view) => {
      reported.push(view);
    },
    debugSetupStatus: async () => ({ ...debugSetup }),
    setupDebugReport: async (connectionId) => {
      const github = debugSetup.github;
      if (!github.length) throw new Error('Add a GitHub connection first (Connections): the report is a GitHub issue.');
      if (!connectionId && github.length > 1) throw new Error('Choose the GitHub connection the reports go to');
      debugSetup = { ...debugSetup, layout: true, shortcut: debugSetup.shortcut ?? 'Ctrl+Alt+Shift+F12' };
      return { ...debugSetup };
    },
    checkThisPc: async () => {
      const steps: PcCheckStep[] = [
        { id: 'system', name: 'This computer', outcome: 'ok', detail: 'Windows 11 Pro 24H2 · AMD Ryzen 7 · 32 GB · NVIDIA GeForce RTX 4070 (32.0.15.6094) · 3 screens', ms: 0 },
        { id: 'screenshot', name: 'A screenshot of every screen', outcome: 'ok', detail: '5760 × 1080 pixels', ms: 48 },
        { id: 'ffmpeg', name: 'ffmpeg', outcome: 'ok', detail: 'C:/Users/Ana/AppData/Local/CargoFrame/tools/ffmpeg/bin/ffmpeg.exe', ms: 120 },
        { id: 'recording', name: 'A 3-second recording', outcome: 'ok', detail: 'Recorded 3.0 s at 3324×622 with the graphics card and NVIDIA. (3412 ms, 912 KB)', ms: 3412 },
        { id: 'sign_in', name: 'Starting when you sign in', outcome: 'warn', detail: 'Another copy starts at sign-in: C:/dev/CargoFrame/target/release/cargoframe.exe', ms: 1 },
        { id: 'credentials', name: 'The credential store', outcome: 'ok', detail: 'A token is kept and read back', ms: 12 },
      ];
      return { steps, report: '# CargoFrame check of this PC\n', saved: 'C:/Users/Ana/AppData/Local/CargoFrame/logs/pc-check.md' };
    },
    setAutostart: async (enabled) => {
      autostart = enabled;
    },
    autostartOthers: async () => [...autostartOthers],
    removeAutostartOthers: async () => {
      autostartOthers = [];
      return [];
    },
    openConfigFolder: async () => {},
    getDataReport: async () => ({
      config_dir: '~/.config/CargoFrame',
      data_dir: '~/.local/share/CargoFrame/data',
      areas: [
        { id: 'work', label: 'Captures and recordings in progress', what: 'Kept for a little while so a window can use them, then removed by themselves.' },
        { id: 'kept', label: 'Zips kept here', what: 'Saved here when the folder for saved zips could not be used.' },
        { id: 'logs', label: 'Log files', what: 'What CargoFrame wrote about itself, for finding a problem.' },
        { id: 'updates', label: 'Downloaded updates', what: 'Installers downloaded by an update, removed after the install.' },
      ].map((a) => ({ ...a, ...kept[a.id]! })),
    }),
    cleanData: async (areas) => {
      const done: Cleaned = { files: 0, bytes: 0, left: 0 };
      for (const id of areas) {
        const k = kept[id];
        if (!k) continue;
        done.files += k.files;
        done.bytes += k.bytes;
        kept[id] = { files: 0, bytes: 0 };
      }
      return done;
    },
    resetEverything: async () => {
      const done: Cleaned = { files: 12, bytes: 40_000, left: 0 };
      for (const id of Object.keys(kept)) {
        done.files += kept[id]!.files;
        done.bytes += kept[id]!.bytes;
        kept[id] = { files: 0, bytes: 0 };
      }
      general = { ...general, notifications: { routine: true, problems: true } };
      return done;
    },
    getSession: async () => (await import('../capture/demo')).demoSession(),
    getSessionImage: async () => (await import('../capture/demo')).demoImage(),
    getSessionClip: async () => new Blob([new Uint8Array(16)], { type: 'video/mp4' }),
    closeCapture: async () => {},
    editLayout: async () => {},
    onSessionChanged: async () => () => {},
    openSettings: async () => {},
    onNavigate: async () => () => {},
    listItems: async <T,>(kind: ItemKind) => ({
      items: [...store[kind].entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([id, item]) => ({ id, item: plain(item) as T }) as ItemEntry<T>),
      problems: [],
    }),
    saveItem: async (kind, id, item) => {
      if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(id)) throw `"${id}" cannot be used as a file name`;
      const name = (item as { name?: string }).name;
      if (!name || !name.trim()) throw 'Give it a name';
      store[kind].set(id, plain(item));
    },
    deleteItem: async (kind, id) => {
      store[kind].delete(id);
    },
    newItemId: async (kind, name) => {
      const base = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 48) || kind;
      let id = base;
      for (let n = 2; store[kind].has(id); n++) id = `${base}-${n}`;
      return id;
    },
    validateLayout: async () => [],
    buttonReach: async (layout) => {
      const out: Record<string, Reach> = {};
      for (const b of layout.blocks) {
        if (b.type !== 'sharing_button') continue;
        const c = b.connection_id ? (store.connection.get(b.connection_id) as Connection | undefined) : undefined;
        const active = c && stateOf(b.connection_id!, c) !== 'disconnected';
        out[b.id] = demoReach(layout, b, active ? demoCapabilities(c.kind) : LOCAL_CAPABILITIES, active ? c.kind : null);
      }
      return out;
    },
    // the CargoFrame bug report is for debug mode, as in the app
    listLayoutTemplates: async () => TEMPLATE_CATALOG.filter((t) => general.debug || t.key !== 'cargoframe-report').map((t) => ({ ...t })),
    layoutFromTemplate: async (key, connectionId) => {
      const t = TEMPLATE_CATALOG.find((x) => x.key === key);
      if (!t) throw 'That template does not exist';
      if (t.service && !connectionId) throw `“${t.name}” needs a connection to be built for`;
      return templateLayout(t, t.service ? connectionId : null);
    },
    exportSettings: async (selection) => {
      if (selection.every((s) => s.ids.length === 0)) throw 'Choose something to export';
      return '/home/me/Documents/cargoframe-settings.json';
    },
    openImport: async () => opts.importFile ?? null,
    applyImport: async (picks) => ({
      imported: picks
        .filter((p) => p.action !== 'skip')
        .map((p) => ({ group: p.group, id: p.id, name: p.id, copy: p.action === 'copy' })),
      skipped: picks.filter((p) => p.action === 'skip').length,
      failed: [],
    }),
    discardImport: async () => {},
    listVariables: async () => [
      ...BUILTINS,
      ...[...store.rule.entries()].flatMap(([id, r]) =>
        valuesOf(id, r as Rule).map((v) => ({
          name: `rule.${v.key}`,
          description: `${v.name || v.key}, from ${(r as Rule).file || 'other values'}`,
        })),
      ),
    ],
    previewValues: async () => demoValues([...store.rule.values()] as Rule[]),
    listSampleFiles: async () => ({
      folder: '/home/me/dev/Harbor',
      files: DEMO_SAMPLES.map((f) => ({ path: f.path, size: f.text.length, modified_ms: f.modified_ms })),
    }),
    readSampleFile: async (path) => {
      const f = DEMO_SAMPLES.find((d) => d.path === path);
      if (!f) throw new Error(`${path} is not in the last capture's folder`);
      return { text: f.text, cut: false, size: f.text.length, modified_ms: f.modified_ms };
    },
    testRule: async (rule, folder) => {
      // A stand-in that answers like the real one: each value is "found" as "<its name> demo".
      // picks are run on the demo files; a file that is not one of them is "found" anyway
      const demo = demoValues([rule]);
      const inDemo = Object.keys(demo.sources).length > 0;
      const file = rule.file.trim() ? (Object.values(demo.sources)[0] ?? rule.file.trim()) : null;
      const values = (rule.values ?? []).map((v) => {
        const got = inDemo || v.find.type === 'pick' ? (demo.values[`rule.${v.key}`] ?? null) : file ? `${v.name || v.key} demo` : null;
        return {
          key: v.key,
          value: got ?? v.fallback,
          problem: got ? null : 'no file matches',
          used_fallback: !got && v.fallback !== null,
        };
      });
      return {
        folder: folder ?? '/home/me/dev/Foo',
        value: values[0]?.value ?? null,
        problem: null,
        file,
        excerpt: 'GPU: RTX 4070\nOS Name: Windows 11',
        kind: 'text',
        files: file ? [file] : [],
        values,
      };
    },
    previewFiles: async (block, program) => {
      const folders = block.look_in.folders.length ? block.look_in.folders : (['project'] as LookFolder[]);
      const names: Record<LookFolder, string> = {
        project: 'Project folder',
        program: 'Program folder',
        above_1: 'One folder above the program',
        above_2: 'Two folders above the program',
        above_3: 'Three folders above the program',
      };
      const places = folders.map((f) => ({ label: names[f], folder: `/home/me/dev/Foo/${f}`, exists: true, found: 2 }));
      return {
        program: program ?? '/home/me/dev/Foo/bin/Foo',
        places,
        missing: [],
        total: places.length * 2,
        ticked: 0,
        truncated: false,
        problem: null,
      };
    },
    getShortcutStatus: async () => ({ route: 'system', note: null, errors: {}, commands: {} }),
    listConnections: async () => ({
      items: [...store.connection.entries()].map(([id, raw]) => {
        const c = raw as Connection;
        return {
          id,
          name: c.name,
          kind: c.kind,
          kind_label: KIND_NAME[c.kind],
          enabled: c.enabled,
          state: stateOf(id, c),
          secrets: TYPED[c.kind]!.map((name) => ({ name, stored: secretsSet.has(`${id}/${name}`) })),
          describe: describeFor(c),
          capabilities: demoCapabilities(c.kind),
        } satisfies ConnectionView;
      }),
      problems: [],
      store_problem: null,
    }),
    setConnectionSecret: async (id, name, value) => {
      if (value) secretsSet.add(`${id}/${name}`);
      else secretsSet.delete(`${id}/${name}`);
    },
    connectConnection: async (id) => {
      signedIn.add(id);
      return 'Signed in as demo@example.com';
    },
    cancelSignIn: async () => {},
    disconnectConnection: async (id) => {
      signedIn.delete(id);
      for (const k of [...secretsSet]) if (k.startsWith(`${id}/`)) secretsSet.delete(k);
    },
    testConnection: async (id) => {
      const c = store.connection.get(id) as Connection | undefined;
      if (!c || stateOf(id, c) !== 'connected') throw 'Not signed in';
      return 'Signed in as demo@example.com';
    },
    describeConnection: async (id) => describeFor(store.connection.get(id) as Connection),
    previewOptions: async (id, list) =>
      opts.options ? opts.options(id, list) : [{ value: '1', label: `${list} one`, group: null }],
    listChats: async () => chats.map((c) => ({ ...c })),
    pinChat: async (link, title) => {
      const m = /([0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12})/i.exec(link);
      if (!m) throw 'That does not look like a Claude chat link. Open the chat and copy its address (https://claude.ai/chat/...)';
      const id = m[1]!.toLowerCase();
      chats = [...chats.filter((c) => c.id !== id), { id, title: title.trim() || 'Untitled chat', pinned: true }];
    },
    setChatPinned: async (id, pinned) => {
      chats = chats.map((c) => (c.id === id ? { ...c, pinned } : c));
    },
    removeChat: async (id) => {
      chats = chats.filter((c) => c.id !== id);
    },
    fetchOptions: async (_sessionId, blockId, parent, query) =>
      opts.options
        ? opts.options('demo', parent ? `${blockId}:${parent}` : query !== undefined ? `${blockId}?${query}` : blockId)
        : parent
          ? []
          : [{ value: 'P1', label: 'Demo project', group: null }],
    setEditedImage: async () => {},
    setClipMark: async () => {},
    setClipLayer: async () => {},
    addFiles: async () => opts.addFiles?.() ?? [{ path: 'added/notes.txt', size: 42, selected: true }],
    readOfferedFile: async (_sessionId, path) =>
      opts.readOfferedFile?.(path) ?? {
        text: [
          '12:00:01 [info] Editor started',
          '12:00:02 [info] Project Foo opened',
          '12:00:05 [warning] Shader cache is old',
          '12:00:09 [error] Save failed: disk full',
          '12:00:10 [info] Retrying save',
          '12:00:11 [error] Save failed again',
        ].join('\n'),
        cut: false,
        size: 220,
      },
    readText: async (_png, attempt) =>
      opts.readText
        ? opts.readText(attempt)
        : { text: 'Build 4.2.1 failed on save', lines: [{ text: 'Build 4.2.1 failed on save', x: 4, y: 4, w: 180, h: 16 }], reader: 'demo', attempt },
    share: async (request) =>
      opts.share
        ? opts.share(request)
        : { status: 'sent', summary: 'Created DEMO-1', url: 'https://example.atlassian.net/browse/DEMO-1', notes: [], close: true },
    openLink: async () => {},
    reportPainted: async () => {},
    getTimings: async () => [
      { id: 'ready', label: 'Shortcut to capture ready', ms: 64, budget_ms: 150, note: null },
      { id: 'paint', label: 'Shortcut to usable window', ms: 410, budget_ms: 700, note: 'cold' },
      { id: 'settings_paint', label: 'Settings window open', ms: 320, budget_ms: 500, note: 'cold' },
    ],
    getMemoryStatus: async () => ({
      mode: general.memory_mode,
      windows: [
        { label: 'settings', name: 'Settings', state: 'open' },
        { label: 'capture', name: 'Capture window', state: general.memory_mode === 'low_memory' ? 'closed' : 'kept' },
      ],
    }),
    getSafe: async () => ({ ...safe }),
    setSafe: async (s) => {
      safe = { ...s };
    },
    testSafeFolder: async (path) => {
      if (!path.trim()) throw 'Choose a folder';
    },
    getVideo: async () => ({ ...video }),
    setVideo: async (v) => {
      video = { ...v };
    },
    detectVideoTools: async (v) => {
      if (opts.noFfmpeg) {
        return {
          found: false, ffmpeg: null, version: null, encoders: [], captures: [], auto_capture: null,
          auto_encoder: null, os: 'windows', problem: v.ffmpeg
            ? `ffmpeg was not found in ${v.ffmpeg}.`
            : 'ffmpeg was not found. Download it from Settings, System, or choose the folder it is in under Recording.',
        };
      }
      return {
        found: true, ffmpeg: 'C:\\Tools\\ffmpeg\\bin\\ffmpeg.exe', version: '7.1-full_build',
        encoders: ['software', 'nvenc', 'media_foundation'], captures: ['gdigrab', 'ddagrab'],
        auto_capture: 'gdigrab', auto_encoder: 'software', os: 'windows', problem: null,
      };
    },
    getSetup: async () => {
      // every check looks at the machine again; a test can change the machine between two checks
      if (setupChecks++ > 0 && opts.setupThen && opts.setupThen !== scenario) {
        scenario = opts.setupThen;
        installed = hasFfmpeg(scenario);
      }
      if (installLeft > 0) {
        installLeft -= 1;
        if (installLeft === 0 && !opts.installFails) {
          installed = true;
          claudeInstalled = true;
        }
        else if (installLeft === 0) installFailed = true;
      }
      return setupReport();
    },
    installMissing: async () => {
      installFailed = false;
      installLeft = opts.installPolls ?? 2;
      return setupReport();
    },
    openSoundSettings: async () => {
      soundSettingsOpened += 1;
      void soundSettingsOpened;
    },
    listAudioDevices: async () => {
      if (opts.noAudio) {
        return { os: 'linux', desktop: [], microphone: [], listed: false, note: 'No sound system answered (PulseAudio or PipeWire). Recordings will have no sound.' };
      }
      if (opts.windows) {
        return {
          os: 'windows', desktop: [], listed: true, note: null,
          microphone: [{ name: 'Microphone (Realtek(R) Audio)', label: 'Microphone (Realtek(R) Audio)', default: false }],
        };
      }
      return {
        os: 'linux', listed: true, note: null,
        desktop: [{ name: 'alsa_output.pci-0000_00_1f.3.analog-stereo.monitor', label: 'Monitor of Built-in Audio Analog Stereo', default: true }],
        microphone: [
          { name: 'alsa_input.pci-0000_00_1f.3.analog-stereo', label: 'Built-in Audio Analog Stereo', default: true },
          { name: 'bluez_input.00_1B_66_00_00_00.0', label: 'Headset (Bluetooth)', default: false },
        ],
      };
    },
    tryVideoRecording: async (v) => {
      if (opts.recordingFails) {
        return {
          ok: false, message: 'The recorder stopped by itself: Cannot load nvcuda.dll. Hardware encoders need the matching graphics card and driver: choose Software to carry on.',
          capture: 'gdigrab', encoder: 'nvenc', width: 0, height: 0, clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
        };
      }
      trialRan = true;
      return {
        ok: true, message: 'Recorded 5.0 s at 1920×1080 with GDI and software (x264). Open the clip to check that it shows your screen.',
        capture: v.capture === 'auto' ? 'gdigrab' : v.capture, encoder: v.encoder === 'auto' ? 'software' : v.encoder,
        width: 1920, height: 1080, clip_ms: 5000, clip_bytes: 2_400_000, took_ms: 6100,
        clip: 'C:\\Users\\me\\AppData\\Local\\CargoFrame\\video-test\\test-clip.mp4',
      };
    },
    getVideoTestClip: async () => {
      if (!trialRan) throw 'There is no test clip yet: run a test recording first';
      return new Blob([new Uint8Array(16)], { type: 'video/mp4' });
    },
    openVideoTest: async () => {
      if (!trialRan) throw 'There is no test clip yet: run a test recording first';
    },
    pickFolder: async () => '/home/me/Documents/CargoFrame zips',
    getUpdateView: async () => updateView(),
    checkUpdate: async () => {
      const { newer, fails } = opts.update ?? {};
      updateLast = fails
        ? { state: 'failed', message: fails.message, needs_token: fails.needs_token === true }
        : newer
          ? {
              state: 'available',
              version: newer.version,
              notes: newer.notes ?? '',
              page: `https://github.com/SotharL/CargoFrame/releases/tag/v${newer.version}`,
              installer: `CargoFrame-${newer.version}-windows-x64-setup.exe`,
              size: newer.size ?? 18 * 1024 * 1024,
              published: '2026-11-01T10:00:00Z',
            }
          : { state: 'up_to_date', latest: '0.1.0-dev' };
      return updateView();
    },
    installUpdate: async () => {
      updatePhase = { state: 'installing' };
      for (const f of updateListeners) f();
    },
    setUpdateToken: async (value) => {
      updateToken = value.trim() !== '';
    },
    onUpdateChanged: async (callback) => {
      updateListeners.add(callback);
      return () => {
        updateListeners.delete(callback);
      };
    },
    ffmpegStatus: async () => ffmpegPhase,
    ffmpegCheck: async () => {
      ffmpegPhase = { state: 'offered', version: '9.0', size: 168 * 1024 * 1024 };
      return ffmpegPhase;
    },
    ffmpegDownload: async () => {
      ffmpegPhase = { state: 'done', version: '9.0' };
      for (const f of ffmpegListeners) f();
    },
    onFfmpegChanged: async (callback) => {
      ffmpegListeners.add(callback);
      return () => {
        ffmpegListeners.delete(callback);
      };
    },
    onDebugChanged: async () => () => {},
  };
}

/** A copy that is safe to keep: the real backend gets JSON too, and a Svelte proxy cannot be structured-cloned. */
const plain = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

const channel = (server: string, category: string | null, name: string): DropdownOption => ({
  value: `${server}/${name}`, label: `#${name}`, group: server, server, category,
});
const DEMO_CHANNELS: DropdownOption[] = [
  channel('Team Foo', 'Development', 'bugs'), channel('Team Foo', 'Development', 'builds'), channel('Team Foo', 'Development', 'general-dev'),
  channel('Team Foo', 'Support', 'help'), channel('Team Foo', 'Support', 'feedback'), channel('Team Foo', null, 'announcements'),
  channel('Friends', null, 'chat'), channel('Friends', null, 'screenshots'),
];
const thread = (parent: string, name: string): DropdownOption => ({
  value: `thread-${name}`, label: name, group: 'Team Foo', server: 'Team Foo', category: parent,
});
const DEMO_THREADS: DropdownOption[] = [
  thread('bugs', 'What is wrong in Foo? (crash)'), thread('bugs', 'Slow start on Windows'), thread('builds', 'Nightly 2026-11-06'),
];

/** Outside the app: the in-memory backend, with the demo's answers to a share. */
export async function demoApi(): Promise<Api> {
  const { demoShare } = await import('../capture/demo');
  const query = new URLSearchParams(location.search);
  const kind = query.get('share');
  // `?update=newer` or `?update=fails` shows what Settings, General looks like after a check.
  const update =
    query.get('update') === 'newer'
      ? { newer: { version: '0.2.0', notes: '- Updates can be installed from Settings.\n- The capture window marks what each button sends.', size: 18 * 1024 * 1024 } }
      : query.get('update') === 'fails'
        ? { fails: { message: 'GitHub says the CargoFrame repository was not found. It may be private: add a token below.', needs_token: true } }
        : undefined;
  // `?demo=discord`: a bot's lists, to look at the server > category > channel steps
  const options =
    query.get('demo') === 'discord'
      ? (_c: string, list: string) =>
          // `<block>:<channel>` is the threads inside one channel
          list.includes(':') ? DEMO_THREADS.filter((t) => list.endsWith(`/${t.category}`)) : DEMO_CHANNELS
      : undefined;
  const api = memoryApi({ share: () => demoShare(kind), update, options });
  // `?media=both` plays a real clip, so "Take screenshot" has a frame to take
  if (query.get('media') === 'both' && typeof MediaRecorder !== 'undefined') {
    api.getSessionClip = async () => (await import('../capture/demo')).demoClip();
  }
  return api;
}
