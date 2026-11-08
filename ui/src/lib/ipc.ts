import type {
  AppInfo, Block, ChatView, Cleaned, Connection, ConnectionsView, ConnectionView, DataReport, Describe, DropdownOption,
  ExportSelection, FilesPreview, General, ImportPick, ImportPreview, ImportReport, IssueView, ItemEntry,
  ItemKind, ItemList, Layout, LayoutTemplate, LookFolder, OptionsError, Rule, RuleTest, Safe, SessionView,
  MemoryStatus, Reach, ShareRequest, ShareResult, TimingView, UpdateLast, UpdateView, VariableInfo, Video,
  VideoTools, VideoTrial, AudioDevices, SetupReport, SetupItem, SetupInstall,
  ServiceKind,
} from './types';
import { demoCapabilities, demoReach, LOCAL_CAPABILITIES } from './demoReach';
import { KIND_NAME } from './connector';

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
  /** What CargoFrame keeps on this computer, by kind. */
  getDataReport(): Promise<DataReport>;
  /** Remove some kinds of data (`work`, `kept`, `logs`, `updates`). Rejects while a window or recording uses it. */
  cleanData(areas: string[]): Promise<Cleaned>;
  /** Back to a new install: sign-ins, settings, layouts, triggers and kept data removed. */
  resetEverything(): Promise<Cleaned>;

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
  /**
   * The list behind a dropdown block, or with `parent` the list inside that choice (the threads of
   * a channel). Rejects with an `OptionsError`.
   */
  fetchOptions(sessionId: number, blockId: string, parent?: string): Promise<DropdownOption[]>;
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
  /** The sound devices of this computer, for a video block's sound choices. */
  listAudioDevices(): Promise<AudioDevices>;
  /** The apps CargoFrame uses, and whether they are on this computer. */
  getSetup(): Promise<SetupReport>;
  /** The one button: install what is missing (Windows). It runs in the background; ask `getSetup`. */
  installMissing(): Promise<SetupReport>;
  /** Open the Windows sound settings, on the Playback tab, where the default output is chosen. */
  openSoundSettings(): Promise<void>;
  /** Record the screen for a few seconds with the settings as they are in the form. */
  tryVideoRecording(video: Video): Promise<VideoTrial>;
  /** The last test clip, to play in the window. */
  getVideoTestClip(): Promise<Blob>;
  /** Open the clip of the last test recording in the video player. */
  openVideoTest(): Promise<void>;
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
    getDataReport: () => invoke<DataReport>('get_data_report'),
    cleanData: (areas) => invoke<Cleaned>('clean_data', { areas }),
    resetEverything: () => invoke<Cleaned>('reset_everything'),
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
    // Only this window's own event: with several CargoFrame windows open, a new capture in one
    // must not reload the others.
    onSessionChanged: async (callback) => {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      return getCurrentWebviewWindow().listen('session-changed', callback);
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
    fetchOptions: async (sessionId, blockId, parent) => {
      try {
        return await invoke<DropdownOption[]>('fetch_options', { sessionId, blockId, parent: parent ?? null });
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
    listAudioDevices: () => invoke('list_audio_devices'),
    getSetup: () => invoke('get_setup'),
    installMissing: () => invoke('install_missing'),
    openSoundSettings: () => invoke('open_sound_settings'),
    tryVideoRecording: (video) => invoke('try_video_recording', { video }),
    getVideoTestClip: async () => {
      const bytes = await invoke<ArrayBuffer>('get_video_test_clip');
      return new Blob([bytes], { type: 'video/mp4' });
    },
    openVideoTest: () => invoke('open_video_test'),
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
  /** Called by `fetchOptions` (as `<block>:<choice>` for the list inside a choice) and `previewOptions`. */
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
  { key: 'x-post', name: 'X post', description: 'Post the screenshot to your X account with a short text (280 characters).', service: 'x' },
  { key: 'bluesky-post', name: 'Bluesky post', description: 'Post the screenshot to your Bluesky account with a short text (300 characters) and a description of the picture.', service: 'bluesky' },
  { key: 'instagram-post', name: 'Instagram post', description: 'Post the screenshot to your Instagram professional account with a caption.', service: 'instagram' },
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
    notifications: { routine: true, problems: true },
    ...initial,
  };
  // what the Your data section finds, and clears (sizes in bytes)
  let kept: Record<string, { files: number; bytes: number }> = {
    work: { files: 4, bytes: 5_300_000 }, kept: { files: 0, bytes: 0 }, logs: { files: 3, bytes: 120_000 }, updates: { files: 0, bytes: 0 },
  };
  let autostart = true;
  let safe: Safe = { folder: null, timeout_secs: 30, max_retries: 3 };
  let video: Video = { ffmpeg: null, capture: 'auto', encoder: 'auto', fps: 30, window_secs: 10, max_height: 1080, ...opts.video };
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
          ? { type: 'manual', text: 'Windows\' package manager (winget) was not found. Install "App Installer" from the Microsoft Store, then come back; or download ffmpeg yourself and choose its folder under Video.' }
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
    x: [], bluesky: ['app_password'], instagram: ['token', 'host_key'],
    discord: ['token', ...Array.from({ length: 10 }, (_, i) => `hook${i}`)],
  };
  const OAUTH = new Set(['google', 'reddit', 'teams', 'x']);
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
      connectors: opts.connectors ?? ['jira', 'google', 'reddit', 'claude', 'slack', 'discord', 'teams', 'x', 'bluesky', 'instagram', 'custom'],
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
    fetchOptions: async (_sessionId, blockId, parent) =>
      opts.options
        ? opts.options('demo', parent ? `${blockId}:${parent}` : blockId)
        : parent
          ? []
          : [{ value: 'P1', label: 'Demo project', group: null }],
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
  };
}

let cached: Promise<Api> | undefined;

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

/** The backend for this page: Tauri when running in the app, an in-memory one otherwise. */
export function getApi(): Promise<Api> {
  cached ??= inTauri() ? tauriApi() : demoApi();
  return cached;
}

/** For tests: replace the backend. */
export function setApiForTests(api: Api | undefined): void {
  cached = api ? Promise.resolve(api) : undefined;
}
