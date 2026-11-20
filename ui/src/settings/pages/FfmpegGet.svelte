<script lang="ts">
  // Getting ffmpeg from inside CargoFrame, on Windows and Linux: the static build of ffmpeg's
  // latest release, checked with its fingerprints and kept in CargoFrame's own folder (no
  // administrator). Offered when ffmpeg is missing, and to update the one in use.
  import { t } from '../../lib/i18n/index.svelte';
  import { onDestroy, onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { formatSize } from '../../capture/values';
  import { getApi } from '../../lib/ipc';
  import type { FfmpegPhase } from '../../lib/types';

  let { missing, ondone }: { missing: boolean; ondone?: () => void } = $props();
  let phase = $state<FfmpegPhase>({ state: 'idle' });
  let error = $state('');
  let stop: (() => void) | undefined;
  let gone = false;
  let reported = false;

  async function read() {
    phase = await (await getApi()).ffmpegStatus();
    if (phase.state === 'done' && !reported) {
      reported = true;
      ondone?.();
    }
  }

  onMount(async () => {
    await read();
    const unlisten = await (await getApi()).onFfmpegChanged(() => void read());
    if (gone) unlisten();
    else stop = unlisten;
  });
  onDestroy(() => {
    gone = true;
    stop?.();
  });

  async function look(thenGet: boolean) {
    error = '';
    try {
      const api = await getApi();
      phase = await api.ffmpegCheck();
      if (thenGet && phase.state === 'offered') await get();
    } catch (e) {
      error = String(e);
    }
  }

  async function get() {
    error = '';
    reported = false;
    try {
      await (await getApi()).ffmpegDownload();
      await read();
    } catch (e) {
      error = String(e);
    }
  }

  const percent = $derived(
    phase.state === 'downloading' && phase.total ? Math.min(100, Math.round((phase.done / phase.total) * 100)) : null,
  );
</script>

<div class="get">
  {#if phase.state === 'checking'}
    <Chip kind="dbg" label={t('Looking for the newest ffmpeg…')} />
  {:else if phase.state === 'downloading'}
    <div role="status" class="row">
      <Chip kind="dbg" label={percent === null ? t('Downloading ffmpeg…') : t('Downloading ffmpeg: {percent}%', { percent })} />
      <progress max="100" value={percent ?? undefined}></progress>
    </div>
  {:else if phase.state === 'unpacking'}
    <Chip kind="dbg" label={t('Checking and unpacking ffmpeg…')} />
  {:else if phase.state === 'done'}
    <Chip kind="ok" label={t('ffmpeg {version} is ready', { version: phase.version })} />
  {:else if phase.state === 'current'}
    <div class="row">
      <Chip kind="ok" label={t('This is the newest ffmpeg')} />
      <button type="button" onclick={() => look(false)}>{t('Look again')}</button>
    </div>
  {:else if phase.state === 'offered'}
    <div class="row">
      <button type="button" class="primary" onclick={get}>
        {missing ? t('Download ffmpeg {version} ({size})', { version: phase.version, size: formatSize(phase.size) }) : t('Update to ffmpeg {version} ({size})', { version: phase.version, size: formatSize(phase.size) })}
      </button>
    </div>
  {:else}
    {#if phase.state === 'failed'}<p class="err selectable" role="alert">{phase.message}</p>{/if}
    <div class="row">
      {#if missing}
        <button type="button" class="primary" onclick={() => look(true)}>{t('Download ffmpeg')}</button>
      {:else}
        <button type="button" onclick={() => look(false)}>{t('Look for a newer ffmpeg')}</button>
      {/if}
    </div>
  {/if}
  <p class="quiet small">{t('From the builds ffmpeg.org links to (BtbN, on GitHub), checked with their fingerprints and kept in CargoFrame\'s own folder: no administrator needed. CargoFrame uses it first from then on.')}</p>
  {#if error}<p class="err selectable" role="alert">{error}</p>{/if}
</div>

<style>
  .get { display: grid; gap: 6px; margin-top: 4px; }
  .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .small { margin: 0; font-size: 12px; }
  .err { margin: 0; color: var(--err); }
  .primary { font-weight: 600; }
  progress { width: 160px; }
</style>
