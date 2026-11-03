<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { getApi } from '../../lib/ipc';
  import type { ClipEdit, VideoClipInfo } from '../../lib/types';
  import { formatSize } from '../values';
  import TrimStrip from './TrimStrip.svelte';
  import CropBox from './CropBox.svelte';

  /**
   * `edit` is what the person chose: the part to keep and perhaps an area to crop to. It is
   * `null` while nothing is changed, which is what the share sends along as "send it as it is".
   */
  let { info, edit = $bindable(null) }: { info: VideoClipInfo; edit?: ClipEdit | null } = $props();

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

  let crop = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let playing = $state(false);

  const kept = $derived(Math.max(0, end - start));
  const wholeClip = $derived(start <= 0 && end >= total && crop === null);

  $effect(() => {
    edit = wholeClip
      ? null
      : {
          start_ms: Math.round(start * 1000),
          end_ms: end >= total ? null : Math.round(end * 1000),
          crop: crop ? { ...crop } : null,
        };
  });

  /** Cropping starts from a box inside the picture, so there is something to see and to drag. */
  function toggleCrop(on: boolean) {
    if (!on) {
      crop = null;
      return;
    }
    const even = (n: number) => Math.round(n / 2) * 2;
    const w = even(info.width * 0.8);
    const h = even(info.height * 0.8);
    crop = { x: even((info.width - w) / 2), y: even((info.height - h) / 2), w, h };
  }
  function playOrPause() {
    if (!player) return;
    if (player.paused) void player.play();
    else player.pause();
  }

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
    crop = null;
  }
</script>

<div class="clip">
  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {:else if cannotPlay}
    <p class="quiet note" role="status">This window cannot play the recording, but it is there and will be sent.</p>
  {:else if url}
    <div class="stage">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={player}
        src={url}
        controls={crop === null}
        muted
        aria-label="Recording"
        ontimeupdate={tick}
        onplay={() => { playing = true; played(); }}
        onpause={() => (playing = false)}
        onerror={() => (cannotPlay = true)}
      ></video>
      {#if crop}<CropBox width={info.width} height={info.height} bind:crop />{/if}
    </div>
  {:else}
    <p class="quiet note">Loading the recording…</p>
  {/if}
  {#if !problem}
    <div class="trim">
      <TrimStrip bind:this={strip} duration={total} bind:start bind:end {now} onseek={seek} />
      <div class="row">
        <button type="button" class="small" onclick={startHere} disabled={!url || cannotPlay}>Start here</button>
        <button type="button" class="small" onclick={endHere} disabled={!url || cannotPlay}>End here</button>
        {#if crop && url && !cannotPlay}
          <button type="button" class="small" onclick={playOrPause}>{playing ? 'Pause' : 'Play'}</button>
        {/if}
        <label class="check"><input type="checkbox" checked={crop !== null} onchange={(e) => toggleCrop(e.currentTarget.checked)} /> Crop</label>
        <button type="button" class="small" onclick={keepAll} disabled={wholeClip}>Keep it all</button>
        <span class="quiet meta" aria-live="polite">
          {#if wholeClip}
            {total.toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}
          {:else}
            {#if start > 0 || end < total}Keeping {start.toFixed(1)} s to {end.toFixed(1)} s ({kept.toFixed(1)} s of {total.toFixed(1)} s){:else}Keeping all {total.toFixed(1)} s{/if}{#if crop}, cropped to {crop.w}×{crop.h}{/if}
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
  .stage { position: relative; min-height: 0; }
  video { width: 100%; height: 100%; min-height: 0; object-fit: contain; background: #000; border-radius: var(--radius-sm); display: block; }
  .check { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; }
  .note, .problem { margin: 0; padding: 12px; font-size: 13px; }
  .problem { color: var(--err); }
  .meta { margin: 0; font-size: 12px; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .small { padding: 2px 8px; font-size: 12px; }
</style>
