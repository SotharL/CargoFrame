<script lang="ts">
  // Test-only: holds the bound crop the way the video block does, and reports it.
  import CropBox from './CropBox.svelte';

  type Box = { x: number; y: number; w: number; h: number };
  let { start, oncrop }: { start: Box; oncrop: (c: Box) => void } = $props();
  // The test hands over the starting box once.
  // svelte-ignore state_referenced_locally
  let crop = $state<Box>({ ...start });
  $effect(() => oncrop({ ...crop }));
</script>

<CropBox width={800} height={400} bind:crop />
