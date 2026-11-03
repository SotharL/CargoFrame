import type {
  AppInfo, Block, ChatView, Connection, ConnectionsView, ConnectionView, Describe, DropdownOption,
  ExportSelection, FilesPreview, General, ImportPick, ImportPreview, ImportReport, IssueView, ItemEntry,
  ItemKind, ItemList, Layout, LayoutTemplate, LookFolder, OptionsError, Rule, RuleTest, Safe, SessionView,
  MemoryStatus, Reach, ShareRequest, ShareResult, TimingView, UpdateLast, UpdateView, VariableInfo, Video,
  VideoRecording, VideoTestEdit, VideoTools, VideoTrial,
} from './types';
import { demoCapabilities, demoReach, LOCAL_CAPABILITIES } from './demoReach';

/** Whatever a failed list request rejected with, as an `OptionsError`. */
export function asOptionsError(e: unknown): OptionsError {
  if (e && typeof e === 'object' && 'message' in e) {
    const o = e as { message: unknown; reconnect?: unknown };
    return { message: String(o.message), reconnect: o.reconnect === true };
  }
  return { message: String(e), reconnect: false };
}

/**
 * Everything the interface asks the backend for. In the app these calls go to Rust through
 * Tauri. In a plain browser (`npm run dev`, tests, screenshots) an in-memory stand-in answers
 * instead, so the interface can be built and checked without the app.
 */
export interface Api {
  appInfo(): Promise<AppInfo>;
  getGeneral(): Promise<General>;
  setGeneral(general: General): Promise<void>;
  getAutostart(): Promise<boolean>;
  setAutostart(enabled: boolean): Promise<void>;
  openConfigFolder(): Promise<void>;

  // ----- the CargoFrame window -----
  getSession(): Promise<SessionView>;
  /** The screenshot as a PNG file. */
  getSessionImage(): Promise<Blob>;
  /** The recording of a video capture (MP4). */
  getSessionClip(): Promise<Blob>;
  closeCapture(): Promise<void>;
  editLayout(layoutId: string | null): Promise<void>;
  /** Called when a new capture replaces the one shown. Returns a function that stops listening. */
  onSessionChanged(callback: () => void): Promise<() => void>;

  /** Open the Settings window on a page (`connections`, `layouts`, ...). */
  openSettings(page: string): Promise<void>;
  /** The Settings window is asked to show a page, for example `layouts/bug`. */
  onNavigate(callback: (target: string) => void): Promise<() => void>;

  // ----- items: triggers, layouts, data rules, info types -----
  listItems<T>(kind: ItemKind): Promise<ItemList<T>>;
  /** Validates, then writes. Rejects with a sentence saying what to change. */
  saveItem(kind: ItemKind, id: string, item: unknown): Promise<void>;
  deleteItem(kind: ItemKind, id: string): Promise<void>;
  newItemId(kind: ItemKind, name: string): Promise<string>;
  validateLayout(layout: Layout): Promise<IssueView[]>;
  /** What each sharing button of the layout sends, by button id: asked of the code that does the sharing. */
  buttonReach(layout: Layout): Promise<Record<string, Reach>>;
  /** The ready-made layouts to start from. */
  listLayoutTemplates(): Promise<LayoutTemplate[]>;
  /** A template as an unsaved layout for a connection. Nothing is written. */
  layoutFromTemplate(key: string, connectionId: string | null): Promise<Layout>;
  listVariables(): Promise<VariableInfo[]>;
  testRule(rule: Rule, folder: string | null): Promise<RuleTest>;
  /** What a Files block would find. Without a program, the one of the last capture is used. */
  previewFiles(block: Extract<Block, { type: 'files' }>, program: string | null): Promise<FilesPreview>;

  // ----- export and import of settings -----
  /** Ask where to save, then write the chosen items. The path, or null when cancelled. */
  exportSettings(selection: ExportSelection[]): Promise<string | null>;
  /** Ask for a settings file and describe what is in it; nothing changes yet. Null when cancelled. */
  openImport(): Promise<ImportPreview | null>;
  /** Bring in the picks from the file opened last. */
  applyImport(picks: ImportPick[]): Promise<ImportReport>;
  discardImport(): Promise<void>;

  // ----- triggers -----
  getShortcutErrors(): Promise<Record<string, string>>;

