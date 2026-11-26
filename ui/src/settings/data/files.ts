// The files a value is picked in: those of the last capture (read by the app, inside its project
// folder) and files opened or dropped here (read in the window, never sent anywhere).
import { getApi } from '../../lib/ipc';
import { linesOf } from './pick';
import { t } from '../../lib/i18n/index.svelte';

export interface ViewFile {
  /** The path in the capture's folder, or "dropped:<name>". */
  id: string;
  name: string;
  modified_ms: number;
  /** null while it is read. */
  lines: string[] | null;
  /** Only the end of it is here: it is large. */
  cut: boolean;
  problem: string | null;
}

/** The most of a file read here: its end, like the window's log viewer. */
const MAX_BYTES = 4 * 1024 * 1024;

/** `Saved/Logs/Game.log` → `Game.log`: the name a rule looks for in every folder. */
export const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

/** "14:03" today, else "16 Nov". */
export function changedAt(ms: number, now = new Date()): string {
  if (!ms) return '';
  const d = new Date(ms);
  const sameDay = d.toDateString() === now.toDateString();
  return sameDay
    ? d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
    : d.toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
}

/** The last capture's files, not read yet. */
export async function captureFiles(): Promise<{ folder: string | null; files: ViewFile[] }> {
  const list = await (await getApi()).listSampleFiles().catch(() => ({ folder: null, files: [] }));
  return {
    folder: list.folder,
    files: list.files.map((f) => ({ id: f.path, name: baseName(f.path), modified_ms: f.modified_ms, lines: null, cut: false, problem: null })),
  };
}

/** Read one of the last capture's files. */
export async function readCaptureFile(f: ViewFile): Promise<ViewFile> {
  try {
    const t = await (await getApi()).readSampleFile(f.id);
    return { ...f, lines: linesOf(t.text), cut: t.cut, modified_ms: t.modified_ms || f.modified_ms };
  } catch (e) {
    return { ...f, lines: [], problem: String(e) };
  }
}

/** A file opened or dropped here. */
export async function readDropped(file: File): Promise<ViewFile> {
  const cut = file.size > MAX_BYTES;
  const base = { id: `dropped:${file.name}`, name: file.name, modified_ms: file.lastModified, cut, problem: null };
  try {
    const text = await (cut ? file.slice(file.size - MAX_BYTES) : file).text();
    // a cut start is half a line
    const lines = linesOf(text);
    if (cut) lines.shift();
    if (text.includes('\u0000')) return { ...base, lines: [], problem: t('{name} is not a text file', { name: file.name }) };
    return { ...base, lines };
  } catch (e) {
    return { ...base, lines: [], problem: String(e) };
  }
}

/**
 * What stays open while the Settings window is: the files opened or dropped (already read), and
 * the capture's files closed with their ×. Leaving a value and coming back finds them as they
 * were; closing the window forgets them.
 */
const kept = { opened: [] as ViewFile[], closed: new Set<string>() };

/** The files to show: those opened before first, then the capture's files not closed. */
export function withKept(capture: ViewFile[]): ViewFile[] {
  const opened = kept.opened.filter((f) => !capture.some((c) => c.id === f.id));
  return [...opened, ...capture.filter((f) => !kept.closed.has(f.id))];
}

/** Keep files just opened or dropped (a file of the same name is replaced). */
export function keepOpened(read: ViewFile[]) {
  kept.opened = [...read, ...kept.opened.filter((f) => !read.some((r) => r.id === f.id))];
  for (const r of read) kept.closed.delete(r.id);
}

/** Close a file: an opened one is let go, a capture's file is hidden until the window closes. */
export function closeKept(id: string) {
  kept.opened = kept.opened.filter((f) => f.id !== id);
  if (!id.startsWith('dropped:')) kept.closed.add(id);
}

/** For tests: as when the Settings window opens. */
export function forgetKept() {
  kept.opened = [];
  kept.closed.clear();
}
