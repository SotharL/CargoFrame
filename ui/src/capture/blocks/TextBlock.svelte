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
  <!-- Title "inside the box" saves a row: it sits in the empty box like a hint and goes away as soon as
       there is text. The title is still the box's tooltip, so hovering the box tells what it is. -->
  <div class="field" class:inside title={inside ? block.title : undefined}>
    {#if inside && value === ''}
      <div class="inner" aria-hidden="true">{block.title}{#if tag}<span class="role">{tag}</span>{/if}</div>
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
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
</style>