  // ----- connections (Settings) -----
  listConnections(): Promise<ConnectionsView>;
  /** Store a password or key. An empty value removes it. It is never read back. */
  setConnectionSecret(id: string, name: string, value: string): Promise<void>;
  /** Sign in. Resolves with a line for the person when they are done. */
  connectConnection(id: string): Promise<string>;
  cancelSignIn(): Promise<void>;
  disconnectConnection(id: string): Promise<void>;
  testConnection(id: string): Promise<string>;
  describeConnection(id: string): Promise<Describe>;
  /** A list from a service, for the info type editor. Rejects with an `OptionsError`. */
  previewOptions(id: string, list: string): Promise<DropdownOption[]>;
  listChats(): Promise<ChatView[]>;
  pinChat(link: string, title: string): Promise<void>;
  setChatPinned(id: string, pinned: boolean): Promise<void>;
  removeChat(id: string): Promise<void>;

  // ----- sharing (the CargoFrame window) -----
  /** The list behind a dropdown block. Rejects with an `OptionsError`. */
  fetchOptions(sessionId: number, blockId: string): Promise<DropdownOption[]>;
  /** The picture as edited, uploaded before a share. `null` goes back to the capture. */
  setEditedImage(sessionId: number, png: Blob | null): Promise<void>;
  /** The mark drawn over a recording (a see-through PNG the size of the recording), or none. */
  setClipMark(sessionId: number, png: Blob | null): Promise<void>;
  share(request: ShareRequest): Promise<ShareResult>;
  openLink(url: string): Promise<void>;
  /** Tell the app the window has painted its first usable content (for the timings). */
  reportPainted(flow: 'capture' | 'settings'): Promise<void>;
  getTimings(): Promise<TimingView[]>;
  getMemoryStatus(): Promise<MemoryStatus>;

  // ----- safe settings -----
  getSafe(): Promise<Safe>;
  setSafe(safe: Safe): Promise<void>;
  testSafeFolder(path: string): Promise<void>;
  getVideo(): Promise<Video>;
  setVideo(video: Video): Promise<void>;
  /** Where ffmpeg is and what it can do, for the settings as they are in the form. */
  detectVideoTools(video: Video): Promise<VideoTools>;
  /** Record the screen for a few seconds with the settings as they are in the form. */
  tryVideoRecording(video: Video): Promise<VideoTrial>;
  /** Start recording until `stopVideoRecording`, keeping only the last seconds. */
  startVideoRecording(video: Video): Promise<void>;
  getVideoRecording(): Promise<VideoRecording>;
  /** Stop, make the clip, and report it like a test recording. */
  stopVideoRecording(): Promise<VideoTrial>;
  /** The last test clip (or, with `exported`, its edited copy), to play in the window. */
  getVideoTestClip(exported?: boolean): Promise<Blob>;
  /** The picture the page drew as the test mark (a PNG the size of the clip); null forgets it. */
  setVideoTestMark(png: Blob | null): Promise<void>;
  /** Cut, crop and mark the test clip into a copy. */
  exportVideoTest(video: Video, edit: VideoTestEdit): Promise<VideoTrial>;
  /** Open the clip of the last test recording (or its edited copy) in the video player. */
  openVideoTest(exported?: boolean): Promise<void>;
  /** The system folder picker; null when cancelled. */
  pickFolder(title?: string): Promise<string | null>;

  // ----- updates -----
  getUpdateView(): Promise<UpdateView>;
  /** Ask GitHub for the newest release. Resolves with the view after the answer. */
  checkUpdate(): Promise<UpdateView>;
  /** Download, verify and install the release the last check found. Returns at once; progress arrives as `onUpdateChanged`. */
  installUpdate(): Promise<void>;
  /** Store the GitHub token. An empty value removes it. It is never read back. */
  setUpdateToken(value: string): Promise<void>;
  /** Called whenever the update view changed. Returns a function that stops listening. */
  onUpdateChanged(callback: () => void): Promise<() => void>;
}

