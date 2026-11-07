<script lang="ts">
  // A small "Show me how" button that starts one guide from where the person already is.
  import { startTour } from './guide.svelte';

  let { tour }: { tour: string } = $props();
</script>

<button type="button" class="link" onclick={() => startTour(tour, true)}>Show me how</button>

<style>
  .link { border: 0; background: none; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font: inherit; font-size: 13px; }
</style>
