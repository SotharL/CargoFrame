<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import type { Block } from '../../lib/types';

  type SharingBlock = Extract<Block, { type: 'sharing_button' }>;
  let {
    block,
    enabled,
    reasons,
    sending = false,
    onshare,
  }: {
    block: SharingBlock;
    enabled: boolean;
    reasons: string[];
    sending?: boolean;
    onshare: () => void;
  } = $props();
</script>

<div class="share">
  <button class="primary" disabled={!enabled} aria-busy={sending} onclick={onshare}>
    {sending ? 'Sending…' : block.label || 'Share'}
  </button>
  {#if !enabled && !sending && reasons.length > 0}
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
