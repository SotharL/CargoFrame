<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { getApi } from '../../lib/ipc';
  import type {
    Video, VideoCapture, VideoEncoder, VideoRecording, VideoTestEdit, VideoTools, VideoTrial,
  } from '../../lib/types';

  const CAPTURES: { id: VideoCapture; label: string }[] = [
    { id: 'auto', label: 'Automatic' },
    { id: 'gdigrab', label: 'GDI (Windows, works almost everywhere)' },
    { id: 'ddagrab', label: 'Desktop Duplication (Windows, lighter on the processor)' },
    { id: 'x11', label: 'X11 (Linux)' },
  ];
  const ENCODERS: { id: VideoEncoder; label: string }[] = [
    { id: 'auto', label: 'Automatic' },
    { id: 'software', label: 'Software (x264, works everywhere)' },
    { id: 'nvenc', label: 'NVIDIA graphics card (NVENC)' },
    { id: 'qsv', label: 'Intel graphics (Quick Sync)' },
    { id: 'amf', label: 'AMD graphics (AMF)' },
    { id: 'media_foundation', label: 'Windows built-in' },
    { id: 'vaapi', label: 'VAAPI (Linux)' },
  ];

  let loaded = $state(false);
  let video = $state<Video>({ ffmpeg: null, capture: 'auto', encoder: 'auto', fps: 30, window_secs: 10, max_height: 1080 });
  let ffmpegText = $state('');
  let status = $state<{ ok: boolean; text: string } | null>(null);
  let tools = $state<VideoTools | null>(null);
  let testing = $state(false);
  let trying = $state(false);
  let trial = $state<VideoTrial | null>(null);
  let openError = $state('');
  let recording = $state<VideoRecording>({ state: 'idle' });
  let stopping = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;
  // trim, crop and mark a copy of the test clip
  let trimStart = $state(0);
  let trimEnd = $state(0);
  let cropOn = $state(false);
  let crop = $state({ x: 0, y: 0, w: 0, h: 0 });
  let markOn = $state(false);
  let exporting = $state(false);
  let copy = $state<VideoTrial | null>(null);
  let playing = $state<'test' | 'exported'>('test');
  let playerUrl = $state<string | null>(null);
  let playerError = $state('');
  let loadingPlayer = $state(false);
  const running = $derived(recording.state === 'recording' || recording.state === 'finishing' || stopping);

  function name(list: { id: string; label: string }[], id: string | null): string {
    const full = list.find((x) => x.id === id)?.label ?? String(id);
    return full.replace(/ \(.*\)$/, '');
  }

  /** Whole numbers only; anything else is told to the person rather than guessed at. */
  function whole(label: string, v: unknown): number | null {
    const n = Number(v);
    if (v === null || v === '' || !Number.isFinite(n)) {
      status = { ok: false, text: `${label} must be a whole number` };
      return null;
    }
    return Math.round(n);
  }

  function current(): Video | null {
    const fps = whole('Frames per second', video.fps);
    const window_secs = whole('Length kept', video.window_secs);
    const max_height = whole('Largest height', video.max_height);
    if (fps === null || window_secs === null || max_height === null) return null;
    return { ...video, ffmpeg: ffmpegText.trim() || null, fps, window_secs, max_height };
  }

  async function save(): Promise<boolean> {
    const v = current();
    if (!v) return false;
    try {
      await (await getApi()).setVideo(v);
      video = v;
      status = { ok: true, text: 'Saved' };
      return true;
    } catch (e) {
      status = { ok: false, text: String(e) };
      return false;
    }
  }

  async function detect() {
    const v = current();
    if (!v) return;
    testing = true;
    try {
      tools = await (await getApi()).detectVideoTools(v);
    } catch (e) {
      tools = null;
      status = { ok: false, text: String(e) };
    } finally {
      testing = false;
    }
  }

  async function savePlace() {
    if (await save()) await detect();
  }

  async function choose() {
    const p = await (await getApi()).pickFolder('Choose the folder that holds ffmpeg');
    if (p) {
      ffmpegText = p;
      await savePlace();
    }
  }

  async function tryRecording() {
    const v = current();
    if (!v) return;
    trying = true;
    trial = null;
    openError = '';
    closePlayer();
    playerError = '';
    try {
      setTrial(await (await getApi()).tryVideoRecording(v));
    } catch (e) {
      trial = {
        ok: false, message: String(e), capture: null, encoder: null, width: 0, height: 0,
        clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
      };
    } finally {
      trying = false;
    }
  }

  function closePlayer() {
    if (playerUrl) URL.revokeObjectURL(playerUrl);
    playerUrl = null;
  }

  /** Play the clip in this window. Whether the window can play H.264 is itself worth knowing. */
  async function watch(exported = false) {
    closePlayer();
    playerError = '';
    loadingPlayer = true;
    try {
      playerUrl = URL.createObjectURL(await (await getApi()).getVideoTestClip(exported));
      playing = exported ? 'exported' : 'test';
    } catch (e) {
      playerError = String(e);
    } finally {
      loadingPlayer = false;
    }
  }

  function playerFailed(e: Event) {
    const err = (e.currentTarget as HTMLVideoElement).error;
    const why = err?.message ? ` (${err.message})` : err ? ` (error ${err.code})` : '';
    playerError = `This window cannot play the clip${why}. The clip itself is fine: open it in your video player instead.`;
    closePlayer();
  }

  /** A new test clip starts a new edit: the whole clip, no crop, and a crop box ready in the middle. */
  function setTrial(t: VideoTrial) {
    trial = t;
    copy = null;
    if (!t.ok) return;
    trimStart = 0;
    trimEnd = Math.round(t.clip_ms / 100) / 10;
    const even = (n: number) => Math.floor(n / 2) * 2;
    crop = { x: even(t.width / 4), y: even(t.height / 4), w: even(t.width / 2), h: even(t.height / 2) };
    cropOn = false;
    markOn = false;
  }

  async function exportCopy() {
    const v = current();
    if (!v) return;
    const nums = [trimStart, trimEnd, crop.x, crop.y, crop.w, crop.h].map(Number);
    if (nums.some((n) => !Number.isFinite(n))) {
      copy = failure('The start, the end and the crop must be numbers');
      return;
    }
    const edit: VideoTestEdit = {
      start_ms: Math.max(0, Math.round(Number(trimStart) * 1000)),
      end_ms: Math.round(Number(trimEnd) * 1000),
      crop: cropOn ? { x: Math.round(crop.x), y: Math.round(crop.y), w: Math.round(crop.w), h: Math.round(crop.h) } : null,
      mark: markOn,
    };
    exporting = true;
    copy = null;
    closePlayer();
    playerError = '';
    try {
      copy = await (await getApi()).exportVideoTest(v, edit);
    } catch (e) {
      copy = failure(String(e));
    } finally {
      exporting = false;
    }
  }

  function failure(message: string): VideoTrial {
    return {
      ok: false, message, capture: null, encoder: null, width: 0, height: 0,
      clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
    };
  }

  function stopPolling() {
    if (timer) clearInterval(timer);
    timer = undefined;
  }

  async function refresh() {
    try {
      recording = await (await getApi()).getVideoRecording();
    } catch {
      return;
    }
    if (recording.state === 'failed') {
      // stopping hands over the reason and clears it
      await stopRecording();
    } else if (recording.state === 'idle') {
      stopPolling();
    }
  }

  async function startRecording() {
    const v = current();
    if (!v) return;
    trial = null;
    openError = '';
    closePlayer();
    playerError = '';
    try {
      await (await getApi()).startVideoRecording(v);
    } catch (e) {
      trial = failure(String(e));
      return;
    }
    await refresh();
    stopPolling();
    timer = setInterval(refresh, 500);
  }

  async function stopRecording() {
    stopPolling();
    stopping = true;
    try {
      setTrial(await (await getApi()).stopVideoRecording());
    } catch (e) {
      trial = failure(String(e));
    } finally {
      stopping = false;
      recording = { state: 'idle' };
    }
  }

  function seconds(ms: number): string {
    return (ms / 1000).toFixed(0);
  }

  async function openClip(exported = false) {
    openError = '';
    try {
      await (await getApi()).openVideoTest(exported);
    } catch (e) {
      openError = String(e);
    }
  }

  function listed(list: string[]): string {
    return list.length ? list.join(', ') : 'none';
  }

  /** Greyed out when the ffmpeg that was found does not have it. */
  function missing(kind: 'capture' | 'encoder', id: string): boolean {
    if (!tools?.found || id === 'auto') return false;
    return kind === 'capture'
      ? !tools.captures.includes(id as VideoCapture)
      : !tools.encoders.includes(id as VideoEncoder);
  }

  function mb(bytes: number): string {
    return (bytes / (1024 * 1024)).toFixed(1);
  }

  onMount(async () => {
    video = await (await getApi()).getVideo();
    ffmpegText = video.ffmpeg ?? '';
    loaded = true;
    // a recording started earlier may still be running
    try {
      recording = await (await getApi()).getVideoRecording();
    } catch {
      recording = { state: 'idle' };
    }
    if (recording.state === 'recording') timer = setInterval(refresh, 500);
    else if (recording.state === 'failed') await stopRecording();
    await detect();
  });

  onDestroy(() => {
    stopPolling();
    closePlayer();
  });
