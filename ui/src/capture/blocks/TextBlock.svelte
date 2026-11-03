<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import { roleTag } from '../../lib/roles';
  import type { Block } from '../../lib/types';

  type TextBlock = Extract<Block, { type: 'text' }>;
  let {
    block,
    value = $bindable(''),
    unresolved = [],
  }: { block: TextBlock; value: string; unresolved?: string[] } = $props();
  const id = $derived(`text-${block.id}`);
  const inside = $derived(block.placement === 'inside');
  const tag = $derived(roleTag(block.role));
</script>

<div class="text">
  {#if !inside}
    <label for={id}>{block.title}{#if tag}<span class="role">{tag}</span>{/if}</label>
  {/if}
  <!-- Title "inside the box" saves a row: it sits in the empty box like a hint, goes away as soon as
       there is text, and comes back as a small tab on the box's edge while the pointer is over it. -->
  <div class="field" class:inside title={inside ? block.title : undefined}>
    {#if inside}
      {#if value === ''}
        <div class="inner" aria-hidden="true">{block.title}{#if tag}<span class="role">{tag}</span>{/if}</div>
      {:else}
        <div class="tab" aria-hidden="true">{block.title}{#if tag}<span class="role">{tag}</span>{/if}</div>
      {/if}
    {/if}
    <textarea {id} bind:value spellcheck="true" aria-label={inside ? block.title : undefined}></textarea>
  </div>
  {#if unresolved.length}
    <div class="chips">
      {#each unresolved as name (name)}
        <Chip kind="dbg" label={`{{${name}}} has no value`} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .text { display: grid; grid-template-rows: auto 1fr auto; gap: 4px; height: 100%; min-height: 0; }
  .text:has(.inside) { grid-template-rows: 1fr auto; }
  label { font-weight: 600; font-size: 13px; display: flex; gap: 6px; align-items: center; }
  .role {
    font-weight: 600; font-size: 11px; padding: 0 6px; border-radius: 999px;
    border: 1px solid var(--accent); color: var(--text);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .field { position: relative; min-height: 0; display: grid; }
  textarea {
    width: 100%; height: 100%; min-height: 0; resize: none;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 8px; font: inherit; color: inherit;
  }
  .inner {
    position: absolute; top: 9px; left: 9px; right: 9px; color: var(--quiet); font-size: 13px;
    display: flex; gap: 6px; align-items: center; pointer-events: none;
  }
  .tab {
    position: absolute; top: -9px; left: 8px; max-width: calc(100% - 16px); z-index: 1;
    padding: 0 8px; border-radius: 999px; border: 1px solid var(--border-strong); background: var(--surface-2);
    color: var(--quiet); font-size: 11px; display: flex; gap: 6px; align-items: center;
    overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
    opacity: 0; transition: opacity 0.12s; pointer-events: none;
  }
  .field:hover .tab { opacity: 1; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
</style>
