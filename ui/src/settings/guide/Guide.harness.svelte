<script lang="ts">
  // Test-only: a page with things for a guide to point at.
  import Guide from './Guide.svelte';

  let { hide = '', onpress = () => {} }: { hide?: string; onpress?: (name: string) => void } = $props();
</script>

<button data-tour="one" onclick={() => onpress('one')}>One</button>
{#if hide !== 'two'}<button data-tour="two" onclick={() => onpress('two')}>Two</button>{/if}
<button data-tour="three">Three</button>
<Guide />
