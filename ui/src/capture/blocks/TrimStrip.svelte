<script lang="ts">
  // The strip under the recording: the part that is kept is shaded, its two ends have handles,
  // a thin line follows the playing position, and a click on the strip moves the picture there.
  // All times are in seconds, in tenths; at least half a second is always kept.
  let {
    duration,
    start = $bindable(0),
    end = $bindable(0),
    now = 0,
    onseek,
  }: { duration: number; start: number; end: number; now?: number; onseek?: (secs: number) => void } = $props();

  const MIN_KEPT = 0.5;
  const clamp = (n: number, lo: number, hi: number) => Math.min(Math.max(n, lo), Math.max(lo, hi));
  const tenth = (n: number) => Math.round(n * 10) / 10;
  const pct = (secs: number) => (duration > 0 ? clamp(secs / duration, 0, 1) * 100 : 0);

  let strip = $state<HTMLElement | null>(null);
  let dragging = $state<'start' | 'end' | null>(null);

  /** Move one end, keeping the other where it is and at least MIN_KEPT between them. */
  export function setEdge(which: 'start' | 'end', secs: number) {
    if (which === 'start') {
      const to = clamp(secs, 0, end - MIN_KEPT);
      start = to < 0.05 ? 0 : tenth(to);
    } else {
      const to = clamp(secs, start + MIN_KEPT, duration);
      end = to > duration - 0.05 ? duration : Math.min(tenth(to), duration);
    }
  }

  function secsAt(clientX: number): number | null {
    if (!strip) return null;
    const box = strip.getBoundingClientRect();
    if (box.width <= 0) return null;
    return clamp((clientX - box.left) / box.width, 0, 1) * duration;
  }

  function grab(e: MouseEvent, which: 'start' | 'end') {
    e.preventDefault();
    e.stopPropagation();
    dragging = which;
  }
  function onMove(e: MouseEvent) {
    if (!dragging) return;
    const secs = secsAt(e.clientX);
    if (secs !== null) setEdge(dragging, secs);
  }
  function release() {
    dragging = null;
  }
  function seek(e: MouseEvent) {
    const secs = secsAt(e.clientX);
    if (secs !== null) onseek?.(secs);
  }
  function key(e: KeyboardEvent, which: 'start' | 'end') {
    const step = e.shiftKey ? 1 : 0.1;
    const at = which === 'start' ? start : end;
    if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') setEdge(which, at - step);
    else if (e.key === 'ArrowRight' || e.key === 'ArrowUp') setEdge(which, at + step);
    else if (e.key === 'Home') setEdge(which, 0);
    else if (e.key === 'End') setEdge(which, duration);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onmousemove={onMove} onmouseup={release} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="strip" role="presentation" aria-label="Timeline" bind:this={strip} onclick={seek}>
  <div class="kept" style:left="{pct(start)}%" style:width="{Math.max(0, pct(end) - pct(start))}%"></div>
  <span class="playhead" aria-hidden="true" style:left="{pct(now)}%"></span>
  {#each [{ which: 'start' as const, label: 'Start of the part to keep', value: start }, { which: 'end' as const, label: 'End of the part to keep', value: end }] as edge (edge.which)}
    <span
      class="edge"
      class:active={dragging === edge.which}
      role="slider"
      tabindex="0"
      aria-label={edge.label}
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={edge.value}
      aria-valuetext="{edge.value.toFixed(1)} seconds"
      style:left="{pct(edge.value)}%"
      onmousedown={(e) => grab(e, edge.which)}
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => key(e, edge.which)}
    ></span>
  {/each}
</div>

<style>
  /* A slim track the full width of the picture. The box is taller than the track so it is easy to
     hit; the handles are small and only a little taller than the track. */
  .strip { position: relative; box-sizing: border-box; width: calc(100% - 12px); height: 16px; margin: 0 6px; cursor: pointer; }
  .strip::before { content: ''; position: absolute; left: 0; right: 0; top: 5px; height: 6px; background: var(--surface-2); border: 1px solid var(--border); border-radius: 3px; box-sizing: border-box; }
  .kept { position: absolute; top: 5px; height: 6px; background: color-mix(in srgb, var(--accent) 55%, transparent); border-radius: 3px; pointer-events: none; }
  .playhead { position: absolute; top: 2px; bottom: 2px; width: 2px; margin-left: -1px; background: var(--text); border-radius: 1px; pointer-events: none; }
  .edge { position: absolute; top: 2px; bottom: 2px; width: 8px; margin-left: -4px; background: var(--accent); border: 1.5px solid #fff; border-radius: 3px; box-sizing: border-box; cursor: ew-resize; box-shadow: 0 0 0 1px rgb(0 0 0 / 0.25); }
  .edge:focus-visible, .edge.active { outline: 2px solid var(--text); outline-offset: 1px; }
</style>
