<script lang="ts">
  import Chip from '../lib/components/Chip.svelte';
  import type { ShareResult } from '../lib/types';

  let {
    result,
    onopenlink,
    onreconnect,
    onclose,
    opened = null,
  }: {
    result: ShareResult;
    onopenlink: (url: string) => void;
    onreconnect: () => void;
    onclose: () => void;
    /** The Claude chat that was opened, when the share went to one. */
    opened?: string | null;
  } = $props();

  const placeText: Record<string, string> = {
    safe_folder: 'A copy was saved in your safe folder',
    picked_folder: 'A copy was saved in the folder you chose',
    kept_locally: 'A copy was kept in CargoFrame’s own folder',
    not_saved: 'The copy could not be saved. It is still here',
  };
</script>

<div class="panel" role="status" aria-live="polite">
  {#if result.status === 'sent'}
    <Chip kind="ok" label={result.summary} />
    {#if opened}<Chip kind="ok" label={`Opened: ${opened}`} />{/if}
    {#each result.notes as n (n)}<Chip kind="warn" label={n} />{/each}
    {#if result.url}
      <button onclick={() => onopenlink(result.url!)}>Open</button>
    {/if}
    {#if !result.close}<button onclick={onclose}>Close</button>{/if}
  {:else if result.status === 'saved_locally'}
    <Chip kind="ok" label="Saved locally" />
    <span class="selectable path">{result.path}</span>
    {#each result.notes as n (n)}<Chip kind="warn" label={n} />{/each}
    {#if !result.close}<button onclick={onclose}>Close</button>{/if}
  {:else if result.status === 'blocked'}
    {#each result.reasons as r (r)}<Chip kind="warn" label={r} />{/each}
  {:else}
    <Chip kind="err" label={`${result.label}: ${result.message}`} />
    {#if result.reconnect}<button onclick={onreconnect}>Reconnect…</button>{/if}
    {#if result.saved}
      <Chip
        kind={result.saved.place === 'not_saved' ? 'err' : 'ok'}
        label={`${placeText[result.saved.place]}: ${result.saved.path}`}
      />
      {#if result.saved.note}<span class="quiet note">{result.saved.note}</span>{/if}
    {:else}
      <Chip kind="err" label="No copy could be made" />
    {/if}
  {/if}
</div>

<style>
  .panel { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .note { font-size: 12.5px; }
  .path { font-size: 12.5px; overflow-wrap: anywhere; }
</style>
