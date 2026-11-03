<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import { kindStyle, KIND_NAME, kindKey } from '../../lib/connector';
  import type { Block, ServiceKind } from '../../lib/types';

  type SharingBlock = Extract<Block, { type: 'sharing_button' }>;
  let {
    block,
    enabled,
    reasons,
    mode = 'send',
    note = null,
    kind = null,
    parts = [],
    opening = null,
    sending = false,
    onshare,
    onhover,
  }: {
    block: SharingBlock;
    enabled: boolean;
    reasons: string[];
    /** `save_locally`: no active connection, so the button saves a zip and says so. */
    mode?: 'send' | 'save_locally';
    note?: string | null;
    /** The destination, which gives the button its colour. `null` saves a zip here. */
    kind?: ServiceKind | null;
    /** What a click carries right now ("Screenshot (edited)", "3 files (12 KB)", "Prompt"…). */
    parts?: string[];
    /** The Claude chat that opens, said before the click. */
    opening?: string | null;
    sending?: boolean;
    onshare: () => void;
    /** Hovering or focusing the button lights up the blocks it sends. */
    onhover?: (on: boolean) => void;
  } = $props();
</script>

<div class="share" style={kindStyle(kind)}>
  <button
    class="send"
    disabled={!enabled}
    aria-busy={sending}
    title={`Sends to ${KIND_NAME[kindKey(kind)]}`}
    onclick={onshare}
    onmouseenter={() => onhover?.(true)}
    onmouseleave={() => onhover?.(false)}
    onfocus={() => onhover?.(true)}
    onblur={() => onhover?.(false)}
  >
    {#if mode === 'save_locally'}
      {sending ? 'Saving…' : 'Save locally'}
    {:else}
      {sending ? 'Sending…' : block.label || 'Share'}
    {/if}
  </button>
  {#if enabled && !sending}
    {#if opening}<p class="quiet line selectable">Opening: {opening}</p>{/if}
    {#if parts.length > 0}
      <p class="quiet line selectable">{mode === 'save_locally' ? 'Will save' : 'Will send'}: {parts.join(' · ')}</p>
    {/if}
    {#if note}<p class="quiet line selectable">{note}</p>{/if}
  {/if}
  {#if !enabled && !sending && reasons.length > 0}
    <div class="why">
      {#each reasons as r (r)}<Chip kind="warn" label={r} />{/each}
    </div>
  {/if}
</div>

<style>
  .share { display: grid; gap: 6px; align-content: start; }
  .send {
    width: 100%; padding: 8px 12px; font-weight: 600;
    background: var(--kind); color: var(--on-kind);
    border: 2px solid var(--kind-line);
  }
  .send:hover:not(:disabled) { background: var(--kind); filter: brightness(1.1); }
  .send:disabled { opacity: 0.55; }
  .send:focus-visible { outline: 2px solid var(--kind-line); outline-offset: 2px; }
  .why { display: flex; flex-wrap: wrap; gap: 4px; }
  .line { margin: 0; font-size: 12.5px; overflow-wrap: anywhere; }
</style>
