<script lang="ts">
  // Holds a video block as the layout page does, so edits to it can be read back in a test.
  import VideoOptions from './VideoOptions.svelte';
  import type { Block } from '../../lib/types';

  type Video = Extract<Block, { type: 'video' }>;
  let { initial, onchange }: { initial: Video; onchange: (b: Video) => void } = $props();
  let block = $state<Video>(copyOfInitial());
  function copyOfInitial(): Video {
    return structuredClone($state.snapshot(initial)) as Video;
  }
  $effect(() => onchange($state.snapshot(block) as Video));
</script>

<VideoOptions {block} />
