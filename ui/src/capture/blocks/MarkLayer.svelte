<script lang="ts">
  // The mark over the recording: a ring with a text under it, where the person clicked. It is
  // drawn here the way `drawMark` will draw it, so what is seen is what the recording gets.
  // Click to move it; with the ring focused the arrow keys move it too (Shift: a bigger step).
  import { onMount } from 'svelte';
  import { containRect } from './contain';
  import { markFontSize, markLineWidth, markRadius, type MarkAt } from './drawMark';

  /** `active` is false while another tool has the mouse: the mark is shown but cannot be moved. */
  let { width, height, at = $bindable(), text, active = true }: { width: number; height: number; at: MarkAt; text: string; active?: boolean } = $props();

  let layer = $state<HTMLElement | null>(null);
  let space = $state({ w: 0, h: 0 });

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

  const shown = $derived(containRect(space, width, height));
  /** CSS pixels for one pixel of the recording. */
  const k = $derived(shown.w > 0 ? shown.w / width : 1);
  const clamp = (n: number, hi: number) => Math.min(Math.max(n, 0), hi);

  function place(e: MouseEvent) {
    if (!active || !layer || shown.w <= 0) return;
    const box = layer.getBoundingClientRect();
    const x = ((e.clientX - box.left - shown.left) / shown.w) * width;
    const y = ((e.clientY - box.top - shown.top) / shown.h) * height;
    if (x < 0 || y < 0 || x > width || y > height) return;
    at = { x: Math.round(x), y: Math.round(y) };
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 50 : 10;
    const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
    const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0;
    if (!dx && !dy) return;
    e.preventDefault();
    at = { x: clamp(at.x + dx, width), y: clamp(at.y + dy, height) };
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="layer" class:idle={!active} bind:this={layer} onclick={place}>
  <div class="frame" style:left="{shown.left}px" style:top="{shown.top}px" style:width="{shown.w}px" style:height="{shown.h}px">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="ring"
      role="group"
      tabindex={active ? 0 : -1}
      aria-label="Mark at {at.x},{at.y}. Click the picture or use the arrow keys to move it."
      style:left="{(at.x / width) * 100}%"
      style:top="{(at.y / height) * 100}%"
      style:width="{2 * markRadius(width, height) * k}px"
      style:height="{2 * markRadius(width, height) * k}px"
      style:border-width="{Math.max(2, markLineWidth(height) * k)}px"
      onkeydown={(e) => active && key(e)}
    >
      {#if text.trim()}
        <span class="label" style:font-size="{markFontSize(height) * k}px" style:margin-top="{(markFontSize(height) / 3) * k}px">{text}</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .layer { position: absolute; inset: 0; cursor: crosshair; }
  .layer.idle { pointer-events: none; cursor: inherit; }
  .frame { position: absolute; overflow: hidden; pointer-events: none; }
  .ring { position: absolute; transform: translate(-50%, -50%); box-sizing: border-box; border-style: solid; border-color: #e5322d; border-radius: 50%; pointer-events: auto; }
  .idle .ring { pointer-events: none; }
  .ring:focus-visible { outline: 2px solid #fff; outline-offset: 2px; }
  .label { position: absolute; top: 100%; left: 50%; transform: translateX(-50%); white-space: nowrap; font-weight: 700; color: #e5322d; text-shadow: -1px -1px 0 #fff, 1px -1px 0 #fff, -1px 1px 0 #fff, 1px 1px 0 #fff; pointer-events: none; }
</style>
