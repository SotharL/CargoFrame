<script lang="ts">
  // Under a block: which sharing buttons send it. Each button is shown in its destination's
  // colour with a ✓ or ✕ and its name, so the colour is never the only signal.
  import { kindStyle } from '../lib/connector';
  import type { BlockMarks } from './willSend';

  let { marks, active = null }: { marks: BlockMarks; active?: string | null } = $props();
</script>

{#if marks.marks.length > 0 || marks.notSent}
  <ul class="marks" aria-label="Sent by">
    {#each marks.marks as m (m.buttonId)}
      <li
        class="mark {m.state}"
        class:active={active === m.buttonId}
        style={kindStyle(m.kind)}
        title={m.state === 'sent' ? `${m.label} sends this` : `${m.label} cannot send this: ${m.why}`}
      >
        <span aria-hidden="true">{m.state === 'sent' ? '✓' : '✕'}</span>
        {m.label}{#if m.state === 'cannot_send'}<span class="why"> · {m.why}</span>{/if}
      </li>
    {/each}
    {#if marks.notSent}
      <li class="mark none" title={marks.notSentWhy ?? undefined}><span aria-hidden="true">–</span> Not sent</li>
    {/if}
  </ul>
{/if}

<style>
  .marks { list-style: none; display: flex; flex-wrap: wrap; gap: 4px; margin: 4px 0 0; padding: 0; }
  .mark {
    display: inline-flex; align-items: baseline; gap: 4px; padding: 0 7px;
    border: 1px solid var(--kind-line); border-radius: 999px; font-size: 11.5px; line-height: 18px;
  }
  .sent { background: var(--kind); color: var(--on-kind); }
  .cannot_send { background: transparent; color: var(--text); border-style: dashed; border-color: var(--err); }
  .none { border-color: var(--border-strong); color: var(--quiet); }
  .why { font-size: 11px; opacity: 0.85; }
  .active { outline: 2px solid var(--kind-line); outline-offset: 1px; }
</style>
