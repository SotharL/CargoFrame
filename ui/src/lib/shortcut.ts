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

/** The key itself, by physical position so it does not change with the keyboard layout. */
export function keyName(code: string): string | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1]!;
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1]!;
  m = /^F([0-9]{1,2})$/.exec(code);
  if (m && Number(m[1]) >= 1 && Number(m[1]) <= 24) return `F${m[1]}`;
  return NAMED[code] ?? null;
}

const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

export interface Pressed {
  /** Modifiers held, plus the key if one is down: `Ctrl+Shift+F9`, or `Ctrl+` while waiting. */
  text: string;
  /** A real key (not just modifiers) was pressed, so the combination can be used. */
  complete: boolean;
}

export function describePress(e: Pick<KeyboardEvent, 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>): Pressed {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  if (e.metaKey) parts.push('Super');
  if (MODIFIER_CODES.test(e.code)) {
    return { text: parts.length ? `${parts.join('+')}+` : '', complete: false };
  }
  const key = keyName(e.code);
  if (!key) return { text: parts.length ? `${parts.join('+')}+` : '', complete: false };
  return { text: [...parts, key].join('+'), complete: true };
}

/** `Ctrl+Shift+F9` as separate keycaps. */
export function keycaps(shortcut: string): string[] {
  return shortcut.split('+').filter(Boolean);
}
