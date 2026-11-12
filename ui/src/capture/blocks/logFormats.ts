// The time formats a person used in log viewers, kept on this computer between captures: the
// last one used for each file name (Game.log opens with its own next time), and the last few
// used at all, offered in the format box. Formats only: never a line of a log.

const FOR_FILE = 'cargoframe.logtime.file.';
const RECENT = 'cargoframe.logtime.recent';
export const KEEP_FORMATS = 5;

const name = (path: string) => path.split(/[\\/]/).pop()?.toLowerCase() ?? path.toLowerCase();

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage can be unavailable: the format is simply not remembered */
  }
}

/** The format last used for a file of this name, if any. */
export function formatFor(path: string): string | null {
  const f = read<unknown>(FOR_FILE + name(path), null);
  return typeof f === 'string' && f ? f : null;
}

/** The formats used lately, the latest first. */
export function recentFormats(): string[] {
  const list = read<unknown>(RECENT, []);
  return Array.isArray(list) ? list.filter((f): f is string => typeof f === 'string' && !!f).slice(0, KEEP_FORMATS) : [];
}

/** Keep a format for a file of this name (empty: back to finding times by itself), and among the recent ones. */
export function keepFormat(path: string, format: string) {
  const f = format.trim();
  write(FOR_FILE + name(path), f || null);
  if (f) write(RECENT, [f, ...recentFormats().filter((x) => x !== f)].slice(0, KEEP_FORMATS));
}

/** How many formats are kept (for files, and the recent list). */
export function formatsKept(): number {
  try {
    let n = 0;
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k?.startsWith(FOR_FILE) || k === RECENT) n++;
    }
    return n;
  } catch {
    return 0;
  }
}

/** Forget every format kept (Settings, Your data). Returns how many there were. */
export function forgetFormats(): number {
  try {
    const mine: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k?.startsWith(FOR_FILE) || k === RECENT) mine.push(k);
    }
    for (const k of mine) localStorage.removeItem(k);
    return mine.length;
  } catch {
    return 0;
  }
}

/** Does a file the window offers (a path under the Files blocks' folder) stand for a file a rule
 *  found (a path under the project folder)? The same file, seen from either folder. */
export function sameFile(offered: string, found: string): boolean {
  const a = offered.replace(/\\/g, '/').toLowerCase();
  const b = found.replace(/\\/g, '/').toLowerCase();
  return a === b || a.endsWith('/' + b) || b.endsWith('/' + a);
}
