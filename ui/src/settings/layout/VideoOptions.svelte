<script lang="ts">
  // The settings of a video block: what is recorded (the screen, or the window that has the focus
  // when the recording starts) and which sounds go in. The devices come from the computer, asked
  // once when the block is opened.
  import { onMount } from 'svelte';
  import Field from '../../lib/components/Field.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import { getApi } from '../../lib/ipc';
  import { DEFAULT_VIDEO_AUDIO } from '../../lib/types';
  import type { AudioChoice, AudioDevice, AudioDevices, Block, VideoSubject } from '../../lib/types';

  let { block }: { block: Extract<Block, { type: 'video' }> } = $props();

  let devices = $state<AudioDevices | null>(null);
  let failed = $state('');

  onMount(async () => {
    try {
      devices = await (await getApi()).listAudioDevices();
    } catch (e) {
      failed = typeof e === 'string' ? e : 'The sound devices could not be listed.';
    }
  });

  const subject = $derived<VideoSubject>(block.subject ?? 'screen');
  const audio = $derived(block.audio ?? DEFAULT_VIDEO_AUDIO);

  const toValue = (c: AudioChoice): string => (typeof c === 'string' ? c : `device:${c.device}`);
  const fromValue = (v: string): AudioChoice => (v === 'off' || v === 'default' ? v : { device: v.slice('device:'.length) });

  function setAudio(which: 'desktop' | 'microphone', value: string) {
    block.audio = { ...audio, [which]: fromValue(value) };
  }

  /** The device the layout names but this computer does not list (a layout moved from another PC). */
  const unknown = (choice: AudioChoice, list: AudioDevice[]) =>
    typeof choice !== 'string' && !list.some((d) => d.name === choice.device) ? choice.device : null;

  const note = $derived(failed || devices?.note || '');
</script>

<Field label="Record" hint={subject === 'focused_window' ? 'The window that has the focus when you press the shortcut, as it is placed and sized then. If it cannot be found, the whole screen is recorded.' : ''}>
  <select value={subject} onchange={(e) => (block.subject = e.currentTarget.value as VideoSubject)}>
    <option value="screen">The whole screen</option>
    <option value="focused_window">The window in focus</option>
  </select>
</Field>

<Field label="Sound of the computer">
  <select value={toValue(audio.desktop)} onchange={(e) => setAudio('desktop', e.currentTarget.value)}>
    <option value="default">On: what this computer plays</option>
    <option value="off">Off</option>
    {#each devices?.desktop ?? [] as d (d.name)}
      <option value={`device:${d.name}`}>{d.label}</option>
    {/each}
    {#if unknown(audio.desktop, devices?.desktop ?? [])}
      {@const name = unknown(audio.desktop, devices?.desktop ?? [])}
      <option value={`device:${name}`}>{name} (not found on this computer)</option>
    {/if}
  </select>
</Field>

<Field label="Microphone">
  <select value={toValue(audio.microphone)} onchange={(e) => setAudio('microphone', e.currentTarget.value)}>
    <option value="off">Off</option>
    <option value="default">On: the default microphone</option>
    {#each devices?.microphone ?? [] as d (d.name)}
      <option value={`device:${d.name}`}>{d.label}</option>
    {/each}
    {#if unknown(audio.microphone, devices?.microphone ?? [])}
      {@const name = unknown(audio.microphone, devices?.microphone ?? [])}
      <option value={`device:${name}`}>{name} (not found on this computer)</option>
    {/if}
  </select>
</Field>

<div class="end">
  <Toggle label="Take a screenshot when the recording stops" checked={block.screenshot_at_end === true} onchange={(v) => (block.screenshot_at_end = v)} />
  <p class="quiet small">For a layout with a screenshot block too. Off, that block starts empty, and “Take screenshot” under the recording keeps the frame on screen.</p>
</div>

{#if note}<p class="quiet small" role="note">{note}</p>{/if}

<style>
  .small { font-size: 12.5px; margin: 0; }
  .end { display: grid; gap: 4px; }
</style>
