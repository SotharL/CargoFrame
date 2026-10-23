import type {
  AppInfo, General, IssueView, ItemEntry, ItemKind, ItemList, Layout, Rule, RuleTest, Safe,
  SessionView, VariableInfo,
} from './types';

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
    getSafe: () => invoke('get_safe'),
    setSafe: (safe) => invoke('set_safe', { safe }),
    testSafeFolder: (path) => invoke('test_safe_folder', { path }),
    pickFolder: (title) => invoke('pick_folder', { title: title ?? null }),
  };
}

export function memoryApi(initial?: Partial<General>): Api {
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

/** The backend for this page: Tauri when running in the app, an in-memory one otherwise. */
export function getApi(): Promise<Api> {
  cached ??= inTauri() ? tauriApi() : Promise.resolve(memoryApi());
  return cached;
}

/** For tests: replace the backend. */
export function setApiForTests(api: Api | undefined): void {
  cached = api ? Promise.resolve(api) : undefined;
}
