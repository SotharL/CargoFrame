<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { getApi } from '../../lib/ipc';
  import type { ClipEdit, VideoClipInfo } from '../../lib/types';
  import { formatSize } from '../values';
  import TrimStrip from './TrimStrip.svelte';
  import CropBox from './CropBox.svelte';
  import MarkLayer from './MarkLayer.svelte';
  import PlayerControls from './PlayerControls.svelte';
  import { containRect } from './contain';
  import { drawMark } from './drawMark';
  import { stepRate } from './player';

  /**
   * `edit` is what the person chose: the part to keep and perhaps an area to crop to. It is
   * `null` while nothing is changed, which is what the share sends along as "send it as it is".
   */
  let {
    info,
    edit = $bindable(null),
    onshot,
  }: {
    info: VideoClipInfo;
    edit?: ClipEdit | null;
    /** Given when the layout has a screenshot block: the button takes the frame on screen for it. */
    onshot?: (frame: Blob) => void;
  } = $props();

  type Box = { x: number; y: number; w: number; h: number };

  const total = $derived(info.duration_ms / 1000);
  let url = $state<string | null>(null);
  let problem = $state('');
  let cannotPlay = $state(false);
  let player = $state<HTMLVideoElement | null>(null);
  let stage = $state<HTMLElement | null>(null);
  let space = $state({ w: 0, h: 0 });
  let now = $state(0);
  let start = $state(0);
  // The block is made again for each capture, so the recording's length is read once.
  let end = $state(untrack(() => info.duration_ms / 1000));

  let playing = $state(false);
  let rate = $state(1);
  let volume = $state(1);
  let muted = $state(false);
  let fullscreen = $state(false);
  let hot = $state(false);
  let shotProblem = $state('');

  /**
   * One tool has the mouse at a time. Crop shows the whole picture with its box; the other
   * tool, and playing, work on the picture as it will be sent (so on the cropped one).
   */
  let mode = $state<'none' | 'crop' | 'mark'>('none');
  /** The area to keep, in the recording's own pixels. It is live: what the box shows is what is sent. */
  let crop = $state<Box | null>(null);
  let cropBefore: Box | null = null;
  let markOn = $state(false);
  let markAt = $state({ x: 0, y: 0 });
  let markText = $state('');

  const kept = $derived(Math.max(0, end - start));
  const wholeClip = $derived(start <= 0 && end >= total && crop === null && !markOn);

  $effect(() => {
    edit = wholeClip
      ? null
      : {
          start_ms: Math.round(start * 1000),
          end_ms: end >= total ? null : Math.round(end * 1000),
          crop: crop ? { ...crop } : null,
          mark: markOn,
        };
  });

  // What is on screen: the whole picture while the crop box is being set, else the part that
  // will be sent. The picture is laid out so that part fills the space, and the rest is clipped.
  const visible = $derived<Box>(mode === 'crop' || !crop ? { x: 0, y: 0, w: info.width, h: info.height } : crop);
  const shown = $derived(containRect(space, visible.w, visible.h));
  const k = $derived(shown.w > 0 ? shown.w / visible.w : 0);

  const even = (n: number) => Math.round(n / 2) * 2;
  function startCrop() {
    cropBefore = crop ? { ...crop } : null;
    if (!crop) {
      // a box inside the picture, so there is something to see and to drag
      const w = even(info.width * 0.8);
      const h = even(info.height * 0.8);
      crop = { x: even((info.width - w) / 2), y: even((info.height - h) / 2), w, h };
    }
    mode = 'crop';
  }
  const applyCrop = () => (mode = 'none');
  function cancelCrop() {
    crop = cropBefore;
    mode = 'none';
  }
  /** Back to the whole capture. */
  const resetCrop = () => {
    crop = null;
    if (mode === 'crop') mode = 'none';
  };
  function startMark() {
    // starts in the middle of the visible picture, so there is something to see and to move
    if (!markOn) {
      markOn = true;
      const v = crop ?? { x: 0, y: 0, w: info.width, h: info.height };
      markAt = { x: Math.round(v.x + v.w / 2), y: Math.round(v.y + v.h / 2) };
    }
    mode = 'mark';
  }
  const doneMark = () => (mode = 'none');
  function removeMark() {
    markOn = false;
    markText = '';
    mode = 'none';
  }
  /** The picture of the mark the share uploads, or `null` while there is no mark. */
  export async function exportMark(): Promise<Blob | null> {
    return markOn ? drawMark(info.width, info.height, markAt, markText) : null;
  }

  // The player's own controls are not used, so nothing else changes these.
  $effect(() => {
    if (!player) return;
    player.playbackRate = rate;
    player.volume = volume;
    player.muted = muted;
  });

  // The picture is laid out from the size of its space, which exists once the recording does.
  $effect(() => {
    const el = stage;
    if (!el) return;
    const measure = () => (space = { w: el.clientWidth, h: el.clientHeight });
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(measure);
    watch.observe(el);
    return () => watch.disconnect();
  });

  onMount(() => {
    getApi()
      .then((api) => api.getSessionClip())
      .then((blob) => (url = URL.createObjectURL(blob)))
      .catch((e) => (problem = String(e)));
    const changed = () => (fullscreen = document.fullscreenElement === stage);
    document.addEventListener('fullscreenchange', changed);
    return () => document.removeEventListener('fullscreenchange', changed);
  });
  onDestroy(() => {
    if (url) URL.revokeObjectURL(url);
    if (typeof document !== 'undefined' && document.fullscreenElement === stage) void document.exitFullscreen?.();
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
  function playOrPause() {
    if (!player) return;
    if (player.paused) void player.play();
    else player.pause();
  }
  function stop() {
    player?.pause();
    seek(start);
  }
  function toggleMute() {
    muted = !muted;
    // coming back from silence at volume 0 would stay silent
    if (!muted && volume === 0) volume = 0.5;
  }
  function setVolume(v: number) {
    volume = v;
    muted = v === 0;
  }
  function toggleFullscreen() {
    if (!stage) return;
    if (document.fullscreenElement === stage) void document.exitFullscreen?.();
    else void stage.requestFullscreen?.()?.catch?.(() => {});
  }
  function onStageKey(e: KeyboardEvent) {
    // only when the picture itself has focus: the handles and boxes use the arrow keys
    if (e.target !== e.currentTarget || e.ctrlKey || e.metaKey || e.altKey) return;
    const key = e.key.toLowerCase();
    if (key === ' ' || key === 'k') playOrPause();
    else if (key === 'f') toggleFullscreen();
    else if (key === 'm' && info.audio) toggleMute();
    else if (key === 'arrowleft') seek(Math.max(start, now - 1));
    else if (key === 'arrowright') seek(Math.min(end, now + 1));
    else return;
    e.preventDefault();
  }

  /** The frame on screen, whole (not cropped), at the recording's own size. */
  async function takeShot() {
    if (!player || !onshot) return;
    shotProblem = '';
    try {
      const canvas = document.createElement('canvas');
      canvas.width = info.width;
      canvas.height = info.height;
      canvas.getContext('2d')!.drawImage(player, 0, 0, info.width, info.height);
      const frame = await new Promise<Blob | null>((done) => canvas.toBlob(done, 'image/png'));
      if (!frame) throw new Error('empty');
      onshot(frame);
    } catch {
      shotProblem = 'This window could not take a picture from the recording.';
    }
  }

  function resetEdits() {
    start = 0;
    end = total;
    crop = null;
    markOn = false;
    markText = '';
    mode = 'none';
  }
</script>

<div class="clip">
  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {:else if cannotPlay}
    <p class="quiet note" role="status">This window cannot play the recording, but it is there and will be sent.</p>
  {:else if url}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="stage"
      class:full={fullscreen}
      bind:this={stage}
      tabindex="0"
      role="group"
      aria-label="Recording player"
      onkeydown={onStageKey}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="picture"
        class:hot
        style:left="{shown.left}px"
        style:top="{shown.top}px"
        style:width="{shown.w}px"
        style:height="{shown.h}px"
        onmouseenter={() => (hot = true)}
        onmouseleave={() => (hot = false)}
      >
        <div
          class="frame"
          style:left="{-visible.x * k}px"
          style:top="{-visible.y * k}px"
          style:width="{info.width * k}px"
          style:height="{info.height * k}px"
        >
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            bind:this={player}
            src={url}
            aria-label="Recording"
            onclick={playOrPause}
            ondblclick={toggleFullscreen}
            ontimeupdate={tick}
            onplay={() => { playing = true; played(); }}
            onpause={() => (playing = false)}
            onerror={() => (cannotPlay = true)}
          ></video>
          {#if markOn}<MarkLayer width={info.width} height={info.height} bind:at={markAt} text={markText} active={mode === 'mark'} />{/if}
          {#if mode === 'crop' && crop}<CropBox width={info.width} height={info.height} bind:crop />{/if}
        </div>
        {#if mode !== 'crop'}
          <PlayerControls
            {playing}
            {rate}
            {muted}
            {volume}
            {now}
            {total}
            {fullscreen}
            audio={info.audio}
            onplaypause={playOrPause}
            onstop={stop}
            onslower={() => (rate = stepRate(rate, -1))}
            onfaster={() => (rate = stepRate(rate, 1))}
            onmute={toggleMute}
            onvolume={setVolume}
            onfullscreen={toggleFullscreen}
          />
        {/if}
      </div>
    </div>
  {:else}
    <p class="quiet note">Loading the recording…</p>
  {/if}
  {#if !problem}
    <div class="trim">
      <TrimStrip duration={total} bind:start bind:end {now} onseek={seek} />
      <div class="row">
        <button type="button" class="small" aria-pressed={mode === 'crop'} onclick={() => (mode === 'crop' ? applyCrop() : startCrop())}>
          {crop && mode !== 'crop' ? 'Edit crop' : 'Crop'}
        </button>
        <button type="button" class="small" aria-pressed={mode === 'mark'} onclick={() => (mode === 'mark' ? doneMark() : startMark())}>
          {markOn && mode !== 'mark' ? 'Edit mark' : 'Mark'}
        </button>
        {#if mode === 'crop'}
          <span class="quiet hint">Drag the box or its corner</span>
          <button type="button" class="small primary" onclick={applyCrop}>Apply crop</button>
          <button type="button" class="small" onclick={cancelCrop}>Cancel</button>
        {:else if mode === 'mark'}
          <input class="marktext" type="text" maxlength="80" placeholder="Text under the mark" aria-label="Text under the mark" bind:value={markText} />
          <span class="quiet hint">Click the picture to move the ring</span>
          <button type="button" class="small primary" onclick={doneMark}>Done</button>
          <button type="button" class="small" onclick={removeMark}>Remove mark</button>
        {:else}
          {#if crop}<button type="button" class="small" onclick={resetCrop}>Reset to full capture</button>{/if}
          {#if markOn}<button type="button" class="small" onclick={removeMark}>Remove mark</button>{/if}
          {#if markOn && markText}<span class="quiet hint selectable">“{markText}”</span>{/if}
        {/if}
        {#if onshot}
          <button type="button" class="small" onclick={takeShot} disabled={!url || cannotPlay}>Take screenshot</button>
        {/if}
        <button type="button" class="small" onclick={resetEdits} disabled={wholeClip}>Reset edits</button>
        {#if shotProblem}<span class="error" role="alert">{shotProblem}</span>{/if}
        <span class="quiet meta" aria-live="polite">
          {#if wholeClip}
            {total.toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}
          {:else}
            {#if start > 0 || end < total}Keeping {start.toFixed(1)} s to {end.toFixed(1)} s ({kept.toFixed(1)} s of {total.toFixed(1)} s){:else}Keeping all {total.toFixed(1)} s{/if}{#if crop}, cropped to {crop.w}×{crop.h}{/if}{#if markOn}, marked{/if}
          {/if}
        </span>
      </div>
    </div>
  {:else}
    <p class="quiet meta">{total.toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}</p>
  {/if}
</div>

<style>
  .clip { height: 100%; min-height: 0; display: grid; grid-template-rows: 1fr auto; gap: 2px; }
  .stage { position: relative; min-height: 0; background: #000; border-radius: var(--radius-sm); overflow: hidden; outline-offset: 2px; }
  .stage.full { border-radius: 0; }
  .picture { position: absolute; overflow: hidden; }
  .frame { position: absolute; }
  video { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: fill; display: block; background: #000; cursor: pointer; }
  .note, .problem { margin: 0; padding: 12px; font-size: 13px; }
  .problem { color: var(--err); }
  .meta { margin: 0; font-size: 12px; }
  .hint { font-size: 12px; }
  .error { color: var(--err); font-size: 12px; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .small { padding: 2px 8px; font-size: 12px; }
  .small[aria-pressed='true'] { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .marktext { font-size: 12px; padding: 2px 6px; width: 170px; }
</style>
