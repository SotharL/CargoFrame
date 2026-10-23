<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import type { Block } from '../../lib/types';

  type SharingBlock = Extract<Block, { type: 'sharing_button' }>;
  let {
    block,
    enabled,
    reasons,
    mode = 'send',
    note = null,
    sending = false,
    onshare,
  }: {
    block: SharingBlock;
    enabled: boolean;
    reasons: string[];
    /** `save_locally`: no active connection, so the button saves a zip and says so. */
    mode?: 'send' | 'save_locally';
    note?: string | null;
    sending?: boolean;
    onshare: () => void;
  } = $props();
</script>

<div class="share">
  <button class="primary" disabled={!enabled} aria-busy={sending} onclick={onshare}>
    {#if mode === 'save_locally'}
      {sending ? 'Saving…' : 'Save locally'}
    {:else}
      {sending ? 'Sending…' : block.label || 'Share'}
    {/if}
  </button>
  {#if enabled && note && !sending}<p class="quiet note selectable">{note}</p>{/if}
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
  .note { margin: 0; font-size: 12.5px; overflow-wrap: anywhere; }
</style>
