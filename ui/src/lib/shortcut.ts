/**
 * Turn a key press into the text the backend understands (`Ctrl+Shift+F9`). The backend checks and
 * canonicalises it again, so this only has to be understood, not authoritative.
 */

const NAMED: Record<string, string> = {
  Space: 'Space',
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  PrintScreen: 'PrintScreen',
  Pause: 'Pause',
  ScrollLock: 'ScrollLock',
  Minus: 'Minus',
  Equal: 'Equal',
  BracketLeft: 'BracketLeft',
  BracketRight: 'BracketRight',
  Semicolon: 'Semicolon',
  Quote: 'Quote',
  Comma: 'Comma',
  Period: 'Period',
  Slash: 'Slash',
  Backslash: 'Backslash',
  Backquote: 'Backquote',
};

/** Keys named as browsers name them, the same in the backend (number pad, sound, music, browser). */
const SAME = new Set([
  'Escape', 'CapsLock', 'NumLock', 'ContextMenu', 'IntlBackslash', 'IntlRo', 'IntlYen',
  'Numpad0', 'Numpad1', 'Numpad2', 'Numpad3', 'Numpad4', 'Numpad5', 'Numpad6', 'Numpad7', 'Numpad8',
  'Numpad9', 'NumpadAdd', 'NumpadSubtract', 'NumpadMultiply', 'NumpadDivide', 'NumpadDecimal',
  'NumpadEnter', 'AudioVolumeMute', 'AudioVolumeDown', 'AudioVolumeUp', 'MediaTrackNext',
  'MediaTrackPrevious', 'MediaStop', 'MediaPlayPause', 'LaunchMail', 'LaunchApp2', 'BrowserBack',
  'BrowserForward', 'BrowserRefresh', 'BrowserSearch', 'BrowserHome',
]);

/** The key itself, by physical position so it does not change with the keyboard layout. */
export function keyName(code: string): string | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1]!;
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1]!;
  m = /^F([0-9]{1,2})$/.exec(code);
  if (m && Number(m[1]) >= 1 && Number(m[1]) <= 24) return `F${m[1]}`;
  if (SAME.has(code)) return code;
  return NAMED[code] ?? null;
}

const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;
const MODIFIER_ORDER = ['Ctrl', 'Alt', 'Shift', 'Super'];
/** At most this many keys besides the modifiers (the backend says the same). */
const MAX_KEYS = 3;

export interface Pressed {
  /** Modifiers held, plus the key if one is down: `Ctrl+Shift+F9`, or `Ctrl+` while waiting. */
  text: string;
  /** A real key (not just modifiers) was pressed, so the combination can be used. */
  complete: boolean;
}

type Mods = Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>;

function modsOf(e: Mods): string[] {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  if (e.metaKey) parts.push('Super');
  return parts;
}

export function describePress(e: Pick<KeyboardEvent, 'code'> & Mods): Pressed {
  const parts = modsOf(e);
  if (MODIFIER_CODES.test(e.code)) {
    return { text: parts.length ? `${parts.join('+')}+` : '', complete: false };
  }
  const key = keyName(e.code);
  if (!key) return { text: parts.length ? `${parts.join('+')}+` : '', complete: false };
  return { text: [...parts, key].join('+'), complete: true };
}

/**
 * A combination being pressed: every key down at once, modifiers first, then the keys (sorted,
 * as the backend keeps them). It is done when every key has been let go.
 */
export class Combination {
  private mods = new Set<string>();
  private keys: string[] = [];
  private down = new Set<string>();
  /** A key that cannot be used was pressed: its code, to say so. */
  unknown = '';

  press(e: Pick<KeyboardEvent, 'code'> & Mods) {
    if (!e.code) return;
    this.down.add(e.code);
    for (const m of modsOf(e)) this.mods.add(m);
    if (MODIFIER_CODES.test(e.code)) return;
    const key = keyName(e.code);
    if (!key) {
      this.unknown = e.code;
      return;
    }
    if (!this.keys.includes(key) && this.keys.length < MAX_KEYS) this.keys.push(key);
  }

  /** A key came up; `true` when none is down any more. */
  release(code: string): boolean {
    this.down.delete(code);
    return this.down.size === 0;
  }

  get text(): string {
    const mods = MODIFIER_ORDER.filter((m) => this.mods.has(m));
    const keys = [...this.keys].sort();
    return [...mods, ...keys].join('+') + (keys.length ? '' : mods.length ? '+' : '');
  }

  get complete(): boolean {
    return this.keys.length > 0;
  }
}

/** The keys the system's shortcut service cannot hold (the backend's `held_by_system`). */
const NOT_SYSTEM = new Set(['IntlBackslash', 'IntlRo', 'IntlYen', 'ContextMenu', 'NumLock', 'LaunchMail', 'LaunchApp2', 'NumpadEnter']);

/** A shortcut CargoFrame watches itself (several keys, or a key the system cannot hold): its
 *  keys still reach the program in use. */
export function watchedByCargoFrame(shortcut: string): boolean {
  const keys = keycaps(shortcut).filter((k) => !MODIFIER_ORDER.includes(k));
  return keys.length > 1 || (keys.length === 1 && (NOT_SYSTEM.has(keys[0]!) || keys[0]!.startsWith('Browser')));
}

/** `Ctrl+Shift+F9` as separate keycaps. */
export function keycaps(shortcut: string): string[] {
  return shortcut.split('+').filter(Boolean);
}

const LABELS: Record<string, string> = {
  IntlBackslash: '<>', IntlRo: 'Ro', IntlYen: '¥', ContextMenu: 'Menu', NumpadAdd: 'Num +',
  NumpadSubtract: 'Num −', NumpadMultiply: 'Num ×', NumpadDivide: 'Num ÷', NumpadDecimal: 'Num .',
  NumpadEnter: 'Num ⏎', AudioVolumeMute: 'Mute', AudioVolumeDown: 'Vol −', AudioVolumeUp: 'Vol +',
  MediaTrackNext: '⏭', MediaTrackPrevious: '⏮', MediaStop: '⏹', MediaPlayPause: '⏯',
  LaunchMail: 'Mail', LaunchApp2: 'Calc', BrowserBack: 'Back', BrowserForward: 'Forward',
  BrowserRefresh: 'Refresh', BrowserSearch: 'Search', BrowserHome: 'Home page', Escape: 'Esc',
  ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→', PageUp: 'PgUp', PageDown: 'PgDn',
  PrintScreen: 'PrtSc',
};

/** How a key is shown on its keycap (`Num +` for NumpadAdd). */
export function keyLabel(key: string): string {
  const pad = /^Numpad([0-9])$/.exec(key);
  if (pad) return `Num ${pad[1]}`;
  return LABELS[key] ?? key;
}
