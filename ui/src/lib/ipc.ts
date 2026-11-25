import type {
  AppInfo, Block, ChatView, Cleaned, ConnectionsView, DataReport, DebugSetup, DebugView, Describe, DropdownOption,
  ExportSelection, FilesPreview, General, ImportPick, ImportPreview, ImportReport, IssueView, ItemKind, ItemList,
  Layout, LayoutTemplate, OptionsError, Rule, RuleTest, Safe, SessionView, FfmpegPhase, MemoryStatus, Reach,
  ShareRequest, ShareResult, TimingView, UpdateView, VariableInfo, Video, VideoTools, VideoTrial, AudioDevices,
  SetupReport, ShortcutStatus, TextReading, FileEntry, LogText, ValuesPreview, SampleFiles, SampleText, LanguageInfo,
  PcCheck,
} from './types';

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
  /** The language the program speaks (from General, or the computer's). */
  getLanguage(): Promise<LanguageInfo>;
  setGeneral(general: General): Promise<void>;
  getAutostart(): Promise<boolean>;
  /** Debug mode: what this window shows now. */
  reportView(view: DebugView): Promise<void>;
  /** Debug mode: the GitHub connections, and whether the bug report is set up. */
  debugSetupStatus(): Promise<DebugSetup>;
  /** Make the bug report layout and its shortcut (what is there is kept). */
  setupDebugReport(connectionId?: string): Promise<DebugSetup>;
  /** *Check this PC*: the real tests on this computer, kept as a report. */
  checkThisPc(): Promise<PcCheck>;
  setAutostart(enabled: boolean): Promise<void>;
  /** Other programs Windows starts as CargoFrame at sign-in (an older copy). */
  autostartOthers(): Promise<string[]>;
  /** Stop them: Windows asks for the administrator's right. Returns what is left. */
  removeAutostartOthers(): Promise<string[]>;
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
  /** What the saved values give in the last capture's folder, and the file each came from. */
  previewValues(): Promise<ValuesPreview>;
  /** The text files of the last capture to pick a value in. */
  listSampleFiles(): Promise<SampleFiles>;
  /** One of them (a path inside the last capture's folder); the end of it when large. */
  readSampleFile(path: string): Promise<SampleText>;
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
  /** How shortcuts work on this desktop, and the ones that could not be set. */
  getShortcutStatus(): Promise<ShortcutStatus>;

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
  /** A dropdown's list; `parent` for the list inside a choice, `query` for what is typed in a list the service searches. */
  fetchOptions(sessionId: number, blockId: string, parent?: string, query?: string): Promise<DropdownOption[]>;
  /** The picture as edited, uploaded before a share. `null` goes back to the capture. */
  setEditedImage(sessionId: number, png: Blob | null): Promise<void>;
  /** The mark drawn over a recording (a see-through PNG the size of the recording), or none. */
  setClipMark(sessionId: number, png: Blob | null): Promise<void>;
  /** A text over a recording (`keys`, `clock`): each line's time and its pictures (a blank one
   *  first, then one per line); `null` forgets it. */
  setClipLayer(sessionId: number, name: 'keys' | 'clock', layer: { times: [number, number][]; pictures: Blob[] } | null): Promise<void>;
  /** Read the text in part of the screenshot (a PNG), the `attempt`-th way, on this computer. */
  readText(png: Blob, attempt: number): Promise<TextReading>;
  /** Pick files to add to a Files block by hand; each is copied into the capture now. */
  addFiles(sessionId: number, blockId: string): Promise<FileEntry[]>;
  /** The text of a file the capture offers (its end when large), for a log viewer. */
  readOfferedFile(sessionId: number, path: string): Promise<LogText>;
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

  // ----- ffmpeg, downloaded by CargoFrame -----
  ffmpegStatus(): Promise<FfmpegPhase>;
  /** Ask for the newest ffmpeg build and compare it with the one in use. */
  ffmpegCheck(): Promise<FfmpegPhase>;
  /** Download what the check offered. Returns at once; progress arrives as `onFfmpegChanged`. */
  ffmpegDownload(): Promise<void>;
  onFfmpegChanged(callback: () => void): Promise<() => void>;
  /** Debug mode was turned on or off (from System): every window follows. */
  onDebugChanged(callback: (on: boolean) => void): Promise<() => void>;
}