export const inTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function tauriApi(): Promise<Api> {
  const { invoke } = await import('@tauri-apps/api/core');
  return {
    appInfo: () => invoke<AppInfo>('app_info'),
    getGeneral: () => invoke<General>('get_general'),
    setGeneral: (general) => invoke('set_general', { general }),
    getAutostart: () => invoke<boolean>('get_autostart'),
    setAutostart: (enabled) => invoke('set_autostart', { enabled }),
    openConfigFolder: () => invoke('open_config_folder'),
    getSession: () => invoke<SessionView>('get_session'),
    getSessionImage: async () => {
      const bytes = await invoke<ArrayBuffer>('get_session_image');
      return new Blob([bytes], { type: 'image/png' });
    },
    getSessionClip: async () => {
      const bytes = await invoke<ArrayBuffer>('get_session_clip');
      return new Blob([bytes], { type: 'video/mp4' });
    },
    closeCapture: () => invoke('close_capture'),
    editLayout: (layoutId) => invoke('edit_layout', { layoutId }),
    onSessionChanged: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event');
      return listen('session-changed', callback);
    },
    openSettings: (page) => invoke('open_settings', { page }),
    onNavigate: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event');
      return listen<string>('navigate', (e) => callback(e.payload));
    },
    listItems: (kind) => invoke('list_items', { kind }),
    saveItem: (kind, id, item) => invoke('save_item', { kind, id, item }),
    deleteItem: (kind, id) => invoke('delete_item', { kind, id }),
    newItemId: (kind, name) => invoke('new_item_id', { kind, name }),
    validateLayout: (layout) => invoke('validate_layout', { layout }),
    buttonReach: (layout) => invoke('button_reach', { layout }),
    listLayoutTemplates: () => invoke('list_layout_templates'),
    layoutFromTemplate: (key, connectionId) => invoke('layout_from_template', { key, connectionId }),
    exportSettings: (selection) => invoke('export_settings', { selection }),
    openImport: () => invoke('open_import'),
    applyImport: (picks) => invoke('apply_import', { picks }),
    discardImport: () => invoke('discard_import'),
    listVariables: () => invoke('list_variables'),
    testRule: (rule, folder) => invoke('test_rule', { rule, folder }),
    previewFiles: (block, program) => invoke('preview_files', { block, program }),
    getShortcutErrors: () => invoke('get_shortcut_errors'),
    listConnections: () => invoke('list_connections'),
    setConnectionSecret: (id, name, value) => invoke('set_connection_secret', { id, name, value }),
    connectConnection: (id) => invoke('connect_connection', { id }),
    cancelSignIn: () => invoke('cancel_sign_in'),
    disconnectConnection: (id) => invoke('disconnect_connection', { id }),
    testConnection: (id) => invoke('test_connection', { id }),
    describeConnection: (id) => invoke('describe_connection', { id }),
    previewOptions: async (id, list) => {
      try {
        return await invoke<DropdownOption[]>('preview_options', { id, list });
      } catch (e) {
        throw asOptionsError(e);
      }
    },
    listChats: () => invoke('list_chats'),
    pinChat: (link, title) => invoke('pin_chat', { link, title }),
    setChatPinned: (id, pinned) => invoke('set_chat_pinned', { id, pinned }),
    removeChat: (id) => invoke('remove_chat', { id }),
    fetchOptions: async (sessionId, blockId) => {
      try {
        return await invoke<DropdownOption[]>('fetch_options', { sessionId, blockId });
      } catch (e) {
        throw asOptionsError(e);
      }
    },
    setEditedImage: async (sessionId, png) => {
      const bytes = png ? new Uint8Array(await png.arrayBuffer()) : new Uint8Array();
      await invoke('set_edited_image', bytes, { headers: { 'x-session-id': String(sessionId) } });
    },
    setClipMark: async (sessionId, png) => {
      const bytes = png ? new Uint8Array(await png.arrayBuffer()) : new Uint8Array();
      await invoke('set_clip_mark', bytes, { headers: { 'x-session-id': String(sessionId) } });
    },
    share: (request) => invoke<ShareResult>('share', { request }),
    openLink: (url) => invoke('open_link', { url }),
    reportPainted: (flow) => invoke('report_painted', { flow }),
    getTimings: () => invoke('get_timings'),
    getMemoryStatus: () => invoke('get_memory_status'),
    getSafe: () => invoke('get_safe'),
    setSafe: (safe) => invoke('set_safe', { safe }),
    testSafeFolder: (path) => invoke('test_safe_folder', { path }),
    getVideo: () => invoke('get_video'),
    setVideo: (video) => invoke('set_video', { video }),
    detectVideoTools: (video) => invoke('detect_video_tools', { video }),
    tryVideoRecording: (video) => invoke('try_video_recording', { video }),
    startVideoRecording: (video) => invoke('start_video_recording', { video }),
    getVideoRecording: () => invoke('get_video_recording'),
    stopVideoRecording: () => invoke('stop_video_recording'),
    getVideoTestClip: async (exported) => {
      const bytes = await invoke<ArrayBuffer>('get_video_test_clip', { exported });
      return new Blob([bytes], { type: 'video/mp4' });
    },
    setVideoTestMark: async (png) => {
      const bytes = png ? new Uint8Array(await png.arrayBuffer()) : new Uint8Array();
      await invoke('set_video_test_mark', bytes);
    },
    exportVideoTest: (video, edit) => invoke('export_video_test', { video, edit }),
    openVideoTest: (exported) => invoke('open_video_test', { exported }),
    pickFolder: (title) => invoke('pick_folder', { title: title ?? null }),
    getUpdateView: () => invoke('get_update_view'),
    checkUpdate: () => invoke('check_update'),
    installUpdate: () => invoke('install_update'),
    setUpdateToken: (value) => invoke('set_update_token', { value }),
    onUpdateChanged: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event');
      return listen('update-changed', callback);
    },
  };
}

