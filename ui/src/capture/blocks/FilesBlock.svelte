<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import type { Block, FilesState } from '../../lib/types';
  import { formatSize, toggleFile } from '../values';

  type FilesBlock = Extract<Block, { type: 'files' }>;
  let {
    block,
    info,
    selected = $bindable([]),
  }: { block: FilesBlock; info: FilesState; selected: string[] } = $props();

  // A long list gets a filter; All and None act on what the filter leaves.
  let filter = $state('');
  const visible = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return q ? info.entries.filter((e) => e.path.toLowerCase().includes(q)) : info.entries;
  });
  const selectAll = () => (selected = [...new Set([...selected, ...visible.map((e) => e.path)])]);
  const selectNone = () => (selected = selected.filter((p) => !visible.some((e) => e.path === p)));
</script>

<div class="files">
  <div class="head">
    <span class="title">{block.title}</span>
    <span class="quiet count">{selected.length} of {info.entries.length} selected{filter.trim() ? ` · ${visible.length} shown` : ''}</span>
    <!-- Next to the title, so the right corner is left to the boxes of the sharing buttons. -->
    <button class="small" onclick={selectAll} disabled={!visible.length}>All</button>
    <button class="small" onclick={selectNone} disabled={!selected.length}>None</button>
  </div>
  {#if info.entries.length > 6}
    <input type="text" class="filter" bind:value={filter} placeholder="Filter files…" aria-label="Filter files" />
  {/if}
  {#if info.problem}
    <Chip kind="dbg" label={info.problem} />
  {/if}
  {#if info.places.length > 1}
    <p class="quiet from" title={info.places.map((p) => p.folder).join('\n')}>
      Looked in: {info.places.map((p) => `${p.label} (${p.exists ? p.found : 'not found'})`).join(', ')}
    </p>
  {/if}
  <ul data-content>
    {#each visible as f (f.path)}
      <li>
        <label>
          <input
            type="checkbox"
            checked={selected.includes(f.path)}
            onchange={(e) => (selected = toggleFile(selected, f.path, e.currentTarget.checked))}
          />
          <span class="path mono selectable" title={f.path}>{f.path}</span>
          <span class="quiet size">{formatSize(f.size)}</span>
        </label>
      </li>
    {:else}
      {#if !info.problem}<li class="quiet empty">{filter.trim() ? 'No file matches the filter.' : 'No files match.'}</li>{/if}
    {/each}
  </ul>
  {#if info.truncated}
    <Chip kind="warn" label="More files exist than are listed" />
  {/if}
</div>

<style>
  /* A column, not a grid with fixed rows: the list takes whatever height is left and scrolls inside
     itself, whether or not the notices above and below it are showing. */
  .files { display: flex; flex-direction: column; gap: 6px; height: 100%; min-height: 0; }
  .filter { width: 100%; flex: none; }
  .from { margin: 0; font-size: 12px; flex: none; }
  .files > :global(.chip) { flex: none; align-self: flex-start; }
  .head { display: flex; gap: 8px; align-items: center; flex: none; }
  .title { font-weight: 600; font-size: 13px; }
  .count { font-size: 12px; }
  .small { padding: 1px 8px; font-size: 12px; }
  ul {
    list-style: none; margin: 0; padding: 4px; overflow-y: auto; overflow-x: hidden;
    flex: 1 1 0; min-height: 84px;
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--radius-sm);
  }
  li label { display: grid; grid-template-columns: auto 1fr auto; gap: 8px; align-items: center; padding: 3px 4px; border-radius: 4px; }
  li label:hover { background: var(--surface-2); }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .size { font-size: 12px; }
  .empty { padding: 6px; }
</style>