/** Pieces in one body, each after its length (four bytes, big-endian), as `set_clip_layer` reads them. */
export async function frames(blobs: Blob[]): Promise<Uint8Array> {
  const parts = await Promise.all(blobs.map(async (b) => new Uint8Array(await b.arrayBuffer())));
  const out = new Uint8Array(parts.reduce((n, p) => n + 4 + p.length, 0));
  const view = new DataView(out.buffer);
  let at = 0;
  for (const p of parts) {
    view.setUint32(at, p.length);
    out.set(p, at + 4);
    at += 4 + p.length;
  }
  return out;
}

export const inTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function tauriApi(): Promise<Api> {
  const { invoke } = await import('@tauri-apps/api/core');
  return {
    appInfo: () => invoke<AppInfo>('app_info'),
    getGeneral: () => invoke<General>('get_general'),
    getLanguage: () => invoke<LanguageInfo>('get_language'),
    setGeneral: (general) => invoke('set_general', { general }),
    getAutostart: () => invoke<boolean>('get_autostart'),
    reportView: (view) => invoke('report_view', { view }),
    debugSetupStatus: () => invoke<DebugSetup>('debug_setup_status'),
    setupDebugReport: (connectionId) => invoke<DebugSetup>('setup_debug_report', { connectionId: connectionId ?? null }),
    checkThisPc: () => invoke<PcCheck>('check_this_pc'),
    setAutostart: (enabled) => invoke('set_autostart', { enabled }),
    autostartOthers: () => invoke<string[]>('autostart_others'),
    removeAutostartOthers: () => invoke<string[]>('remove_autostart_others'),
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
    previewValues: () => invoke('preview_values'),
    listSampleFiles: () => invoke('list_sample_files'),
    readSampleFile: (path) => invoke('read_sample_file', { path }),
    previewFiles: (block, program) => invoke('preview_files', { block, program }),
    getShortcutStatus: () => invoke('get_shortcut_status'),
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
    fetchOptions: async (sessionId, blockId, parent, query) => {
      try {
        return await invoke<DropdownOption[]>('fetch_options', {
          sessionId,
          blockId,
          parent: parent ?? null,
          query: query ?? null,
        });
      } catch (e) {
        throw asOptionsError(e);
      }
    },
    setEditedImage: async (sessionId, png) => {
      const bytes = png ? new Uint8Array(await png.arrayBuffer()) : new Uint8Array();
      await invoke('set_edited_image', bytes, { headers: { 'x-session-id': String(sessionId) } });
    },
    addFiles: (sessionId, blockId) => invoke<FileEntry[]>('add_files', { sessionId, blockId }),
    readOfferedFile: (sessionId, path) => invoke<LogText>('read_offered_file', { sessionId, path }),
    readText: async (png, attempt) => {
      const bytes = new Uint8Array(await png.arrayBuffer());
      return invoke<TextReading>('read_text', bytes, { headers: { 'x-attempt': String(attempt) } });
    },
    setClipMark: async (sessionId, png) => {
      const bytes = png ? new Uint8Array(await png.arrayBuffer()) : new Uint8Array();
      await invoke('set_clip_mark', bytes, { headers: { 'x-session-id': String(sessionId) } });
    },
    setClipLayer: async (sessionId, name, layer) => {
      const body = layer ? await frames([new Blob([JSON.stringify(layer.times)]), ...layer.pictures]) : new Uint8Array();
      await invoke('set_clip_layer', body, { headers: { 'x-session-id': String(sessionId), 'x-layer': name } });
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
    ffmpegStatus: () => invoke('ffmpeg_status'),
    ffmpegCheck: () => invoke('ffmpeg_check'),
    ffmpegDownload: () => invoke('ffmpeg_download'),
    onFfmpegChanged: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event');
      return listen('ffmpeg-changed', callback);
    },
    onDebugChanged: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event');
      return listen<boolean>('debug-changed', (e) => callback(e.payload));
    },
  };
}

/** The backend this page uses, once asked for. */
let cached: Promise<Api> | undefined;

/** The backend for this page: Tauri when running in the app, the in-memory one (`memoryApi.ts`) otherwise. */
export function getApi(): Promise<Api> {
  cached ??= inTauri() ? tauriApi() : import('./memoryApi').then((m) => m.demoApi());
  return cached;
}

/** For tests: replace the backend. */
export function setApiForTests(api: Api | undefined): void {
  cached = api ? Promise.resolve(api) : undefined;
}
