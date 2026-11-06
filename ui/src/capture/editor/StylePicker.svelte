<script lang="ts">
  // One small button in the toolbar that shows the current colour and opens a panel with the
  // colours and the line thickness, so the toolbar stays on one line. It closes on a click
  // elsewhere, on Escape, and once a colour is picked.
  import type { Thickness } from './model';

  let {
    colors,
    thicknesses,
    color = $bindable(),
    thickness = $bindable(),
  }: { colors: string[]; thicknesses: Thickness[]; color: string; thickness: Thickness } = $props();

  let open = $state(false);
  let root = $state<HTMLElement | null>(null);

  function away(e: MouseEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  function key(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      // the capture window closes on Escape; with the panel open it only closes the panel
      e.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window onclick={away} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="style" bind:this={root} onkeydown={key}>
  <button
    type="button"
    class="dot"
    aria-haspopup="true"
    aria-expanded={open}
    title="Colour and thickness"
    aria-label="Colour and thickness"
    onclick={() => (open = !open)}
  >
    <span class="chip" style:background={color}></span>
  </button>
  {#if open}
    <div class="panel" role="group" aria-label="Colour and thickness">
      <div class="row" role="group" aria-label="Colour">
        {#each colors as c (c)}
          <button
            type="button"
            class="swatch"
            class:on={color === c}
            style:--sw={c}
            aria-label="Colour {c}"
            aria-pressed={color === c}
            onclick={() => { color = c; open = false; }}
          ></button>
        {/each}
      </div>
      <div class="row" role="group" aria-label="Thickness">
        {#each thicknesses as t (t)}
          <button type="button" class="thick" class:on={thickness === t} aria-pressed={thickness === t} title={t} onclick={() => (thickness = t)}>
            <span class="bar {t}"></span>
            <span class="sr-only">{t}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .style { position: relative; display: inline-flex; }
  .dot { width: 30px; height: 28px; padding: 0; display: inline-grid; place-items: center; }
  .chip { width: 16px; height: 16px; border-radius: 50%; border: 2px solid var(--border-strong); box-sizing: border-box; }
  .panel {
    position: absolute; top: calc(100% + 4px); left: 0; z-index: 20; display: grid; gap: 8px;
    padding: 8px; background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius); box-shadow: 0 4px 14px rgb(0 0 0 / 0.3);
  }
  .row { display: flex; gap: 6px; align-items: center; }
  .swatch { width: 22px; height: 22px; padding: 0; border-radius: 50%; background: var(--sw); border: 2px solid var(--border-strong); }
  .swatch.on { outline: 2px solid var(--accent); outline-offset: 2px; }
  .thick { width: 34px; height: 26px; padding: 0; display: inline-grid; place-items: center; }
  .thick.on { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .bar { display: block; width: 18px; background: currentColor; border-radius: 2px; }
  .bar.thin { height: 2px; }
  .bar.medium { height: 4px; }
  .bar.thick { height: 7px; }
</style>
