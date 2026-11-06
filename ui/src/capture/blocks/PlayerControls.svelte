<script lang="ts">
  // The playback buttons laid over the top of the recording. They are see-through and only
  // show while the pointer is over the picture or a button has keyboard focus, so the picture
  // keeps all its room. Each button is a plain, well-known sign with a name for screen readers.
  import { RATES } from './player';

  let {
    playing,
    rate,
    muted,
    volume,
    now,
    total,
    fullscreen = false,
    onplaypause,
    onstop,
    onslower,
    onfaster,
    onmute,
    onvolume,
    onfullscreen,
  }: {
    playing: boolean;
    /** 1 is normal speed. */
    rate: number;
    muted: boolean;
    /** 0 to 1. */
    volume: number;
    /** Where the picture is and how long the recording is, in seconds. */
    now: number;
    total: number;
    fullscreen?: boolean;
    onplaypause: () => void;
    onstop: () => void;
    onslower: () => void;
    onfaster: () => void;
    onmute: () => void;
    onvolume: (v: number) => void;
    onfullscreen: () => void;
  } = $props();

  const slowest = $derived(rate <= RATES[0]!);
  const fastest = $derived(rate >= RATES[RATES.length - 1]!);
  const silent = $derived(muted || volume === 0);
</script>

<div class="controls" role="toolbar" aria-label="Playback">
  <button type="button" class="b" title={playing ? 'Pause (Space)' : 'Play (Space)'} aria-label={playing ? 'Pause' : 'Play'} onclick={onplaypause}>
    <svg viewBox="0 0 24 24" aria-hidden="true">
      {#if playing}<path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" />{:else}<path d="M8 5v14l11-7z" />{/if}
    </svg>
  </button>
  <button type="button" class="b" title="Stop: back to the start" aria-label="Stop" onclick={onstop}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7h10v10H7z" /></svg>
  </button>
  <button type="button" class="b" title="Slower" aria-label="Slower" disabled={slowest} onclick={onslower}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M11 6v12l-8.5-6zM21.5 6v12L13 12z" /></svg>
  </button>
  <span class="rate" aria-live="polite" title="Playback speed">{rate}×</span>
  <button type="button" class="b" title="Faster" aria-label="Faster" disabled={fastest} onclick={onfaster}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M13 6v12l8.5-6zM2.5 6v12L11 12z" /></svg>
  </button>
  <span class="time" aria-label="Position">{now.toFixed(1)} / {total.toFixed(1)} s</span>
  <span class="gap"></span>
  <button type="button" class="b" title={silent ? 'Unmute (M)' : 'Mute (M)'} aria-label={silent ? 'Unmute' : 'Mute'} onclick={onmute}>
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 9.5v5h3.5L12 18.5v-13L7.5 9.5z" />
      {#if silent}
        <path class="line" d="M15.5 9.5l5 5M20.5 9.5l-5 5" />
      {:else}
        <path class="line" d="M15.5 9a4 4 0 010 6M18 6.5a8 8 0 010 11" />
      {/if}
    </svg>
  </button>
  <input
    class="vol"
    type="range"
    min="0"
    max="1"
    step="0.05"
    aria-label="Volume"
    value={muted ? 0 : volume}
    oninput={(e) => onvolume(Number(e.currentTarget.value))}
  />
  <button type="button" class="b" title={fullscreen ? 'Leave full screen (F)' : 'Full screen (F)'} aria-label={fullscreen ? 'Leave full screen' : 'Full screen'} onclick={onfullscreen}>
    <svg viewBox="0 0 24 24" aria-hidden="true">
      {#if fullscreen}
        <path class="line" d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />
      {:else}
        <path class="line" d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
      {/if}
    </svg>
  </button>
</div>

<style>
  .controls {
    position: absolute; top: 0; left: 0; right: 0; z-index: 3;
    display: flex; align-items: center; gap: 2px; padding: 6px 8px 22px;
    color: #fff; font-size: 12px;
    background: linear-gradient(to bottom, rgb(0 0 0 / 0.62), rgb(0 0 0 / 0));
    opacity: 0; pointer-events: none; transition: opacity 0.15s ease;
  }
  /* Shown while the pointer is over the picture or a button is focused (the parent sets `.hot`). */
  :global(.hot) > .controls, .controls:focus-within { opacity: 1; pointer-events: auto; }
  .b {
    width: 28px; height: 28px; padding: 0; display: inline-grid; place-items: center;
    background: transparent; border: 0; border-radius: 6px; color: inherit; box-shadow: none;
  }
  .b:hover:not(:disabled) { background: rgb(255 255 255 / 0.2); }
  .b:disabled { opacity: 0.4; }
  .b:focus-visible { outline: 2px solid #fff; outline-offset: -2px; }
  svg { width: 18px; height: 18px; fill: currentColor; stroke: none; }
  svg .line { fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .rate { min-width: 34px; text-align: center; font-variant-numeric: tabular-nums; }
  .time { margin-left: 8px; font-variant-numeric: tabular-nums; opacity: 0.9; white-space: nowrap; }
  .gap { flex: 1; }
  .vol { width: 64px; margin: 0 4px; accent-color: #fff; background: transparent; }
</style>
