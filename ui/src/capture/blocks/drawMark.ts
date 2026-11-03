/** Where the mark goes on the recording, in the recording's own pixels. */
export interface MarkAt {
  x: number;
  y: number;
}

const RED = '#e5322d';

/** The ring is a tenth of the shorter side in radius; the text sits just under it. */
export const markRadius = (width: number, height: number) => Math.min(width, height) / 10;
export const markFontSize = (height: number) => Math.max(16, Math.round(height / 18));
export const markLineWidth = (height: number) => Math.max(4, Math.round(height / 120));

/**
 * A ring and a text on a see-through picture the size of the recording, as a PNG. ffmpeg lays it
 * over the kept part of the recording, so what the window shows is what the recording gets.
 */
export function drawMark(width: number, height: number, at: MarkAt, text: string): Promise<Blob> {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return Promise.reject('This window cannot draw the mark');
  const clamp = (n: number, hi: number) => Math.min(Math.max(n, 0), hi);
  const cx = clamp(at.x, width);
  const cy = clamp(at.y, height);
  ctx.lineWidth = markLineWidth(height);
  ctx.strokeStyle = RED;
  ctx.beginPath();
  ctx.arc(cx, cy, markRadius(width, height), 0, Math.PI * 2);
  ctx.stroke();
  const label = text.trim();
  if (label) {
    const size = markFontSize(height);
    ctx.font = `bold ${size}px sans-serif`;
    ctx.textBaseline = 'top';
    ctx.textAlign = 'center';
    ctx.lineWidth = Math.max(2, Math.round(size / 8));
    ctx.strokeStyle = '#ffffff';
    const y = Math.min(cy + markRadius(width, height) + size / 3, height - size * 1.2);
    ctx.strokeText(label, cx, y);
    ctx.fillStyle = RED;
    ctx.fillText(label, cx, y);
  }
  return new Promise((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject('The mark could not be made into a picture')), 'image/png');
  });
}
