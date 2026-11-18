<script lang="ts">
  import type { Snippet } from 'svelte';
  // `aside` sits on the title's line (a choice short enough to need no row of its own); `tour`
  // names the section for the guides.
  let { title, hint, aside, tour, children }: { title: string; hint?: string; aside?: Snippet; tour?: string; children?: Snippet } = $props();
</script>

<section data-tour={tour}>
  {#if aside}
    <div class="head"><h2>{title}</h2>{@render aside()}</div>
  {:else}
    <h2>{title}</h2>
  {/if}
  {#if hint}<p class="quiet hint">{hint}</p>{/if}
  {#if children}<div class="body">{@render children()}</div>{/if}
</section>

<style>
  section {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
    display: grid;
    gap: 8px;
  }
  .hint { font-size: 13px; }
  .head { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .head h2 { margin: 0; }
  .body { display: grid; gap: 12px; margin-top: 4px; }
</style>
