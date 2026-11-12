<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import { roleTag } from '../../lib/roles';
  import { KIND_NAME } from '../../lib/connector';
  import type { Block, ServiceKind } from '../../lib/types';
  import RichBox from './RichBox.svelte';
  import { MARKS, type Mark } from './formatting';

  type TextBlock = Extract<Block, { type: 'text' }>;
  let {
    block,
    value = $bindable(''),
    unresolved = [],
    marks = [],
    services = [],
  }: {
    block: TextBlock;
    value: string;
    unresolved?: string[];
    /** The formatting buttons to offer, and the services they are for. None: a plain box. */
    marks?: Mark[];
    services?: ServiceKind[];
  } = $props();
  const id = $derived(`text-${block.id}`);
  const inside = $derived(block.placement === 'inside');
  const tag = $derived(roleTag(block.role));
  const buttons = $derived(MARKS.filter((m) => marks.includes(m.mark)));
  const forWhom = $derived(services.map((k) => KIND_NAME[k]).join(', '));
  let rich = $state<RichBox>();
</script>

<div class="text">
  {#if !inside}
    <label id={`${id}-label`} for={buttons.length > 0 ? undefined : id}>{block.title}{#if tag}<span class="role">{tag}</span>{/if}</label>
  {/if}
  {#if buttons.length > 0}
    <div class="tools" role="toolbar" aria-label={`Formatting for ${forWhom}`} title={`Written the way ${forWhom} shows it when sent`}>
      {#each buttons as b (b.mark)}
        <button
          type="button"
          class={b.mark}
          aria-label={b.label}
          title={b.hint}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => rich?.format(b.mark)}
        >{b.text}</button>
      {/each}
    </div>
  {/if}
  <!-- Title "inside the box" saves a row: it sits in the empty box like a hint and goes away as soon as
       there is text. The title is still the box's tooltip, so hovering the box tells what it is. -->
  <div class="field" class:inside title={inside ? block.title : undefined} data-content>
    {#if inside && value === ''}
      <div class="inner" aria-hidden="true">{block.title}{#if tag}<span class="role">{tag}</span>{/if}</div>
    {/if}
    {#if buttons.length > 0}
      <RichBox
        bind:this={rich}
        bind:value
        {id}
        {marks}
        label={inside ? block.title : undefined}
        labelledby={inside ? undefined : `${id}-label`}
      />
    {:else}
      <textarea {id} bind:value spellcheck="true" aria-label={inside ? block.title : undefined}></textarea>
    {/if}
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
  .text:has(.tools) { grid-template-rows: auto auto 1fr auto; }
  .text:has(.tools):has(.inside) { grid-template-rows: auto 1fr auto; }
  .tools { display: flex; flex-wrap: wrap; gap: 4px; }
  .tools button {
    min-width: 28px; height: 26px; padding: 0 6px; font-size: 12.5px; line-height: 1;
    border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); color: inherit; cursor: pointer;
  }
  .tools button:hover { background: var(--surface-2); }
  .tools .bold { font-weight: 700; }
  .tools .italic { font-style: italic; }
  .tools .underline { text-decoration: underline; }
  .tools .code { font-family: var(--mono); font-size: 11.5px; }
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
