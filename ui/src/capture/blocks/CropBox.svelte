<script lang="ts">
  // The box drawn over the recording to crop it, as on a screenshot: drag over the picture to
  // draw the part to keep (the rest is dimmed), then apply. `crop` is in the recording's own
  // pixels, `null` until a box is drawn; the recording is shown "contained" in its space, so the
  // picture's real place is worked out from the size of the space. Drawn, the box can be dragged
  // to move it and by its corner to resize it; the arrow keys move it (Shift: a bigger step) and
  // Alt with an arrow resizes it. Sizes stay even, as the encoder needs.
  import { onMount } from 'svelte';
  import { containRect } from './contain';

  type Box = { x: number; y: number; w: number; h: number };
  let { width, height, crop = $bindable() }: { width: number; height: number; crop: Box | null } = $props();

  const MIN = 16;
  const clamp = (n: number, lo: number, hi: number) => Math.min(Math.max(n, lo), Math.max(lo, hi));
  const even = (n: number) => Math.round(n / 2) * 2;

  let layer = $state<HTMLElement | null>(null);
  let space = $state({ w: 0, h: 0 });
  let drag: { mode: 'move' | 'size'; sx: number; sy: number; from: Box } | null = null;
  /** A box being drawn: from where the mouse went down, in the recording's pixels. */
  let drawing: { x: number; y: number; before: Box | null } | null = null;
  let active = $state(false);

  onMount(() => {
    if (!layer || typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => measure());
    watch.observe(layer);
    measure();
    return () => watch.disconnect();
  });
  function measure() {
    if (layer) space = { w: layer.clientWidth, h: layer.clientHeight };
  }

  /** Where the picture really is inside the space, in CSS pixels. */
  const shown = $derived(containRect(space, width, height));
  const scale = $derived(shown.w > 0 ? width / shown.w : 1);

  function begin(e: MouseEvent, mode: 'move' | 'size') {
    if (!crop) return;
    e.preventDefault();
    e.stopPropagation();
    drag = { mode, sx: e.clientX, sy: e.clientY, from: { ...crop } };
    active = true;
  }
  /** The point of the recording under the mouse, kept on the picture. */
  function pointOf(e: MouseEvent) {
    const r = layer!.getBoundingClientRect();
    return {
      x: clamp(((e.clientX - r.left - shown.left) / Math.max(1, shown.w)) * width, 0, width),
      y: clamp(((e.clientY - r.top - shown.top) / Math.max(1, shown.h)) * height, 0, height),
    };
  }
  function beginDraw(e: MouseEvent) {
    if (e.button !== 0 || !layer || shown.w <= 0) return;
    e.preventDefault();
    const p = pointOf(e);
    drawing = { ...p, before: crop ? { ...crop } : null };
    active = true;
  }
  function onMove(e: MouseEvent) {
    if (drawing) {
      const p = pointOf(e);
      const x = even(Math.min(p.x, drawing.x));
      const y = even(Math.min(p.y, drawing.y));
      const w = even(Math.abs(p.x - drawing.x));
      const h = even(Math.abs(p.y - drawing.y));
      if (w >= MIN && h >= MIN) crop = { x, y, w: Math.min(w, width - x), h: Math.min(h, height - y) };
      return;
    }
    if (!drag || !crop) return;
    const dx = (e.clientX - drag.sx) * scale;
    const dy = (e.clientY - drag.sy) * scale;
    const f = drag.from;
    if (drag.mode === 'move') {
      crop = { ...f, x: even(clamp(f.x + dx, 0, width - f.w)), y: even(clamp(f.y + dy, 0, height - f.h)) };
    } else {
      crop = { ...f, w: even(clamp(f.w + dx, MIN, width - f.x)), h: even(clamp(f.h + dy, MIN, height - f.y)) };
    }
  }
  function end() {
    drag = null;
    drawing = null;
    active = false;
  }
  function key(e: KeyboardEvent) {
    if (!crop) return;
    const step = e.shiftKey ? 50 : 10;
    const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
    const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0;
    if (!dx && !dy) return;
    e.preventDefault();
    if (e.altKey) {
      crop = { ...crop, w: even(clamp(crop.w + dx, MIN, width - crop.x)), h: even(clamp(crop.h + dy, MIN, height - crop.y)) };
    } else {
      crop = { ...crop, x: even(clamp(crop.x + dx, 0, width - crop.w)), y: even(clamp(crop.y + dy, 0, height - crop.h)) };
    }
  }
</script>

<svelte:window onmousemove={onMove} onmouseup={end} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="layer" bind:this={layer} onmousedown={beginDraw}>
  <div class="frame" class:empty={!crop} style:left="{shown.left}px" style:top="{shown.top}px" style:width="{shown.w}px" style:height="{shown.h}px">
    {#if crop}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="box"
        class:active
        role="group"
        tabindex="0"
        aria-label="Crop area: {crop.w}×{crop.h}, from {crop.x},{crop.y}. Arrow keys move it, Alt with an arrow resizes it."
        style:left="{(crop.x / width) * 100}%"
        style:top="{(crop.y / height) * 100}%"
        style:width="{(crop.w / width) * 100}%"
        style:height="{(crop.h / height) * 100}%"
        onmousedown={(e) => begin(e, 'move')}
        onkeydown={key}
      >
        <span class="grip" aria-label="Resize the crop area" role="presentation" onmousedown={(e) => begin(e, 'size')}></span>
      </div>
    {/if}
  </div>
</div>

<style>
  .layer { position: absolute; inset: 0; cursor: crosshair; }
  .frame { position: absolute; overflow: hidden; }
  /* nothing drawn yet: the whole picture is dimmed a little, as an invitation to draw */
  .frame.empty { background: rgba(0, 0, 0, 0.25); }
  .box { position: absolute; box-sizing: border-box; border: 2px solid var(--accent); box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.5); cursor: move; pointer-events: auto; }
  .box:focus-visible, .box.active { outline: 2px solid #fff; outline-offset: 1px; }
  .grip { position: absolute; right: 0; bottom: 0; width: 12px; height: 12px; background: var(--accent); border: 2px solid #fff; border-radius: 2px; box-sizing: border-box; cursor: nwse-resize; }
</style>
