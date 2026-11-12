import type { ClipPlace, KeyLine } from '../../lib/types';

/**
 * The keys pressed over a recording, and a crop that follows a window as it moved. The window
 * shows both as the clip plays; the share sends the keys as pictures (one per line, all the same
 * size, drawn here) that ffmpeg lays over the clip, so what plays here is what is sent.
 */

/** The height of the letters, as a part of the picture's height. */
export const KEY_SIZE = 0.055;
/** Where the keys start, as parts of the picture: near the bottom-left corner. */
export const KEY_START = { fx: 0.03, fy: 0.84 };
/** Where the time starts: above the keys, clear of the player's buttons at the top. */
export const CLOCK_START = { fx: 0.03, fy: 0.72 };

const two = (n: number) => String(n).padStart(2, '0');

/** A moment on this computer's clock, as logs write it: 2026-11-12 14:03:27. */
export function clockText(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}

/** The time over a clip that started at `startedAt` (ms since 1970): one line per second of the
 *  clock, each from when that second starts on the clip. */
export function clockLines(startedAt: number, durationMs: number): KeyLine[] {
  const out: KeyLine[] = [];
  let t = 0;
  while (t < durationMs && out.length < 400) {
    const wall = startedAt + t;
    const next = Math.min(durationMs, t + (1000 - (((wall % 1000) + 1000) % 1000)));
    out.push({ from_ms: Math.round(t), to_ms: Math.round(next), text: clockText(wall) });
    t = next;
  }
  return out;
}

/** The line showing at `ms` on the clip's timeline, if any. */
export function lineAt(lines: readonly KeyLine[] | undefined, ms: number): KeyLine | null {
  if (!lines) return null;
  for (const l of lines) {
    if (ms >= l.from_ms && ms < l.to_ms) return l;
    if (l.from_ms > ms) break;
  }
  return null;
}

/** Where a following crop's corner is at `ms`: it slides from each place to the next. */
export function placeAt(places: readonly ClipPlace[], ms: number): { x: number; y: number } {
  const first = places[0];
  if (!first) return { x: 0, y: 0 };
  if (ms <= first.at_ms) return { x: first.x, y: first.y };
  for (let i = 1; i < places.length; i++) {
    const a = places[i - 1]!;
    const b = places[i]!;
    if (ms < b.at_ms) {
      const k = b.at_ms > a.at_ms ? (ms - a.at_ms) / (b.at_ms - a.at_ms) : 1;
      const even = (n: number) => Math.round(n / 2) * 2;
      return { x: even(a.x + (b.x - a.x) * k), y: even(a.y + (b.y - a.y) * k) };
    }
  }
  const last = places[places.length - 1]!;
  return { x: last.x, y: last.y };
}

/** The letters' size, in pixels, on a picture `height` high. */
export const keyPixels = (height: number) => Math.max(12, Math.round(height * KEY_SIZE));
const font = (px: number) => `700 ${px}px system-ui, "Segoe UI", "DejaVu Sans", sans-serif`;

/**
 * The pictures of the keys for a sent picture `height` high: a see-through blank one first,
 * then one for each line, all the same size (as wide as the widest line), black letters with a
 * white outline. The window cannot draw without a canvas, and says so.
 */
export async function drawKeys(lines: readonly KeyLine[], height: number): Promise<Blob[]> {
  const px = keyPixels(height);
  const outline = Math.max(2, Math.round(px * 0.18));
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('This window cannot draw the keys pressed');
  ctx.font = font(px);
  const widest = Math.max(1, ...lines.map((l) => ctx.measureText(l.text).width));
  canvas.width = Math.ceil(widest + outline * 2 + 4);
  canvas.height = Math.ceil(px * 1.4 + outline * 2);
  const picture = () =>
    new Promise<Blob>((done, fail) => canvas.toBlob((b) => (b ? done(b) : fail(new Error('The keys could not be made into a picture'))), 'image/png'));
  const out: Blob[] = [];
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  out.push(await picture());
  for (const l of lines) {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    // setting the size clears the drawing style too, so it is set for each one
    ctx.font = font(px);
    ctx.textBaseline = 'middle';
    ctx.lineJoin = 'round';
    ctx.lineWidth = outline;
    ctx.strokeStyle = '#fff';
    ctx.fillStyle = '#000';
    const y = canvas.height / 2;
    ctx.strokeText(l.text, outline + 2, y);
    ctx.fillText(l.text, outline + 2, y);
    out.push(await picture());
  }
  return out;
}
