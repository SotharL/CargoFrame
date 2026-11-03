/** Where a picture really is inside a space that shows it "contained" (all of it, centred, never stretched). */
export interface Shown {
  left: number;
  top: number;
  w: number;
  h: number;
}

export function containRect(space: { w: number; h: number }, width: number, height: number): Shown {
  if (space.w <= 0 || space.h <= 0 || width <= 0 || height <= 0) return { left: 0, top: 0, w: 0, h: 0 };
  const k = Math.min(space.w / width, space.h / height);
  const w = width * k;
  const h = height * k;
  return { left: (space.w - w) / 2, top: (space.h - h) / 2, w, h };
}
