<script lang="ts">
  import type { Snippet } from 'svelte';
  let { title, hint, children }: { title: string; hint?: string; children: Snippet } = $props();
</script>

<section>
  <h2>{title}</h2>
  {#if hint}<p class="quiet hint">{hint}</p>{/if}
  <div class="body">{@render children()}</div>
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
  .body { display: grid; gap: 12px; margin-top: 4px; }
</style>
