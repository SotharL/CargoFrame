<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { getApi } from '../../lib/ipc';
  import type { ClipTrim, VideoClipInfo } from '../../lib/types';
  import { formatSize } from '../values';
  import TrimStrip from './TrimStrip.svelte';

  /**
   * `trim` is what the person chose to keep: `null` while all of the recording is kept, which is
   * what the share sends along as "cut nothing".
   */
  let { info, trim = $bindable(null) }: { info: VideoClipInfo; trim?: ClipTrim | null } = $props();

  const total = $derived(info.duration_ms / 1000);
  let url = $state<string | null>(null);
  let problem = $state('');
  let cannotPlay = $state(false);
  let player = $state<HTMLVideoElement | null>(null);
  let now = $state(0);
  let start = $state(0);
  // The block is made again for each capture, so the recording's length is read once.
  let end = $state(untrack(() => info.duration_ms / 1000));
  let strip = $state<{ setEdge: (which: 'start' | 'end', secs: number) => void } | null>(null);

  const kept = $derived(Math.max(0, end - start));
  const wholeClip = $derived(start <= 0 && end >= total);

  $effect(() => {
    trim = wholeClip ? null : { start_ms: Math.round(start * 1000), end_ms: end >= total ? null : Math.round(end * 1000) };
  });

  onMount(async () => {
    try {
      const blob = await (await getApi()).getSessionClip();
      url = URL.createObjectURL(blob);
    } catch (e) {
      problem = String(e);
    }
  });
  onDestroy(() => {
    if (url) URL.revokeObjectURL(url);
  });

  function seek(secs: number) {
    if (!player) return;
    player.currentTime = secs;
    now = secs;
  }
  /** Playing stays inside the part that is kept: it stops at the end and starts again from the start. */
  function tick() {
    if (!player) return;
    now = player.currentTime;
    if (now >= end && !player.paused) {
      player.pause();
      seek(end);
    }
  }
  function played() {
    if (player && (player.currentTime < start || player.currentTime >= end - 0.05)) seek(start);
  }
  const startHere = () => strip?.setEdge('start', now);
  const endHere = () => strip?.setEdge('end', now);
  function keepAll() {
    start = 0;
    end = total;
  }
</script>

<div class="clip">
  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {:else if cannotPlay}
    <p class="quiet note" role="status">This window cannot play the recording, but it is there and will be sent.</p>
  {:else if url}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={player}
      src={url}
      controls
      muted
      aria-label="Recording"
      ontimeupdate={tick}
      onplay={played}
      onerror={() => (cannotPlay = true)}
    ></video>
  {:else}
    <p class="quiet note">Loading the recording…</p>
  {/if}
  {#if !problem}
    <div class="trim">
      <TrimStrip bind:this={strip} duration={total} bind:start bind:end {now} onseek={seek} />
      <div class="row">
        <button type="button" class="small" onclick={startHere} disabled={!url || cannotPlay}>Start here</button>
        <button type="button" class="small" onclick={endHere} disabled={!url || cannotPlay}>End here</button>
        <button type="button" class="small" onclick={keepAll} disabled={wholeClip}>Keep it all</button>
        <span class="quiet meta" aria-live="polite">
          {#if wholeClip}
            {total.toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}
          {:else}
            Keeping {start.toFixed(1)} s to {end.toFixed(1)} s ({kept.toFixed(1)} s of {total.toFixed(1)} s)
          {/if}
        </span>
      </div>
    </div>
  {:else}
    <p class="quiet meta">{total.toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}</p>
  {/if}
</div>

<style>
  .clip { height: 100%; min-height: 0; display: grid; grid-template-rows: 1fr auto; gap: 4px; }
  video { width: 100%; height: 100%; min-height: 0; object-fit: contain; background: #000; border-radius: var(--radius-sm); }
  .note, .problem { margin: 0; padding: 12px; font-size: 13px; }
  .problem { color: var(--err); }
  .meta { margin: 0; font-size: 12px; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .small { padding: 2px 8px; font-size: 12px; }
</style>
