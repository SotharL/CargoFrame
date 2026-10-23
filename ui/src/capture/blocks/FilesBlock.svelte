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
</script>

<div class="files">
  <div class="head">
    <span class="title">{block.title}</span>
    <span class="quiet count">{selected.length} of {info.entries.length} selected</span>
    <span class="spacer"></span>
    <button class="small" onclick={() => (selected = info.entries.map((e) => e.path))} disabled={!info.entries.length}>All</button>
    <button class="small" onclick={() => (selected = [])} disabled={!selected.length}>None</button>
  </div>
  {#if info.problem}
    <Chip kind="dbg" label={info.problem} />
  {/if}
  <ul>
    {#each info.entries as f (f.path)}
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
      {#if !info.problem}<li class="quiet empty">No files match.</li>{/if}
    {/each}
  </ul>
  {#if info.truncated}
    <Chip kind="warn" label="More files exist than are listed" />
  {/if}
</div>

<style>
  .files { display: grid; grid-template-rows: auto auto 1fr auto; gap: 6px; height: 100%; min-height: 0; }
  .head { display: flex; gap: 8px; align-items: center; }
  .title { font-weight: 600; font-size: 13px; }
  .count { font-size: 12px; }
  .spacer { flex: 1; }
  .small { padding: 1px 8px; font-size: 12px; }
  ul {
    list-style: none; margin: 0; padding: 4px; overflow: auto; min-height: 0;
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--radius-sm);
  }
  li label { display: grid; grid-template-columns: auto 1fr auto; gap: 8px; align-items: center; padding: 3px 4px; border-radius: 4px; }
  li label:hover { background: var(--surface-2); }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .size { font-size: 12px; }
  .empty { padding: 6px; }
</style>
