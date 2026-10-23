import type {
  AppInfo, ChatView, Connection, ConnectionsView, ConnectionView, Describe, DropdownOption,
  General, IssueView, ItemEntry, ItemKind, ItemList, Layout, OptionsError, Rule, RuleTest, Safe,
  SessionView, ShareRequest, ShareResult, VariableInfo,
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
  setGeneral(general: General): Promise<void>;
  getAutostart(): Promise<boolean>;
  setAutostart(enabled: boolean): Promise<void>;
  openConfigFolder(): Promise<void>;

  // ----- the CargoFrame window -----
  getSession(): Promise<SessionView>;
  /** The screenshot as a PNG file. */
  getSessionImage(): Promise<Blob>;
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
  listVariables(): Promise<VariableInfo[]>;
  testRule(rule: Rule, folder: string | null): Promise<RuleTest>;

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
  share(request: ShareRequest): Promise<ShareResult>;
  copyText(text: string): Promise<void>;
  openLink(url: string): Promise<void>;

  // ----- safe settings -----
  getSafe(): Promise<Safe>;
  setSafe(safe: Safe): Promise<void>;
  testSafeFolder(path: string): Promise<void>;
  /** The system folder picker; null when cancelled. */
  pickFolder(title?: string): Promise<string | null>;
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
    listVariables: () => invoke('list_variables'),
    testRule: (rule, folder) => invoke('test_rule', { rule, folder }),
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
    share: (request) => invoke<ShareResult>('share', { request }),
    copyText: (text) => invoke('copy_text', { text }),
    openLink: (url) => invoke('open_link', { url }),
    getSafe: () => invoke('get_safe'),
    setSafe: (safe) => invoke('set_safe', { safe }),
    testSafeFolder: (path) => invoke('test_safe_folder', { path }),
    pickFolder: (title) => invoke('pick_folder', { title: title ?? null }),
  };
}

/** Options for the in-memory stand-in: what a share and a list request should do. */
export interface MemoryOptions {
  general?: Partial<General>;
  /** Called by `share`; the default answers with a made-up success. */
  share?: (request: ShareRequest) => ShareResult | Promise<ShareResult>;
  /** Called by `fetchOptions` and `previewOptions`. */
  options?: (connectionId: string, list: string) => DropdownOption[] | Promise<DropdownOption[]>;
}

export function memoryApi(opts: MemoryOptions = {}): Api {
  const initial = opts.general;
  let general: General = {
    theme: 'system',
    memory_mode: 'balanced',
    check_updates: false,
    ...initial,
  };
  let autostart = true;
  let safe: Safe = { folder: null, timeout_secs: 30, max_retries: 3 };
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
  const TYPED: Record<string, string[]> = {
    jira: ['token'], google: ['client_secret'], reddit: [], claude: [], custom: ['secret'],
  };
  const OAUTH = new Set(['google', 'reddit']);
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
    closeCapture: async () => {},
    editLayout: async () => {},
    onSessionChanged: async () => () => {},
    openSettings: async () => {},
    onNavigate: async () => () => {},
    listItems: async <T,>(kind: ItemKind) => ({
      items: [...store[kind].entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([id, item]) => ({ id, item: structuredClone(item) as T }) as ItemEntry<T>),
      problems: [],
    }),
    saveItem: async (kind, id, item) => {
      if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(id)) throw `"${id}" cannot be used as a file name`;
      const name = (item as { name?: string }).name;
      if (!name || !name.trim()) throw 'Give it a name';
      store[kind].set(id, structuredClone(item));
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
    listVariables: async () => [
      ...BUILTINS,
      ...[...store.rule.entries()].map(([id, r]) => ({
        name: `rule.${id}`,
        description: `Data rule: ${(r as Rule).name}`,
      })),
    ],
    testRule: async (_rule, folder) => ({
      folder: folder ?? '/home/me/dev/Foo',
      value: '1.2.3',
      problem: null,
    }),
    getShortcutErrors: async () => ({}),
    listConnections: async () => ({
      items: [...store.connection.entries()].map(([id, raw]) => {
        const c = raw as Connection;
        return {
          id,
          name: c.name,
          kind: c.kind,
          kind_label: { jira: 'Jira', google: 'Google', reddit: 'Reddit', claude: 'Claude', custom: 'Custom API' }[c.kind],
          enabled: c.enabled,
          state: stateOf(id, c),
          secrets: TYPED[c.kind]!.map((name) => ({ name, stored: secretsSet.has(`${id}/${name}`) })),
          describe: describeFor(c),
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
    share: async (request) =>
      opts.share
        ? opts.share(request)
        : { status: 'sent', summary: 'Created DEMO-1', url: 'https://example.atlassian.net/browse/DEMO-1', notes: [], copy_next: null, close: true },
    copyText: async () => {},
    openLink: async () => {},
    getSafe: async () => ({ ...safe }),
    setSafe: async (s) => {
      safe = { ...s };
    },
    testSafeFolder: async (path) => {
      if (!path.trim()) throw 'Choose a folder';
    },
    pickFolder: async () => '/home/me/Documents/CargoFrame zips',
  };
}

let cached: Promise<Api> | undefined;

/** Outside the app: the in-memory backend, with the demo's answers to a share. */
async function demoApi(): Promise<Api> {
  const { demoShare } = await import('../capture/demo');
  const kind = new URLSearchParams(location.search).get('share');
  return memoryApi({ share: () => demoShare(kind) });
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
