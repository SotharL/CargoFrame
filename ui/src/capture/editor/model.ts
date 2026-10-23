/**
 * The screenshot editor's data and rules, with no drawing code so they can be tested without
 * a canvas. All coordinates are in pixels of the original screenshot, so the picture can be
 * zoomed or cropped for display without changing what was drawn.
 */

export interface Pt {
  x: number;
  y: number;
}

export interface PxRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Annotation =
  | { kind: 'pen'; color: string; width: number; points: Pt[] }
  | { kind: 'rect'; color: string; width: number; a: Pt; b: Pt }
  | { kind: 'ellipse'; color: string; width: number; a: Pt; b: Pt }
  | { kind: 'text'; color: string; size: number; at: Pt; text: string };

export interface EditorState {
  /** Visible part of the picture; `null` shows all of it. */
  crop: PxRect | null;
  annotations: Annotation[];
  /** Removed by undo, available to redo until something new is drawn. */
  redo: Annotation[];
}

export const emptyState = (): EditorState => ({ crop: null, annotations: [], redo: [] });

/** The smallest crop worth keeping; anything smaller is treated as an accidental click. */
export const MIN_CROP = 8;

/** Colours offered in the toolbar. The first is the default. */
export const COLORS = ['#e04f4f', '#e0934f', '#f2d33b', '#62e04f', '#4f8ee0', '#b04fe0', '#ffffff', '#1f1e1d'];

/** Stroke and text sizes scale with the picture so they look the same on a 1080p and a 4K shot. */
export function unit(size: { width: number; height: number }): number {
  return Math.max(2, Math.round(Math.min(size.width, size.height) / 300));
}

export type Thickness = 'thin' | 'medium' | 'thick';
export const THICKNESS: Thickness[] = ['thin', 'medium', 'thick'];

export function strokeWidth(size: { width: number; height: number }, t: Thickness): number {
  const mult = { thin: 1, medium: 2, thick: 4 }[t];
  return unit(size) * mult;
}

export function textSize(size: { width: number; height: number }, t: Thickness): number {
  const mult = { thin: 6, medium: 9, thick: 14 }[t];
  return unit(size) * mult;
}

/** The rectangle spanned by two corners, whichever way they were dragged. */
export function rectOf(a: Pt, b: Pt): PxRect {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.abs(a.x - b.x), h: Math.abs(a.y - b.y) };
}

export function clampPoint(p: Pt, size: { width: number; height: number }): Pt {
  return {
    x: Math.min(Math.max(p.x, 0), size.width),
    y: Math.min(Math.max(p.y, 0), size.height),
  };
}

/** Keep only the part of `r` that lies on the picture, in whole pixels. */
export function clampRect(r: PxRect, size: { width: number; height: number }): PxRect {
  const x0 = Math.max(0, Math.round(r.x));
  const y0 = Math.max(0, Math.round(r.y));
  const x1 = Math.min(size.width, Math.round(r.x + r.w));
  const y1 = Math.min(size.height, Math.round(r.y + r.h));
  return { x: x0, y: y0, w: Math.max(0, x1 - x0), h: Math.max(0, y1 - y0) };
}

/** What is shown and exported: the crop, or the whole picture. */
export function visibleRect(state: EditorState, size: { width: number; height: number }): PxRect {
  return state.crop ?? { x: 0, y: 0, w: size.width, h: size.height };
}

/** Screen position inside the displayed canvas to a point on the picture. */
export function toPicturePoint(
  client: Pt,
  shown: { left: number; top: number; width: number; height: number },
  visible: PxRect,
): Pt {
  const fx = shown.width === 0 ? 0 : (client.x - shown.left) / shown.width;
  const fy = shown.height === 0 ? 0 : (client.y - shown.top) / shown.height;
  return { x: visible.x + fx * visible.w, y: visible.y + fy * visible.h };
}

export function setCrop(
  state: EditorState,
  r: PxRect,
  size: { width: number; height: number },
): EditorState {
  const c = clampRect(r, size);
  if (c.w < MIN_CROP || c.h < MIN_CROP) return state;
  if (c.x === 0 && c.y === 0 && c.w === size.width && c.h === size.height) {
    return { ...state, crop: null };
  }
  return { ...state, crop: c };
}

export const resetCrop = (state: EditorState): EditorState => ({ ...state, crop: null });

/** A shape is worth keeping once it has some size; a click without a drag is dropped. */
export function isMeaningful(a: Annotation): boolean {
  switch (a.kind) {
    case 'pen':
      return a.points.length >= 2;
    case 'rect':
    case 'ellipse':
      return Math.abs(a.a.x - a.b.x) >= 3 && Math.abs(a.a.y - a.b.y) >= 3;
    case 'text':
      return a.text.trim().length > 0;
  }
}

export function add(state: EditorState, a: Annotation): EditorState {
  if (!isMeaningful(a)) return state;
  return { ...state, annotations: [...state.annotations, a], redo: [] };
}

export function undo(state: EditorState): EditorState {
  const last = state.annotations[state.annotations.length - 1];
  if (!last) return state;
  return {
    ...state,
    annotations: state.annotations.slice(0, -1),
    redo: [...state.redo, last],
  };
}

export function redo(state: EditorState): EditorState {
  const next = state.redo[state.redo.length - 1];
  if (!next) return state;
  return {
    ...state,
    annotations: [...state.annotations, next],
    redo: state.redo.slice(0, -1),
  };
}

/** Remove every annotation and the crop. */
export const clearAll = (): EditorState => emptyState();

export const isEdited = (s: EditorState): boolean => s.crop !== null || s.annotations.length > 0;

/** Pixel size of the exported picture. */
export function exportSize(state: EditorState, size: { width: number; height: number }) {
  const v = visibleRect(state, size);
  return { width: v.w, height: v.h };
}
