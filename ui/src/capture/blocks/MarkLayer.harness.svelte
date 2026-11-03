<script lang="ts">
  // Test-only: holds the bound mark position the way the video block does, and reports it.
  import MarkLayer from './MarkLayer.svelte';

  let { onat }: { onat: (a: { x: number; y: number }) => void } = $props();
  let at = $state({ x: 400, y: 200 });
  $effect(() => onat({ ...at }));
</script>

<MarkLayer width={800} height={400} bind:at text="" />
