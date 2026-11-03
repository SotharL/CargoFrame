<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { getApi } from '../../lib/ipc';
  import type { VideoClipInfo } from '../../lib/types';
  import { formatSize } from '../values';

  let { info }: { info: VideoClipInfo } = $props();

  let url = $state<string | null>(null);
  let problem = $state('');
  let cannotPlay = $state(false);

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
</script>

<div class="clip">
  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {:else if cannotPlay}
    <p class="quiet note" role="status">This window cannot play the recording, but it is there and will be sent.</p>
  {:else if url}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video src={url} controls muted aria-label="Recording" onerror={() => (cannotPlay = true)}></video>
  {:else}
    <p class="quiet note">Loading the recording…</p>
  {/if}
  <p class="quiet meta">{(info.duration_ms / 1000).toFixed(1)} s · {info.width}×{info.height} · {formatSize(info.bytes)}</p>
</div>

<style>
  .clip { height: 100%; min-height: 0; display: grid; grid-template-rows: 1fr auto; gap: 4px; }
  video { width: 100%; height: 100%; min-height: 0; object-fit: contain; background: #000; border-radius: var(--radius-sm); }
  .note, .problem { margin: 0; padding: 12px; font-size: 13px; }
  .problem { color: var(--err); }
  .meta { margin: 0; font-size: 12px; }
</style>
