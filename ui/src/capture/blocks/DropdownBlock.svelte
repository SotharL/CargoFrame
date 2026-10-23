<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import type { Block, DropdownState } from '../../lib/types';

  type DropdownBlock = Extract<Block, { type: 'dropdown' }>;
  let {
    block,
    info,
    value = $bindable(null),
  }: { block: DropdownBlock; info: DropdownState; value: string | null } = $props();
  const id = $derived(`dd-${block.id}`);
  const groups = $derived.by(() => {
    const out: { name: string | null; options: DropdownState['options'] }[] = [];
    for (const o of info.options) {
      const last = out[out.length - 1];
      if (last && last.name === o.group) last.options.push(o);
      else out.push({ name: o.group, options: [o] });
    }
    return out;
  });
</script>

<div class="dd">
  <label for={id}>{block.title}</label>
  <select {id} bind:value disabled={info.status !== 'ready'}>
    {#if info.status === 'loading'}
      <option value={null}>Loading…</option>
    {:else if info.status === 'needs_connection'}
      <option value={null}>Not connected</option>
    {:else}
      {#each groups as g, i (i)}
        {#if g.name}
          <optgroup label={g.name}>
            {#each g.options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
          </optgroup>
        {:else}
          {#each g.options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        {/if}
      {/each}
    {/if}
  </select>
  {#if info.status === 'needs_connection'}
    <Chip kind="err" label={info.message ?? 'Not connected'} />
  {/if}
</div>

<style>
  .dd { display: grid; gap: 4px; align-content: start; }
  label { font-weight: 600; font-size: 13px; }
  select { width: 100%; }
</style>