/** Options for the in-memory stand-in: what a share and a list request should do. */
export interface MemoryOptions {
  general?: Partial<General>;
  /** Starting video settings. */
  video?: Partial<Video>;
  /** The stand-in finds no ffmpeg. */
  noFfmpeg?: boolean;
  /** The stand-in's test recording fails. */
  recordingFails?: boolean;
  /** Called by `share`; the default answers with a made-up success. */
  share?: (request: ShareRequest) => ShareResult | Promise<ShareResult>;
  /** Called by `fetchOptions` and `previewOptions`. */
  options?: (connectionId: string, list: string) => DropdownOption[] | Promise<DropdownOption[]>;
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

const TEMPLATE_CATALOG: LayoutTemplate[] = [
  { key: 'local-capture', name: 'Save a capture locally', description: 'The screenshot, a note and the files you tick, saved as a zip. No account needed.', service: null },
  { key: 'local-video', name: 'Save a recording locally', description: 'The recording, a note and the files you tick, saved as a zip. For a trigger that makes a video.', service: null },
  { key: 'jira-bug', name: 'Jira bug report', description: 'Project, issue type, summary and description, with the screenshot and files attached.', service: 'jira' },
  { key: 'jira-video', name: 'Jira bug report with a recording', description: 'Like the Jira bug report, with the recording attached. For a trigger that makes a video.', service: 'jira' },
  { key: 'jira-comment', name: 'Jira comment on an issue', description: 'Pick a recent issue and add a comment with the screenshot.', service: 'jira' },
  { key: 'google-drive', name: 'Google Drive upload', description: 'Upload the screenshot and ticked files to a Drive folder.', service: 'google' },
  { key: 'google-drive-video', name: 'Google Drive upload of a recording', description: 'Upload the recording and ticked files to a Drive folder. For a trigger that makes a video.', service: 'google' },
  { key: 'google-sheet', name: 'Google Sheet row', description: 'Add a row to a spreadsheet.', service: 'google' },
  { key: 'reddit-post', name: 'Reddit post', description: 'Post the screenshot to a subreddit, with a title and text.', service: 'reddit' },
  { key: 'claude-ask', name: 'Ask Claude about a capture', description: 'Write a prompt and hand the screenshot and prompt to Claude.', service: 'claude' },
  { key: 'slack-video', name: 'Slack message with a recording', description: 'Post a message with the recording into a channel, or reply in a thread. For a trigger that makes a video.', service: 'slack' },
  { key: 'discord-message', name: 'Discord message', description: 'Post a message with the screenshot and a zip of the ticked files into a channel, or start a post in a forum. Nobody is pinged.', service: 'discord' },
  { key: 'slack-message', name: 'Slack message', description: 'Post a message with the screenshot and a zip of the ticked files into a channel the app is in, or reply in a thread.', service: 'slack' },
  { key: 'teams-message', name: 'Teams message', description: 'Post a message with the screenshot inside it into a chat, a channel or a channel thread, as you.', service: 'teams' },
  { key: 'custom-post', name: 'Custom API post', description: 'A title and a message with the screenshot, sent through your request.', service: 'custom' },
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
      { id: 'note', type: 'text', rect: { x: 7, y: 0, w: 5, h: 3 }, title: 'Note', placement: 'above', role: 'plain', default_text: '{{app.title}}' },
      {
        id: 'send', type: 'sharing_button', rect: { x: 7, y: 6, w: 5, h: 1 },
        label: t.service ? 'Send' : 'Save locally', connection_id: connectionId,
        prompt_block_id: null, mappings: [], on_success: 'close_window',
      },
    ],
  };
}

