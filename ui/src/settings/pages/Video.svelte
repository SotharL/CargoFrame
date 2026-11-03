<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Video, VideoCapture, VideoEncoder, VideoTools, VideoTrial } from '../../lib/types';

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
    try {
      trial = await (await getApi()).tryVideoRecording(v);
    } catch (e) {
      trial = {
        ok: false, message: String(e), capture: null, encoder: null, width: 0, height: 0,
        clip_ms: 0, clip_bytes: 0, took_ms: 0, clip: null,
      };
    } finally {
      trying = false;
    }
  }

  async function openClip() {
    openError = '';
    try {
      await (await getApi()).openVideoTest();
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
    await detect();
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
      <Field label="Capture" hint="How the screen is read.">
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
    hint="Records the screen for 5 seconds with the settings above and keeps the clip so you can watch it. Move something on the screen while it records. Recording from a shortcut is not available yet; this only checks what your PC can do."
  >
    <div class="row">
      <button type="button" class="primary" onclick={tryRecording} disabled={trying || (tools !== null && !tools.found)}>
        {trying ? 'Recording…' : 'Try a 5-second recording'}
      </button>
    </div>
    {#if trying}
      <p class="quiet" role="status">Recording for 5 seconds. Move something on the screen…</p>
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
            <button type="button" onclick={openClip}>Open the clip</button>
          </div>
          {#if openError}<p class="problem" role="alert">{openError}</p>{/if}
        {/if}
      </div>
    {/if}
  </Section>
{/if}

<style>
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; min-width: 0; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .three { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
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
</style>
