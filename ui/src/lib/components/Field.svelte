<script lang="ts">
  import type { Snippet } from 'svelte';
  let { label, hint, error, children }: { label: string; hint?: string; error?: string; children: Snippet } = $props();
</script>

<div class="field">
  <label>
    <span class="label">{label}</span>
    {@render children()}
  </label>
  {#if error}<p class="error" role="alert">{error}</p>{:else if hint}<p class="hint quiet">{hint}</p>{/if}
</div>

<style>
  .field { display: grid; gap: 4px; }
  label { display: grid; gap: 4px; }
  .label { font-weight: 600; font-size: 13px; }
  .hint, .error { font-size: 12.5px; }
  .error { color: var(--err); }
  .field :global(input[type='text']), .field :global(select), .field :global(textarea) { width: 100%; }
  .field :global(textarea) {
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; font: inherit; resize: vertical;
  }
</style>