export function memoryApi(opts: MemoryOptions = {}): Api {
  const initial = opts.general;
  let general: General = {
    theme: 'system',
    memory_mode: 'balanced',
    check_updates: false,
    accent: '#4f8ee0',
    ...initial,
  };
  let autostart = true;
  let safe: Safe = { folder: null, timeout_secs: 30, max_retries: 3 };
  let video: Video = { ffmpeg: null, capture: 'auto', encoder: 'auto', fps: 30, window_secs: 10, max_height: 1080, ...opts.video };
  let trialRan = false;
  let copyMade = false;
  let markDrawn = false;
  let recordingSince: number | null = null;
  let recordingEnded: string | null = null;
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
    jira: ['token'], google: ['client_secret'], reddit: [], claude: [], slack: ['token'], discord: ['token'], teams: [], custom: ['secret'],
  };
  const OAUTH = new Set(['google', 'reddit', 'teams']);
  const describeFor = (c: Connection): Describe => {
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
          { key: 'thread', label: 'Thread or forum post', hint: 'Overrides the channel', list: 'threads' },
          { key: 'message', label: 'Message', hint: 'Defaults to the prompt text', list: null },
          { key: 'title', label: 'Title', hint: 'The name of a forum post', list: null },
        ],
        lists: [
          { id: 'channels', label: 'Channels and forums the bot can see', param: null },
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
    return secretsSet.has(`${id}/${TYPED[c.kind]![0]}`) ? 'connected' : 'disconnected';
  };
  const BUILTINS: VariableInfo[] = [
    ['app.exe', 'File name of the program in focus'],
    ['app.path', 'Full path of that program'],
    ['app.title', 'The title of its window'],
    ['project.folder', 'The project folder found from the program\'s path'],
    ['project.name', 'That folder\'s name'],
    ['capture.date', 'Date of the capture (YYYY-MM-DD)'],
    ['capture.time', 'Time of the capture (HH:MM:SS)'],
  ].map(([name, description]) => ({ name: name!, description: description! }));
  return {
    appInfo: async () => ({
      version: '0.1.0-dev',
      os: 'linux',
      config_dir: '~/.config/CargoFrame',
      data_dir: '~/.local/share/CargoFrame',
      tray_available: true,
    }),
    getGeneral: async () => ({ ...general }),
    setGeneral: async (g) => {
      general = { ...g };
    },
    getAutostart: async () => autostart,
    setAutostart: async (enabled) => {
      autostart = enabled;
    },
    openConfigFolder: async () => {},
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
    listLayoutTemplates: async () => TEMPLATE_CATALOG.map((t) => ({ ...t })),
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
      ...[...store.rule.entries()].map(([id, r]) => ({
        name: `rule.${id}`,
        description: `Data rule: ${(r as Rule).name}`,
      })),
    ],
    testRule: async (rule, folder) => ({
      folder: folder ?? '/home/me/dev/Foo',
      value: '1.2.3',
      problem: null,
      file: rule.file.trim() || null,
      excerpt: '{\n  "version": "1.2.3"\n}',
    }),
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
    getShortcutErrors: async () => ({}),
    listConnections: async () => ({
      items: [...store.connection.entries()].map(([id, raw]) => {
        const c = raw as Connection;
        return {
          id,
          name: c.name,
          kind: c.kind,
          kind_label: { jira: 'Jira', google: 'Google', reddit: 'Reddit', claude: 'Claude', slack: 'Slack', discord: 'Discord', teams: 'Teams', custom: 'Custom API' }[c.kind],
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
    fetchOptions: async (_sessionId, blockId) =>
      opts.options ? opts.options('demo', blockId) : [{ value: 'P1', label: 'Demo project', group: null }],
    setEditedImage: async () => {},
    setClipMark: async () => {},
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
            : 'ffmpeg was not found. Install it (on Windows: winget install Gyan.FFmpeg) or choose the folder it is in.',
        };
      }
      return {
        found: true, ffmpeg: 'C:\\Tools\\ffmpeg\\bin\\ffmpeg.exe', version: '7.1-full_build',
        encoders: ['software', 'nvenc', 'media_foundation'], captures: ['gdigrab', 'ddagrab'],
        auto_capture: 'gdigrab', auto_encoder: 'software', os: 'windows', problem: null,
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
    startVideoRecording: async () => {
      if (recordingSince !== null) throw 'A recording is already running.';
      recordingSince = Date.now();
      recordingEnded = opts.recordingFails
        ? 'The recorder stopped by itself: Cannot load nvcuda.dll. Hardware encoders need the matching graphics card and driver: choose Software to carry on.'
        : null;
    },
    getVideoRecording: async () => {
      if (recordingEnded) return { state: 'failed', message: recordingEnded };
      if (recordingSince === null) return { state: 'idle' };
      const elapsed = Date.now() - recordingSince;
      return {
        state: 'recording', elapsed_ms: elapsed, buffered_ms: Math.min(elapsed, video.window_secs * 1000),
        squeezed: false, keep_secs: video.window_secs,
      };
    },
    stopVideoRecording: async () => {
      const ended = recordingEnded;
      const since = recordingSince;
      recordingSince = null;
      recordingEnded = null;
      if (ended) {
        return {
          ok: false, message: ended, capture: null, encoder: null, width: 0, height: 0, clip_ms: 0,
          clip_bytes: 0, took_ms: 0, clip: null,
        };
      }
      if (since === null) {
        return {
          ok: false, message: 'No recording is running.', capture: null, encoder: null, width: 0, height: 0,
          clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
        };
      }
      trialRan = true;
      const ran = Date.now() - since;
      const kept = Math.min(ran, video.window_secs * 1000);
      return {
        ok: true,
        message: `Recorded ${(kept / 1000).toFixed(1)} s at 1920×1080 with GDI and software (x264). Open the clip to check that it shows your screen.`,
        capture: 'gdigrab', encoder: 'software', width: 1920, height: 1080, clip_ms: kept, clip_bytes: kept * 480,
        took_ms: ran, clip: 'C:\\Users\\me\\AppData\\Local\\CargoFrame\\video-test\\test-clip.mp4',
      };
    },
    getVideoTestClip: async (exported) => {
      if (!trialRan || (exported && !copyMade)) throw 'There is no test clip yet: run a test recording first';
      return new Blob([new Uint8Array(16)], { type: 'video/mp4' });
    },
    setVideoTestMark: async (png) => {
      markDrawn = png !== null;
    },
    exportVideoTest: async (_v, edit) => {
      if (!trialRan) {
        return {
          ok: false, message: 'There is no test clip yet: run a test recording first', capture: null, encoder: null,
          width: 0, height: 0, clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
        };
      }
      if (edit.mark === 'drawn' && !markDrawn) {
        return {
          ok: false, message: 'The mark was not drawn: try again', capture: null, encoder: null, width: 0,
          height: 0, clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
        };
      }
      const length = (edit.end_ms ?? 5000) - edit.start_ms;
      if (length < 500) {
        return {
          ok: false, message: `The clip would be ${(Math.max(length, 0) / 1000).toFixed(1)} s long; it must be at least 0.5 s`,
          capture: null, encoder: null, width: 0, height: 0, clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
        };
      }
      copyMade = true;
      const w = edit.crop ? edit.crop.w - (edit.crop.w % 2) : 1920;
      const h = edit.crop ? edit.crop.h - (edit.crop.h % 2) : 1080;
      return {
        ok: true, message: `Made a copy of ${(length / 1000).toFixed(1)} s at ${w}×${h}. Open it to check the cut, the crop and the mark.`,
        capture: null, encoder: null, width: w, height: h, clip_ms: length, clip_bytes: length * 300, took_ms: 900,
        clip: 'C:\\Users\\me\\AppData\\Local\\CargoFrame\\video-test\\exported.mp4',
      };
    },
    openVideoTest: async (exported) => {
      if (!trialRan || (exported && !copyMade)) throw 'There is no test clip yet: run a test recording first';
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
  };
}

let cached: Promise<Api> | undefined;

/** A copy that is safe to keep: the real backend gets JSON too, and a Svelte proxy cannot be structured-cloned. */
const plain = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

/** Outside the app: the in-memory backend, with the demo's answers to a share. */
async function demoApi(): Promise<Api> {
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
  return memoryApi({ share: () => demoShare(kind), update });
}

/** The backend for this page: Tauri when running in the app, an in-memory one otherwise. */
export function getApi(): Promise<Api> {
  cached ??= inTauri() ? tauriApi() : demoApi();
  return cached;
}

/** For tests: replace the backend. */
export function setApiForTests(api: Api | undefined): void {
  cached = api ? Promise.resolve(api) : undefined;
}
