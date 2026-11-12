<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { getApi } from '../../lib/ipc';
  import type { ClipEdit, VideoClipInfo } from '../../lib/types';
  import { formatSize } from '../values';
  import TrimStrip from './TrimStrip.svelte';
  import CropBox from './CropBox.svelte';
  import MarkCanvas, { type MarkTool } from './MarkCanvas.svelte';
  import PlayerControls from './PlayerControls.svelte';
  import StylePicker from '../editor/StylePicker.svelte';
  import { COLORS, THICKNESS, clearAll, emptyState, redo, undo, type EditorState, type TextStyle, type Thickness } from '../editor/model';
  import { containRect } from './contain';
  import { drawMarks } from './drawMark';
  import { stepRate } from './player';
  import { KEY_SIZE, KEY_START, drawKeys, lineAt, placeAt } from './keyLayer';

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
  // a continuous recording of the whole screen starts on the window that was in focus
  let crop = $state<Box | null>(untrack(() => (info.crop ? { ...info.crop } : null)));
  let cropBefore: Box | null = null;
  /** The marks drawn over the recording, and the tool, colour and size they are drawn with. */
  let marks = $state<EditorState>(emptyState());
  let markTool = $state<MarkTool>('ellipse');
  let markColor = $state(COLORS[0]!);
  let markThickness = $state<Thickness>('medium');
  let markText = $state<TextStyle>({ bold: true, italic: false, box: false });
  let markCanvas = $state<ReturnType<typeof MarkCanvas>>();
  const markOn = $derived(marks.annotations.length > 0);

  /** The crop follows the recorded window as it was moved (the places the capture noted). */
  // the block is made again for each capture, so these are read once
  const canFollow = untrack(() => (info.follow?.length ?? 0) > 1 && !!info.crop);
  let followOn = $state(canFollow);
  /** The keys pressed show over the clip, where the person put them (parts of the picture). */
  const hasKeys = untrack(() => (info.keys?.length ?? 0) > 0);
  let keysOn = $state(hasKeys);
  let keysAt = $state({ ...KEY_START });
  const showKeys = $derived(keysOn && hasKeys);

  const kept = $derived(Math.max(0, end - start));
  const wholeClip = $derived(start <= 0 && end >= total && crop === null && !markOn && !showKeys);
  /** Nothing changed from how the window opened: Reset edits has nothing to do. */
  const pristine = $derived(
    start <= 0 &&
      end >= total &&
      !markOn &&
      followOn === canFollow &&
      keysOn === hasKeys &&
      keysAt.fx === KEY_START.fx &&
      keysAt.fy === KEY_START.fy &&
      JSON.stringify(crop) === JSON.stringify(info.crop ?? null),
  );

  /** The size of the picture as it is sent: the crop, or the whole recording (made even). */
  const sent = $derived(crop ? { w: crop.w, h: crop.h } : { w: info.width & ~1, h: info.height & ~1 });

  $effect(() => {
    edit = wholeClip
      ? null
      : {
          start_ms: Math.round(start * 1000),
          end_ms: end >= total ? null : Math.round(end * 1000),
          crop: crop ? { ...crop } : null,
          mark: markOn,
          // said only when on, so a plain edit stays as it was
          ...(followOn && crop ? { follow: true } : {}),
          ...(showKeys ? { keys: { x: Math.round(keysAt.fx * sent.w), y: Math.round(keysAt.fy * sent.h) } } : {}),
        };
  });

  /** Where the crop is at this moment: it moves with the window when it follows it. */
  const liveCrop = $derived<Box | null>(crop && followOn && info.follow ? { ...crop, ...placeAt(info.follow, now * 1000) } : crop);
  /** The line of keys showing now. */
  const keyLine = $derived(showKeys ? lineAt(info.keys, now * 1000) : null);

  // What is on screen: the whole picture while the crop box is being set, else the part that
  // will be sent. The picture is laid out so that part fills the space, and the rest is clipped.
  const visible = $derived<Box>(mode === 'crop' || !liveCrop ? { x: 0, y: 0, w: info.width, h: info.height } : liveCrop);
  const shown = $derived(containRect(space, visible.w, visible.h));
  const k = $derived(shown.w > 0 ? shown.w / visible.w : 0);

  const even = (n: number) => Math.round(n / 2) * 2;
  /** Crop as on a screenshot: drag over the picture to draw the part to keep, then apply. */
  function startCrop() {
    markCanvas?.commitText();
    // a crop set by hand stays where it is put: it starts from where the window is now
    if (followOn && liveCrop) crop = { ...liveCrop };
    followOn = false;
    cropBefore = crop ? { ...crop } : null;
    mode = 'crop';
  }
  /** Follow the window again, from the crop the capture started with. */
  function toggleFollow() {
    if (followOn) {
      if (liveCrop) crop = { ...liveCrop };
      followOn = false;
    } else if (info.crop) {
      crop = { ...info.crop };
      followOn = true;
    }
  }
  /** For the keyboard, or to start from something: a box in the middle, a little smaller than the picture. */
  function middleBox() {
    const w = even(info.width * 0.8);
    const h = even(info.height * 0.8);
    crop = { x: even((info.width - w) / 2), y: even((info.height - h) / 2), w, h };
  }
  const applyCrop = () => (mode = 'none');
  function cancelCrop() {
    crop = cropBefore;
    mode = 'none';
  }
  /** Back to the whole capture. */
  const resetCrop = () => {
    crop = null;
    followOn = false;
    if (mode === 'crop') mode = 'none';
  };
  const startMark = () => (mode = 'mark');
  function doneMark() {
    markCanvas?.commitText();
    mode = 'none';
  }
  function removeMarks() {
    marks = clearAll();
    if (mode === 'mark') mode = 'none';
  }
  /** The pictures of the keys the share uploads (a blank one, then one per line), or `null`. */
  export async function exportKeys(): Promise<Blob[] | null> {
    return showKeys && info.keys ? drawKeys(info.keys, sent.h) : null;
  }

  // Dragging the keys to where they show; they stay inside the picture.
  let keysBox = $state<HTMLElement | null>(null);
  let dragFrom: { x: number; y: number; fx: number; fy: number } | null = null;
  function keysDown(e: PointerEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    dragFrom = { x: e.clientX, y: e.clientY, ...keysAt };
  }
  function keysMove(e: PointerEvent) {
    if (!dragFrom || shown.w <= 0) return;
    const box = keysBox?.getBoundingClientRect();
    const maxX = Math.max(0, 1 - (box?.width ?? 0) / shown.w);
    const maxY = Math.max(0, 1 - (box?.height ?? 0) / shown.h);
    keysAt = {
      fx: Math.min(maxX, Math.max(0, dragFrom.fx + (e.clientX - dragFrom.x) / shown.w)),
      fy: Math.min(maxY, Math.max(0, dragFrom.fy + (e.clientY - dragFrom.y) / shown.h)),
    };
  }
  const keysUp = () => (dragFrom = null);
  function keysKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.05 : 0.01;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const m = moves[e.key];
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      keysOn = false;
    } else if (m) {
      e.preventDefault();
      e.stopPropagation();
      keysAt = { fx: Math.min(0.97, Math.max(0, keysAt.fx + m[0])), fy: Math.min(0.95, Math.max(0, keysAt.fy + m[1])) };
    }
  }

  /** The picture of the marks the share uploads, or `null` while there are none. */
  export async function exportMark(): Promise<Blob | null> {
    markCanvas?.commitText();
    return marks.annotations.length > 0 ? drawMarks(info.width, info.height, marks.annotations) : null;
  }
  const MARK_TOOLS: { tool: MarkTool; label: string; path: string }[] = [
    { tool: 'pen', label: 'Draw', path: 'M4 20l1-4L17 4l3 3L8 19z' },
    { tool: 'rect', label: 'Square', path: 'M4 5h16v14H4z' },
    { tool: 'ellipse', label: 'Circle', path: 'M12 5a8 7 0 100 14a8 7 0 100-14' },
    { tool: 'arrow', label: 'Arrow', path: 'M5 19L19 5M10 5h9v9' },
    { tool: 'text', label: 'Text', path: 'M5 6V4h14v2M12 4v16M9 20h6' },
  ];

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
  // while it plays, the keys and a following crop keep up with every picture
  $effect(() => {
    if (!playing || !player || typeof requestAnimationFrame === 'undefined') return;
    let id = requestAnimationFrame(function step() {
      tick();
      id = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(id);
  });
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
    // while cropping, Enter applies and Escape puts the crop back as it was
    if (mode === 'crop' && (e.key === 'Enter' || e.key === 'Escape')) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Enter' && crop) applyCrop();
      else if (e.key === 'Escape') cancelCrop();
      return;
    }
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

  /** Back to how the window opened: all of it, the crop it started with, the keys where they were. */
  function resetEdits() {
    start = 0;
    end = total;
    crop = info.crop ? { ...info.crop } : null;
    followOn = canFollow;
    keysOn = hasKeys;
    keysAt = { ...KEY_START };
    marks = clearAll();
    mode = 'none';
  }
</script>

<div class="clip" data-content>
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
          {#if markOn || mode === 'mark'}
            <MarkCanvas
              bind:this={markCanvas}
              width={info.width}
              height={info.height}
              bind:doc={marks}
              tool={markTool}
              bind:color={markColor}
              thickness={markThickness}
              bind:textStyle={markText}
              active={mode === 'mark'}
            />
          {/if}
          {#if mode === 'crop'}<CropBox width={info.width} height={info.height} bind:crop />{/if}
        </div>
        {#if showKeys && mode !== 'crop'}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <div
            class="keys"
            class:empty={!keyLine}
            class:hot
            bind:this={keysBox}
            style:left="{keysAt.fx * shown.w}px"
            style:top="{keysAt.fy * shown.h}px"
            style:font-size="{Math.max(9, shown.h * KEY_SIZE)}px"
            role="group"
            tabindex="0"
            aria-label="Keys pressed: drag, or use the arrow keys, to move them; Delete takes them off"
            title="Drag to move the keys"
            onpointerdown={keysDown}
            onpointermove={keysMove}
            onpointerup={keysUp}
            onpointercancel={keysUp}
            onkeydown={keysKey}
          >
            <span class="line">{keyLine?.text ?? 'Keys'}</span>
            <button type="button" class="off" aria-label="Take the keys off the recording" title="Take the keys off" onclick={() => (keysOn = false)}>×</button>
          </div>
        {/if}
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
          {markOn && mode !== 'mark' ? 'Edit marks' : 'Mark'}
        </button>
        {#if hasKeys && mode === 'none'}
          <button type="button" class="small" aria-pressed={keysOn} title="The keys pressed while it recorded" onclick={() => (keysOn = !keysOn)}>Keys</button>
        {/if}
        {#if canFollow && mode === 'none'}
          <button type="button" class="small" aria-pressed={followOn} title="The crop moves with the window as it was moved" onclick={toggleFollow}>Follow window</button>
        {/if}
        {#if mode === 'crop'}
          <span class="quiet hint">{crop ? 'Drag the box or its corner, or draw a new one' : 'Drag over the picture to choose the part to keep'}</span>
          {#if !crop}<button type="button" class="small" onclick={middleBox}>Start from the middle</button>{/if}
          <button type="button" class="small primary" onclick={applyCrop} disabled={!crop}>Apply crop</button>
          <button type="button" class="small" onclick={cancelCrop}>Cancel</button>
        {:else if mode === 'mark'}
          <span class="tools" role="toolbar" aria-label="Marks on the recording" data-mark-tools>
            {#each MARK_TOOLS as t (t.tool)}
              <button type="button" class="tool" aria-label={t.label} title={t.label} aria-pressed={markTool === t.tool} onmousedown={(e) => e.preventDefault()} onclick={() => { markCanvas?.commitText(); markTool = t.tool; }}>
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d={t.path} /></svg>
              </button>
            {/each}
            {#if markTool === 'text'}
              <button type="button" class="tool letter" aria-label="Bold" title="Bold" aria-pressed={!!markText.bold} onmousedown={(e) => e.preventDefault()} onclick={() => (markText.bold = !markText.bold)}><b>B</b></button>
              <button type="button" class="tool letter" aria-label="Italic" title="Italic" aria-pressed={!!markText.italic} onmousedown={(e) => e.preventDefault()} onclick={() => (markText.italic = !markText.italic)}><i>I</i></button>
              <button type="button" class="tool letter" aria-label="On a box" title="On a box: readable on any picture" aria-pressed={!!markText.box} onmousedown={(e) => e.preventDefault()} onclick={() => (markText.box = !markText.box)}>▭</button>
            {/if}
            <StylePicker colors={COLORS} thicknesses={THICKNESS} bind:color={markColor} bind:thickness={markThickness} />
            <button type="button" class="tool" aria-label="Undo" title="Undo" disabled={!markOn} onclick={() => (marks = undo(marks))}>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 010 11H11" /></svg>
            </button>
            <button type="button" class="tool" aria-label="Redo" title="Redo" disabled={marks.redo.length === 0} onclick={() => (marks = redo(marks))}>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 000 11H13" /></svg>
            </button>
          </span>
          <span class="quiet hint">{markTool === 'text' ? 'Click to write, click a text to change it' : 'Drag on the picture'}</span>
          <button type="button" class="small primary" onclick={doneMark}>Done</button>
          {#if markOn}<button type="button" class="small" onclick={removeMarks}>Remove marks</button>{/if}
        {:else}
          {#if crop}<button type="button" class="small" onclick={resetCrop}>Reset to full capture</button>{/if}
          {#if markOn}<button type="button" class="small" onclick={removeMarks}>Remove marks</button>{/if}
        {/if}
        {#if onshot}
          <button type="button" class="small" onclick={takeShot} disabled={!url || cannotPlay}>Take screenshot</button>
        {/if}
        <button type="button" class="small" onclick={resetEdits} disabled={pristine}>Reset edits</button>
        {#if shotProblem}<span class="error" role="alert">{shotProblem}</span>{/if}
        <span class="quiet meta" aria-live="polite">
          {#if wholeClip}
            {total.toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}
          {:else}
            {#if start > 0 || end < total}Keeping {start.toFixed(1)} s to {end.toFixed(1)} s ({kept.toFixed(1)} s of {total.toFixed(1)} s){:else}Keeping all {total.toFixed(1)} s{/if}{#if crop}, cropped to {crop.w}×{crop.h}{#if followOn}{' following the window'}{/if}{/if}{#if markOn}, marked{/if}{#if showKeys}, with the keys{/if}
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
  .tools { display: inline-flex; flex-wrap: wrap; gap: 2px; align-items: center; }
  .tool { width: 26px; height: 26px; padding: 0; display: inline-grid; place-items: center; font-size: 13px; }
  .tool svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .tool[aria-pressed='true'] { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .tool.letter { font-family: Georgia, serif; }
  /* black letters with a white outline, as the sent clip has them */
  .keys {
    position: absolute; display: inline-flex; align-items: center; gap: 0.35em; padding: 0.1em 0.25em; line-height: 1.4;
    font-weight: 700; font-family: system-ui, 'Segoe UI', 'DejaVu Sans', sans-serif; color: #000; white-space: pre;
    cursor: grab; touch-action: none; user-select: none; border-radius: 4px; border: 1px dashed transparent;
    text-shadow: -0.08em -0.08em 0 #fff, 0.08em -0.08em 0 #fff, -0.08em 0.08em 0 #fff, 0.08em 0.08em 0 #fff,
      0 -0.11em 0 #fff, 0 0.11em 0 #fff, -0.11em 0 0 #fff, 0.11em 0 0 #fff;
  }
  .keys:active { cursor: grabbing; }
  .keys.hot, .keys:focus-visible { border-color: rgb(255 255 255 / 0.7); }
  /* where they go, while no key shows: only when the mouse is over the picture */
  .keys.empty .line { opacity: 0; }
  .keys.empty.hot .line, .keys.empty:focus-visible .line { opacity: 0.45; }
  .keys .off {
    font: 600 12px/1 system-ui, sans-serif; padding: 1px 4px; border-radius: 999px; text-shadow: none; opacity: 0;
    background: rgb(0 0 0 / 0.6); color: #fff; border: 0; cursor: pointer;
  }
  .keys.hot .off, .keys:focus-within .off { opacity: 1; }
</style>
