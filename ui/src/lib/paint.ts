/** Run `f` once the browser has painted what was just rendered: after two animation frames. */
export function afterPaint(f: () => void): void {
  if (typeof requestAnimationFrame !== 'function') {
    f();
    return;
  }
  requestAnimationFrame(() => requestAnimationFrame(f));
}
