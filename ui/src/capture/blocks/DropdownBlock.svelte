<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import Combobox from '../../lib/components/Combobox.svelte';
  import type { Block, ComboOption, DropdownState } from '../../lib/types';

  /** From this many choices on, the list can be searched instead of scrolled. */
  const SEARCH_FROM = 8;

  type DropdownBlock = Extract<Block, { type: 'dropdown' }>;
  let {
    block,
    info,
    value = $bindable(null),
  }: { block: DropdownBlock; info: DropdownState; value: string | null } = $props();
  const id = $derived(`dd-${block.id}`);
  const searchable = $derived(info.status === 'ready' && info.options.length > SEARCH_FROM);
  const choices = $derived<ComboOption[]>(info.options.map((o) => ({ value: o.value, label: o.label, note: o.group })));
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
  {#if searchable}
    <Combobox {id} options={choices} value={value ?? ''} onchange={(v) => (value = v)} placeholder="Search {info.options.length} choices…" />
  {:else}
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
  {/if}
  {#if info.status === 'needs_connection'}
    <Chip kind="err" label={info.message ?? 'Not connected'} />
  {/if}
</div>

<style>
  .dd { display: grid; gap: 4px; align-content: start; }
  label { font-weight: 600; font-size: 13px; }
  select { width: 100%; }
</style>
