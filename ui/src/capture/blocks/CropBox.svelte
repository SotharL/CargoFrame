<script lang="ts">
  // The box drawn over the recording to crop it. `crop` is in the recording's own pixels; the
  // recording is shown "contained" in its space, so the picture's real place is worked out from
  // the size of the space. Drag the box to move it, its corner to resize it; the arrow keys move
  // it (Shift: a bigger step) and Alt with an arrow resizes it. Sizes stay even, as the encoder needs.
  import { onMount } from 'svelte';

  type Box = { x: number; y: number; w: number; h: number };
  let { width, height, crop = $bindable() }: { width: number; height: number; crop: Box } = $props();

  const MIN = 16;
  const clamp = (n: number, lo: number, hi: number) => Math.min(Math.max(n, lo), Math.max(lo, hi));
  const even = (n: number) => Math.round(n / 2) * 2;

  let layer = $state<HTMLElement | null>(null);
  let space = $state({ w: 0, h: 0 });
  let drag: { mode: 'move' | 'size'; sx: number; sy: number; from: Box } | null = null;
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
  const shown = $derived.by(() => {
    if (space.w <= 0 || space.h <= 0 || width <= 0 || height <= 0) return { left: 0, top: 0, w: 0, h: 0 };
    const k = Math.min(space.w / width, space.h / height);
    const w = width * k;
    const h = height * k;
    return { left: (space.w - w) / 2, top: (space.h - h) / 2, w, h };
  });
  const scale = $derived(shown.w > 0 ? width / shown.w : 1);

  function begin(e: MouseEvent, mode: 'move' | 'size') {
    e.preventDefault();
    e.stopPropagation();
    drag = { mode, sx: e.clientX, sy: e.clientY, from: { ...crop } };
    active = true;
  }
  function onMove(e: MouseEvent) {
    if (!drag) return;
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
    active = false;
  }
  function key(e: KeyboardEvent) {
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

<div class="layer" bind:this={layer}>
  <div class="frame" style:left="{shown.left}px" style:top="{shown.top}px" style:width="{shown.w}px" style:height="{shown.h}px">
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
  </div>
</div>

<style>
  .layer { position: absolute; inset: 0; pointer-events: none; }
  .frame { position: absolute; overflow: hidden; }
  .box { position: absolute; box-sizing: border-box; border: 2px solid var(--accent); box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.5); cursor: move; pointer-events: auto; }
  .box:focus-visible, .box.active { outline: 2px solid #fff; outline-offset: 1px; }
  .grip { position: absolute; right: 0; bottom: 0; width: 12px; height: 12px; background: var(--accent); border: 2px solid #fff; border-radius: 2px; box-sizing: border-box; cursor: nwse-resize; }
</style>
