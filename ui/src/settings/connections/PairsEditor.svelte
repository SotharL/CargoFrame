<script lang="ts">
  import type { Pair } from '../../lib/types';

  let {
    pairs = $bindable(),
    label,
    nameLabel = 'Name',
    valueLabel = 'Value',
    addLabel = 'Add',
  }: {
    pairs: Pair[];
    label: string;
    nameLabel?: string;
    valueLabel?: string;
    addLabel?: string;
  } = $props();
</script>

<div class="pairs">
  <span class="label">{label}</span>
  {#each pairs as p, i (i)}
    <div class="row">
      <input type="text" aria-label="{label}: {nameLabel} {i + 1}" placeholder={nameLabel} bind:value={p.name} />
      <input type="text" aria-label="{label}: {valueLabel} {i + 1}" placeholder={valueLabel} bind:value={p.value} />
      <button type="button" aria-label="Remove {label} {i + 1}" onclick={() => pairs.splice(i, 1)}>Remove</button>
    </div>
  {/each}
  <div><button type="button" onclick={() => pairs.push({ name: '', value: '' })}>{addLabel}</button></div>
</div>

<style>
  .pairs { display: grid; gap: 6px; }
  .label { font-weight: 600; font-size: 13px; }
  .row { display: grid; grid-template-columns: 1fr 2fr auto; gap: 6px; }
</style>
