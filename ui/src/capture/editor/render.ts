import type { Annotation, EditorState, PxRect } from './model';
import { textBounds, visibleRect } from './model';

/** The canvas font of a text mark. */
export function fontOf(a: Extract<Annotation, { kind: 'text' }>): string {
  return `${a.italic ? 'italic ' : ''}${a.bold === false ? 500 : 700} ${a.size}px system-ui, 'Segoe UI', sans-serif`;
}

let ruler: CanvasRenderingContext2D | null | undefined;
/** The width of a line of a text mark, as the canvas draws it (a guess where there is no canvas). */
export function measureLine(a: Extract<Annotation, { kind: 'text' }>, line: string): number {
  if (ruler === undefined) ruler = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
  if (!ruler) return line.length * a.size * 0.58;
  ruler.font = fontOf(a);
  return ruler.measureText(line).width;
}

/** Black or white, whichever reads better on `hex`; used for text outlines. */
export function contrastOf(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return '#000000';
  const n = parseInt(m[1]!, 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255 > 0.6 ? '#000000' : '#ffffff';
}

function drawOne(ctx: CanvasRenderingContext2D, a: Annotation): void {
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  switch (a.kind) {
    case 'pen': {
      ctx.strokeStyle = a.color;
      ctx.lineWidth = a.width;
      ctx.beginPath();
      const [first, ...rest] = a.points;
      if (!first) return;
      ctx.moveTo(first.x, first.y);
      if (rest.length === 0) ctx.lineTo(first.x + 0.01, first.y);
      // Curve through the midpoints for a smooth line from jittery pointer samples.
      for (let i = 0; i < rest.length - 1; i++) {
        const p = rest[i]!;
        const q = rest[i + 1]!;
        ctx.quadraticCurveTo(p.x, p.y, (p.x + q.x) / 2, (p.y + q.y) / 2);
      }
      const last = rest[rest.length - 1];
      if (last) ctx.lineTo(last.x, last.y);
      ctx.stroke();
      return;
    }
    case 'rect': {
      ctx.strokeStyle = a.color;
      ctx.lineWidth = a.width;
      ctx.strokeRect(
        Math.min(a.a.x, a.b.x),
        Math.min(a.a.y, a.b.y),
        Math.abs(a.a.x - a.b.x),
        Math.abs(a.a.y - a.b.y),
      );
      return;
    }
    case 'ellipse': {
      ctx.strokeStyle = a.color;
      ctx.lineWidth = a.width;
      ctx.beginPath();
      ctx.ellipse(
        (a.a.x + a.b.x) / 2,
        (a.a.y + a.b.y) / 2,
        Math.abs(a.a.x - a.b.x) / 2,
        Math.abs(a.a.y - a.b.y) / 2,
        0,
        0,
        Math.PI * 2,
      );
      ctx.stroke();
      return;
    }
    case 'arrow': {
      ctx.strokeStyle = a.color;
      ctx.fillStyle = a.color;
      ctx.lineWidth = a.width;
      const ang = Math.atan2(a.b.y - a.a.y, a.b.x - a.a.x);
      const head = Math.max(a.width * 4, 12);
      // the shaft stops where the head starts, so the tip stays sharp
      const end = { x: a.b.x - Math.cos(ang) * head * 0.8, y: a.b.y - Math.sin(ang) * head * 0.8 };
      ctx.beginPath();
      ctx.moveTo(a.a.x, a.a.y);
      ctx.lineTo(end.x, end.y);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(a.b.x, a.b.y);
      ctx.lineTo(a.b.x - Math.cos(ang - 0.45) * head, a.b.y - Math.sin(ang - 0.45) * head);
      ctx.lineTo(a.b.x - Math.cos(ang + 0.45) * head, a.b.y - Math.sin(ang + 0.45) * head);
      ctx.closePath();
      ctx.fill();
      return;
    }
    case 'text': {
      ctx.font = fontOf(a);
      ctx.textBaseline = 'top';
      const lines = a.text.split('\n');
      if (a.box) {
        const r = textBounds(a, (l) => ctx.measureText(l).width);
        ctx.fillStyle = contrastOf(a.color);
        ctx.globalAlpha = 0.85;
        ctx.beginPath();
        ctx.roundRect?.(r.x, r.y, r.w, r.h, a.size * 0.2);
        if (!ctx.roundRect) ctx.rect(r.x, r.y, r.w, r.h);
        ctx.fill();
        ctx.globalAlpha = 1;
      }
      ctx.lineWidth = Math.max(2, a.size / 7);
      ctx.strokeStyle = contrastOf(a.color);
      ctx.fillStyle = a.color;
      lines.forEach((line, i) => {
        const y = a.at.y + i * a.size * 1.25;
        // without a box, an outline keeps the text readable on any picture
        if (!a.box) ctx.strokeText(line, a.at.x, y);
        ctx.fillText(line, a.at.x, y);
      });
      return;
    }
  }
}

export function drawAnnotations(ctx: CanvasRenderingContext2D, list: readonly Annotation[]): void {
  for (const a of list) {
    ctx.save();
    drawOne(ctx, a);
    ctx.restore();
  }
}

export interface PaintOptions {
  /** The shape being drawn right now. */
  live?: Annotation | null;
  /** A crop being dragged: the rest of the picture is dimmed. */
  cropPreview?: PxRect | null;
}

/**
 * Draw the visible part of the picture, scaled to fill the canvas, then the annotations on
 * top. Used for the screen and, at scale 1, for the exported file.
 */
export function paint(
  ctx: CanvasRenderingContext2D,
  image: CanvasImageSource,
  size: { width: number; height: number },
  state: EditorState,
  canvas: { width: number; height: number },
  opts: PaintOptions = {},
): void {
  const v = visibleRect(state, size);
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(image, v.x, v.y, v.w, v.h, 0, 0, canvas.width, canvas.height);
  ctx.save();
  ctx.scale(canvas.width / v.w, canvas.height / v.h);
  ctx.translate(-v.x, -v.y);
  drawAnnotations(ctx, state.annotations);
  if (opts.live) drawAnnotations(ctx, [opts.live]);
  if (opts.cropPreview) {
    const c = opts.cropPreview;
    ctx.fillStyle = 'rgba(0,0,0,0.55)';
    // Four bands around the chosen rectangle.
    ctx.fillRect(v.x, v.y, v.w, c.y - v.y);
    ctx.fillRect(v.x, c.y + c.h, v.w, v.y + v.h - (c.y + c.h));
    ctx.fillRect(v.x, c.y, c.x - v.x, c.h);
    ctx.fillRect(c.x + c.w, c.y, v.x + v.w - (c.x + c.w), c.h);
    ctx.strokeStyle = '#4f8ee0';
    ctx.lineWidth = Math.max(2, v.w / canvas.width * 2);
    ctx.setLineDash([8 * (v.w / canvas.width), 6 * (v.w / canvas.width)]);
    ctx.strokeRect(c.x, c.y, c.w, c.h);
  }
  ctx.restore();
}

/** The edited picture as a PNG file. */
export function exportPng(
  image: CanvasImageSource,
  size: { width: number; height: number },
  state: EditorState,
): Promise<Blob> {
  const v = visibleRect(state, size);
  const canvas = document.createElement('canvas');
  canvas.width = v.w;
  canvas.height = v.h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return Promise.reject(new Error('This view cannot draw pictures'));
  paint(ctx, image, size, state, canvas);
  return new Promise((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('Could not encode the picture'))), 'image/png'),
  );
}
