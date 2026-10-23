import { describe, expect, it } from 'vitest';
import {
  add, clampRect, clearAll, emptyState, exportSize, isEdited, isMeaningful, rectOf, redo,
  resetCrop, setCrop, strokeWidth, textSize, toPicturePoint, undo, unit, visibleRect,
  type Annotation,
} from './model';

const SIZE = { width: 1920, height: 1080 };
const rect: Annotation = { kind: 'rect', color: '#f00', width: 4, a: { x: 10, y: 10 }, b: { x: 100, y: 80 } };
const pen: Annotation = { kind: 'pen', color: '#f00', width: 4, points: [{ x: 1, y: 1 }, { x: 9, y: 9 }] };

describe('geometry', () => {
  it('rectOf works whichever way the corners are dragged', () => {
    expect(rectOf({ x: 50, y: 60 }, { x: 10, y: 20 })).toEqual({ x: 10, y: 20, w: 40, h: 40 });
  });

  it('clampRect keeps the part on the picture, in whole pixels', () => {
    expect(clampRect({ x: -10, y: 5.4, w: 100, h: 5000 }, SIZE)).toEqual({ x: 0, y: 5, w: 90, h: 1075 });
    expect(clampRect({ x: 5000, y: 0, w: 10, h: 10 }, SIZE).w).toBe(0);
  });

  it('maps a screen position to the picture, through the crop', () => {
    const shown = { left: 100, top: 50, width: 400, height: 200 };
    expect(toPicturePoint({ x: 300, y: 150 }, shown, { x: 0, y: 0, w: 1920, h: 1080 })).toEqual({ x: 960, y: 540 });
    // 400x200 shown for a crop starting at (500, 300) of size 800x400
    expect(toPicturePoint({ x: 300, y: 150 }, shown, { x: 500, y: 300, w: 800, h: 400 })).toEqual({ x: 900, y: 500 });
    expect(toPicturePoint({ x: 1, y: 1 }, { left: 0, top: 0, width: 0, height: 0 }, { x: 0, y: 0, w: 10, h: 10 })).toEqual({ x: 0, y: 0 });
  });

  it('sizes scale with the picture', () => {
    expect(unit({ width: 1920, height: 1080 })).toBe(4);
    expect(unit({ width: 3840, height: 2160 })).toBe(7);
    expect(unit({ width: 100, height: 80 })).toBe(2);
    expect(strokeWidth(SIZE, 'thick')).toBe(16);
    expect(textSize(SIZE, 'medium')).toBe(36);
  });
});

describe('crop', () => {
  it('shows the whole picture until a crop is set', () => {
    expect(visibleRect(emptyState(), SIZE)).toEqual({ x: 0, y: 0, w: 1920, h: 1080 });
  });

  it('ignores accidental tiny crops and a crop of everything', () => {
    const s = emptyState();
    expect(setCrop(s, { x: 5, y: 5, w: 3, h: 300 }, SIZE)).toBe(s);
    expect(setCrop(s, { x: 0, y: 0, w: 1920, h: 1080 }, SIZE).crop).toBeNull();
  });

  it('keeps a real crop, reports the export size, and resets', () => {
    const s = setCrop(emptyState(), { x: 100, y: 50, w: 640, h: 360 }, SIZE);
    expect(s.crop).toEqual({ x: 100, y: 50, w: 640, h: 360 });
    expect(exportSize(s, SIZE)).toEqual({ width: 640, height: 360 });
    expect(isEdited(s)).toBe(true);
    expect(resetCrop(s).crop).toBeNull();
  });
});

describe('annotations', () => {
  it('drops clicks that draw nothing', () => {
    expect(isMeaningful({ ...rect, b: { x: 11, y: 11 } })).toBe(false);
    expect(isMeaningful({ ...pen, points: [{ x: 1, y: 1 }] })).toBe(false);
    expect(isMeaningful({ kind: 'text', color: '#fff', size: 20, at: { x: 0, y: 0 }, text: '   ' })).toBe(false);
    const s = emptyState();
    expect(add(s, { ...rect, b: { x: 11, y: 11 } })).toBe(s);
  });

  it('undo and redo walk the history; drawing something new clears redo', () => {
    let s = add(add(emptyState(), rect), pen);
    expect(s.annotations).toHaveLength(2);
    s = undo(s);
    expect(s.annotations).toEqual([rect]);
    expect(s.redo).toEqual([pen]);
    s = redo(s);
    expect(s.annotations).toEqual([rect, pen]);
    s = undo(undo(s));
    expect(s.annotations).toHaveLength(0);
    expect(undo(s)).toBe(s);
    s = add(s, rect);
    expect(s.redo).toEqual([]);
    expect(redo(s)).toBe(s);
  });

  it('clearAll removes annotations and crop', () => {
    expect(isEdited(clearAll())).toBe(false);
  });
});
