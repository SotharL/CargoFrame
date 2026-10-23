<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import type { Block } from '../../lib/types';

  type SharingBlock = Extract<Block, { type: 'sharing_button' }>;
  let {
    block,
    enabled,
    reasons,
    onshare,
  }: { block: SharingBlock; enabled: boolean; reasons: string[]; onshare: () => void } = $props();
</script>

<div class="share">
  <button class="primary" disabled={!enabled} onclick={onshare}>{block.label || 'Share'}</button>
  {#if !enabled}
    <div class="why">
      {#each reasons as r (r)}<Chip kind="warn" label={r} />{/each}
    </div>
  {/if}
</div>

<style>
  .share { display: grid; gap: 6px; align-content: start; }
  .primary { width: 100%; padding: 8px 12px; }
  .why { display: flex; flex-wrap: wrap; gap: 4px; }
</style>
