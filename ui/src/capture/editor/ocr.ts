// Reading the text in part of the screenshot: the part is cut from the picture as it was taken
// (without the marks drawn on it), sent to the app to read on this computer, and the lines found
// are shown where they are on the picture.
import type { TextReading } from '../../lib/types';
import type { PxRect } from './model';

/** How many ways there are to read a part ("Try again" goes to the next, then back to the first). */
export const READ_TRIES = 3;

/** The part of the picture at `r` (in its pixels) as a PNG. */
export async function regionPng(image: CanvasImageSource, r: PxRect): Promise<Blob> {
  const c = document.createElement('canvas');
  c.width = Math.max(1, Math.round(r.w));
  c.height = Math.max(1, Math.round(r.h));
  const ctx = c.getContext('2d');
  if (!ctx) throw new Error('the picture could not be cut');
  ctx.drawImage(image, r.x, r.y, r.w, r.h, 0, 0, c.width, c.height);
  return new Promise((ok, fail) => c.toBlob((b) => (b ? ok(b) : fail(new Error('the picture could not be cut'))), 'image/png'));
}

/** Each line found, placed on the whole picture (it was found in the part at `r`). */
export function placed(reading: TextReading, r: PxRect): (PxRect & { text: string })[] {
  return reading.lines.map((l) => ({ text: l.text, x: r.x + l.x, y: r.y + l.y, w: l.w, h: l.h }));
}

/** The text to copy: the lines picked, in their order, or all of it when none is. */
export function toCopy(reading: TextReading, picked: number[]): string {
  if (picked.length === 0) return reading.text;
  return [...picked].sort((a, b) => a - b).map((i) => reading.lines[i]?.text ?? '').join('\n');
}
