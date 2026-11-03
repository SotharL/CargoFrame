<script lang="ts">
  // Test-only: holds the bound `trim` the way the capture window does, and reports it.
  import VideoBlock from './VideoBlock.svelte';
  import type { ClipEdit, VideoClipInfo } from '../../lib/types';

  let { info, onedit }: { info: VideoClipInfo; onedit: (e: ClipEdit | null) => void } = $props();
  let edit = $state<ClipEdit | null>(null);
  $effect(() => onedit($state.snapshot(edit) as ClipEdit | null));
</script>

<VideoBlock {info} bind:edit />
