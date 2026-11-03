<script lang="ts">
  // Test-only: holds the bound `trim` the way the capture window does, and reports it.
  import VideoBlock from './VideoBlock.svelte';
  import type { ClipTrim, VideoClipInfo } from '../../lib/types';

  let { info, ontrim }: { info: VideoClipInfo; ontrim: (t: ClipTrim | null) => void } = $props();
  let trim = $state<ClipTrim | null>(null);
  $effect(() => ontrim($state.snapshot(trim) as ClipTrim | null));
</script>

<VideoBlock {info} bind:trim />
