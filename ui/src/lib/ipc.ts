import type { AppInfo, General, SessionView } from './types';

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