</script>

{#if loaded}
  <Section
    title="ffmpeg"
    hint="CargoFrame uses ffmpeg, a free program, to record and to write the video. It is not part of CargoFrame: install it (on Windows: winget install Gyan.FFmpeg) or point to the folder you unpacked it in."
  >
    <Field
      label="Where ffmpeg is"
      hint="A folder that holds ffmpeg and ffprobe, or the ffmpeg program itself. Leave it empty to look next to CargoFrame and then wherever your system looks for programs."
    >
      <div class="row">
        <input type="text" bind:value={ffmpegText} onchange={savePlace} placeholder="Find it for me" />
        <button type="button" onclick={choose}>Choose…</button>
        <button type="button" onclick={detect} disabled={testing}>{testing ? 'Testing…' : 'Test ffmpeg'}</button>
      </div>
    </Field>
    {#if tools}
      {#if tools.found}
        <div class="facts" aria-label="What was found">
          <Chip kind="ok" label="ffmpeg found" />
          <dl>
            <dt>Program</dt><dd><code>{tools.ffmpeg}</code></dd>
            <dt>Version</dt><dd>{tools.version}</dd>
            <dt>Captures</dt><dd>{listed(tools.captures.map((c) => name(CAPTURES, c)))}</dd>
            <dt>Encoders</dt><dd>{listed(tools.encoders.map((e) => name(ENCODERS, e)))}</dd>
            {#if tools.auto_capture && tools.auto_encoder}
              <dt>Automatic uses</dt>
              <dd>{name(CAPTURES, tools.auto_capture)} and {name(ENCODERS, tools.auto_encoder)}</dd>
            {/if}
          </dl>
          <p class="quiet note">
            An encoder in the list is only in this ffmpeg. Whether this PC can use it (a graphics card, a driver) is what the test recording below tells you.
          </p>
        </div>
      {:else}
        <div class="facts">
          <Chip kind="err" label="ffmpeg not found" />
        </div>
      {/if}
      {#if tools.problem}<p class="problem" role="alert">{tools.problem}</p>{/if}
    {/if}
  </Section>

  <Section title="Capture and compression" hint="Automatic is a safe choice. Change these only to try what your PC does best.">
    <div class="two">
      <Field label="Capture" hint="How the screen is read. GDI records all your screens together; Desktop Duplication records the first one Windows lists, usually your main screen.">
        <select bind:value={video.capture} onchange={save}>
          {#each CAPTURES as c (c.id)}
            <option value={c.id} disabled={missing('capture', c.id)}>
              {c.label}{missing('capture', c.id) ? ' (not in this ffmpeg)' : ''}
            </option>
          {/each}
        </select>
      </Field>
      <Field label="Compression" hint="Software uses the processor; a graphics card encoder is lighter if this PC has one.">
        <select bind:value={video.encoder} onchange={save}>
          {#each ENCODERS as e (e.id)}
            <option value={e.id} disabled={missing('encoder', e.id)}>
              {e.label}{missing('encoder', e.id) ? ' (not in this ffmpeg)' : ''}
            </option>
          {/each}
        </select>
      </Field>
    </div>
    <div class="three">
      <Field label="Frames per second" hint="5 to 60. Fewer is lighter.">
        <input type="number" min="5" max="60" bind:value={video.fps} onchange={save} />
      </Field>
      <Field label="Length kept (seconds)" hint="1 to 30. Only the last seconds before you stop are kept.">
        <input type="number" min="1" max="30" bind:value={video.window_secs} onchange={save} />
      </Field>
      <Field label="Largest height (pixels)" hint="Bigger screens are scaled down. 0 keeps the screen's own size.">
        <input type="number" min="0" max="4320" bind:value={video.max_height} onchange={save} />
      </Field>
    </div>
    {#if status}<Chip kind={status.ok ? 'ok' : 'err'} label={status.text} />{/if}
  </Section>

  <Section
    title="Try it on this PC"
    hint="Records the screen with the settings above and keeps the clip so you can watch it. Move something on the screen while it records. Recording from a shortcut is not available yet; this only checks what your PC can do."
  >
    <div class="row wrap">
      <button type="button" class="primary" onclick={tryRecording} disabled={trying || running || (tools !== null && !tools.found)}>
        {trying ? 'Recording…' : 'Try a 5-second recording'}
      </button>
      {#if running}
        <button type="button" onclick={stopRecording} disabled={stopping}>
          {stopping ? 'Making the clip…' : 'Stop and make the clip'}
        </button>
      {:else}
        <button type="button" onclick={startRecording} disabled={trying || (tools !== null && !tools.found)}>
          Record until I stop
        </button>
      {/if}
    </div>
    {#if trying}
      <p class="quiet" role="status">Recording for 5 seconds. Move something on the screen…</p>
    {/if}
    {#if recording.state === 'recording'}
      <div class="facts" role="status" aria-label="Recording">
        <Chip kind="warn" label="Recording" />
        <p>
          {seconds(recording.elapsed_ms)} s so far. The last {seconds(recording.buffered_ms)} s are kept (up to {recording.keep_secs} s),
          as in a real recording.
        </p>
        {#if recording.squeezed}<p class="quiet note">The disk limit cut the start short.</p>{/if}
        <p class="quiet note">
          Open Task Manager now (Performance: CPU, GPU) to see what recording costs on this PC. It stops by itself after 10 minutes, or when you close Settings.
        </p>
      </div>
    {/if}
    {#if trial}
      <div class="facts" aria-label="Test result">
        <Chip kind={trial.ok ? 'ok' : 'err'} label={trial.ok ? 'It works' : 'It did not work'} />
        <p class={trial.ok ? '' : 'problem'} role={trial.ok ? 'status' : 'alert'}>{trial.message}</p>
        {#if trial.ok}
          <dl>
            <dt>Clip</dt><dd>{(trial.clip_ms / 1000).toFixed(1)} s, {trial.width}×{trial.height}, {mb(trial.clip_bytes)} MB</dd>
            <dt>Time taken</dt><dd>{(trial.took_ms / 1000).toFixed(1)} s</dd>
            {#if trial.clip}<dt>File</dt><dd><code>{trial.clip}</code></dd>{/if}
          </dl>
          <div class="row">
            <button type="button" onclick={() => openClip(false)}>Open the clip</button>
            <button type="button" onclick={() => watch(false)} disabled={loadingPlayer}>
              {loadingPlayer ? 'Loading…' : 'Watch it here'}
            </button>
          </div>
          <details class="edit">
            <summary>Trim, crop and mark a copy</summary>
            <div class="editbody">
              <p class="quiet note">
                Makes an edited copy of the test clip the way an edited video will be saved. The test clip itself is not changed.
              </p>
              <div class="two">
                <Field label="Start (seconds)">
                  <input type="number" min="0" step="0.1" bind:value={trimStart} />
                </Field>
                <Field label="End (seconds)">
                  <input type="number" min="0" step="0.1" bind:value={trimEnd} />
                </Field>
              </div>
              <label class="check"><input type="checkbox" bind:checked={cropOn} /> Crop to an area</label>
              {#if cropOn}
                <div class="four">
                  <Field label="Left (px)"><input type="number" min="0" bind:value={crop.x} /></Field>
                  <Field label="Top (px)"><input type="number" min="0" bind:value={crop.y} /></Field>
                  <Field label="Width (px)"><input type="number" min="2" bind:value={crop.w} /></Field>
                  <Field label="Height (px)"><input type="number" min="2" bind:value={crop.h} /></Field>
                </div>
              {/if}
              <label class="check"><input type="checkbox" bind:checked={markOn} /> Lay a red square over the copy (checks that marks are drawn)</label>
              <div class="row">
                <button type="button" onclick={exportCopy} disabled={exporting}>
                  {exporting ? 'Making the copy…' : 'Export a copy'}
                </button>
              </div>
              {#if copy}
                <div class="facts" aria-label="Copy result">
                  <Chip kind={copy.ok ? 'ok' : 'err'} label={copy.ok ? 'Copy made' : 'No copy made'} />
                  <p class={copy.ok ? '' : 'problem'} role={copy.ok ? 'status' : 'alert'}>{copy.message}</p>
                  {#if copy.ok}
                    <dl>
                      <dt>Copy</dt><dd>{(copy.clip_ms / 1000).toFixed(1)} s, {copy.width}×{copy.height}, {mb(copy.clip_bytes)} MB</dd>
                      {#if copy.clip}<dt>File</dt><dd><code>{copy.clip}</code></dd>{/if}
                    </dl>
                    <div class="row">
                      <button type="button" onclick={() => openClip(true)}>Open the copy</button>
                      <button type="button" onclick={() => watch(true)} disabled={loadingPlayer}>Watch the copy here</button>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          </details>
          {#if openError}<p class="problem" role="alert">{openError}</p>{/if}
          {#if playerError}<p class="problem" role="alert">{playerError}</p>{/if}
          {#if playerUrl}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video
              src={playerUrl}
              controls
              muted
              aria-label={playing === 'exported' ? 'Edited copy' : 'Test clip'}
              onerror={playerFailed}
            ></video>
          {/if}
        {/if}
      </div>
    {/if}
  </Section>
{/if}

<style>
  .row { display: flex; gap: 8px; }
  .row.wrap { flex-wrap: wrap; }
  .row input { flex: 1; min-width: 0; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; align-items: start; }
  .three { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; align-items: start; }
  .facts { display: grid; gap: 8px; justify-items: start; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 14px; margin: 0; font-size: 13px; }
  dt { font-weight: 600; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  .note { font-size: 12.5px; margin: 0; }
  .problem { color: var(--err); font-size: 13px; margin: 0; }
  p { margin: 0; }
  input[type='number'] {
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; width: 100%;
  }
  .primary { font-weight: 600; }
  .four { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; align-items: start; }
  .editbody { display: grid; gap: 12px; margin-top: 10px; }
  .check { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  .edit { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px; width: 100%; box-sizing: border-box; }
  .edit summary { cursor: pointer; font-weight: 600; font-size: 13px; }
  video { max-width: 100%; border: 1px solid var(--border); border-radius: var(--radius-sm); background: #000; }
</style>
