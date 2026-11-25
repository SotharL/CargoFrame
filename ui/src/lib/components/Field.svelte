<script lang="ts">
  import type { Snippet } from 'svelte';
  /** `tour` names this field for the guides (settings/guide/tours.ts). */
  let { label, hint, error, tour, children }: { label: string; hint?: string; error?: string; tour?: string; children: Snippet } = $props();
</script>

<div class="field" data-tour={tour}>
  <label>
    <span class="label">{label}</span>
    {@render children()}
  </label>
  {#if error}<p class="error" role="alert">{error}</p>{:else if hint}<p class="hint quiet">{hint}</p>{/if}
</div>

<style>
  /* Kept at the top of a taller neighbour's row: a select is never stretched to fill it. */
  .field { display: grid; gap: 4px; align-content: start; }
  label { display: grid; gap: 4px; align-content: start; }
  .label { font-weight: 600; font-size: 13px; }
  .hint, .error { font-size: 12.5px; }
  .error { color: var(--err); }
  .field :global(input[type='text']), .field :global(select), .field :global(textarea) { width: 100%; }
  .field :global(textarea) {
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; font: inherit; resize: vertical;
  }
</style>
