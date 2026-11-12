<script lang="ts">
  // In the top-right corner of a block: one small box per sharing button, in that button's colour. A ticked
  // box (with a ✓, so colour is never the only sign) means that button sends this block; clear it
  // to leave the block out of that button's share. A box that cannot be ticked says why when the
  // pointer rests on it. Hovering or focusing a box lights up its button.
  import { kindStyle } from '../lib/connector';
  import type { TickBox } from './ticks';

  let {
    boxes,
    active = null,
    onchange,
    onhover,
  }: {
    boxes: TickBox[];
    /** The button under the pointer, whose blocks and boxes are lit. */
    active?: string | null;
    onchange: (buttonId: string, on: boolean) => void;
    onhover?: (buttonId: string | null) => void;
  } = $props();
</script>

{#if boxes.length > 0}
  <div class="ticks" role="group" aria-label="Sent by">
    {#each boxes as b (b.buttonId)}
      <label
        class="box"
        class:on={b.checked}
        class:blocked={b.disabled}
        class:locked={b.locked}
        class:lit={active === b.buttonId}
        style={kindStyle(b.kind)}
        title={b.title}
        onmouseenter={() => onhover?.(b.buttonId)}
        onmouseleave={() => onhover?.(null)}
      >
        <input
          type="checkbox"
          checked={b.checked}
          disabled={b.disabled || b.locked}
          aria-label={b.title}
          onchange={(e) => onchange(b.buttonId, e.currentTarget.checked)}
          onfocus={() => onhover?.(b.buttonId)}
          onblur={() => onhover?.(null)}
        />
        <span class="square" aria-hidden="true">{b.checked ? '✓' : ''}</span>
        {#if b.badge}<span class="badge" aria-hidden="true">{b.badge}</span>{/if}
      </label>
    {/each}
  </div>
{/if}

<style>
  /* From the block's top-right corner: their top on the block's top edge. */
  .ticks {
    position: absolute; top: 0; right: 0; z-index: 5;
    display: flex; align-items: center; gap: 1px; padding: 0 3px;
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: 999px;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.18);
  }
  .box { position: relative; display: inline-flex; align-items: center; gap: 2px; padding: 3px 2px; cursor: pointer; }
  .box input { position: absolute; opacity: 0; inset: 0; margin: 0; cursor: inherit; }
  .square {
    display: inline-grid; place-items: center; width: 16px; height: 16px; box-sizing: border-box;
    border: 2px solid var(--kind-line); border-radius: 4px; background: transparent;
    font-size: 11px; font-weight: 800; line-height: 1; color: var(--on-kind);
  }
  .on .square { background: var(--kind); }
  .blocked { cursor: not-allowed; }
  .blocked .square { border-style: dashed; border-color: var(--border-strong); opacity: 0.6; }
  .locked { cursor: not-allowed; }
  .locked .square { box-shadow: 0 0 0 1px var(--surface), 0 0 0 2px var(--kind-line); }
  .lit .square { box-shadow: 0 0 0 2px var(--surface), 0 0 0 4px var(--kind-line); }
  .box:has(input:focus-visible) .square { outline: 2px solid var(--text); outline-offset: 1px; }
  .badge { font-size: 10px; font-weight: 700; color: var(--quiet); line-height: 1; margin-right: 2px; }
</style>
