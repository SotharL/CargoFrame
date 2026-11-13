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
    lit = false,
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
    /** One of this button's boxes is under the pointer. */
    lit?: boolean;
    onshare: () => void;
    /** Hovering or focusing the button lights up the blocks it sends. */
    onhover?: (on: boolean) => void;
  } = $props();

  const verb = $derived(mode === 'save_locally' ? 'Saves' : 'Sends');
  /** One short line: what goes, then the chat that opens. The whole of it is in the tooltip. */
  const summary = $derived(
    parts.length === 0 ? '' : `${verb}: ${parts.join(' · ')}${opening ? ` → ${opening}` : ''}`,
  );
  const full = $derived([summary, note].filter(Boolean).join('\n'));
</script>

<div class="share" style={kindStyle(kind)}>
  <button
    class="send"
    class:lit
    disabled={!enabled}
    aria-busy={sending}
    title={[mode === 'save_locally' ? '' : block.label, `Sends to ${KIND_NAME[kindKey(kind)]}`, full].filter(Boolean).join('\n')}
    onclick={onshare}
    onmouseenter={() => onhover?.(true)}
    onmouseleave={() => onhover?.(false)}
    onfocus={() => onhover?.(true)}
    onblur={() => onhover?.(false)}
  >
    <span class="label">
      {#if mode === 'save_locally'}
        {sending ? 'Saving…' : 'Save locally'}
      {:else}
        {sending ? 'Sending…' : block.label || 'Share'}
      {/if}
    </span>
  </button>
  {#if enabled && !sending && (summary || note)}
    <p class="quiet line selectable" title={full}>{summary || note}</p>
  {/if}
  {#if !enabled && !sending && reasons.length > 0}
    <div class="why">
      {#each reasons as r (r)}<Chip kind="warn" label={r} />{/each}
    </div>
  {/if}
</div>

<style>
  /* Whatever its size, the block never grows past its cell: the label and the reasons are cut
     (the button's tooltip says them in full). */
  .share { display: grid; gap: 4px; align-content: start; min-width: 0; min-height: 0; overflow: hidden; }
  .send {
    width: 100%; min-width: 0; padding: 8px 12px; font-weight: 600;
    background: var(--kind); color: var(--on-kind);
    border: 2px solid var(--kind-line);
  }
  .send:hover:not(:disabled) { background: var(--kind); filter: brightness(1.1); }
  .send:disabled { opacity: 0.55; }
  .send:focus-visible { outline: 2px solid var(--kind-line); outline-offset: 2px; }
  .send.lit { box-shadow: 0 0 0 3px var(--bg), 0 0 0 5px var(--kind-line); }
  .label { display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .why { display: flex; flex-wrap: wrap; gap: 4px; min-width: 0; overflow: hidden; }
  /* A single line; the full text is the tooltip. */
  .line { margin: 0; font-size: 12.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* A block only one row high has no room for it: the button's tooltip says it instead. */
  @container (max-height: 72px) {
    .line { display: none; }
  }
</style>
