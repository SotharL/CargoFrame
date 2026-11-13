<script lang="ts">
  // In the top-right corner of a block: one dot per sharing button, in that button's colour. A full
  // dot means that button sends this block; an empty ring (still in the button's colour, so it is
  // told apart, and full against empty is never colour alone) leaves the block out of that
  // button's share. Click to switch. A dot that cannot be switched on says why when the pointer
  // rests on it. Hovering or focusing a dot lights up its button.
  //
  // The dots sit at the end of the block's first row (its title, or its tools), which keeps room
  // for them (`--corner`, see cornerWidth); a block with no such row has them on its top edge.
  import { kindStyle } from '../lib/connector';
  import type { TickBox } from './ticks';

  let {
    boxes,
    active = null,
    edge = false,
    onchange,
    onhover,
  }: {
    boxes: TickBox[];
    /** The button under the pointer, whose blocks and boxes are lit. */
    active?: string | null;
    /** The block has no first row to sit in: the dots go on its top edge. */
    edge?: boolean;
    onchange: (buttonId: string, on: boolean) => void;
    onhover?: (buttonId: string | null) => void;
  } = $props();
</script>

{#if boxes.length > 0}
  <div class="ticks" class:edge role="group" aria-label="Sent by">
    {#each boxes as b (b.buttonId)}
      <label
        class="dot"
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
        <span class="mark" aria-hidden="true">{b.badge ?? ''}</span>
      </label>
    {/each}
  </div>
{/if}

<style>
  /* No container: the dots alone, centred on the block's first row (22 px high). */
  .ticks { position: absolute; top: 0; right: 0; z-index: 5; height: 22px; display: flex; align-items: center; }
  /* On the top edge: centred on the block's border, in the gap above it. */
  .ticks.edge { top: -11px; }
  .dot { position: relative; display: inline-grid; place-items: center; width: 18px; height: 18px; cursor: pointer; }
  .dot input { position: absolute; opacity: 0; inset: 0; margin: 0; cursor: inherit; }
  .mark {
    display: inline-grid; place-items: center; width: 14px; height: 14px; box-sizing: border-box;
    border: 2px solid var(--kind-line); border-radius: 50%; background: var(--surface);
    font-size: 8.5px; font-weight: 800; line-height: 1; color: var(--kind-line);
    transition: background 0.12s, box-shadow 0.12s;
  }
  .on .mark { background: var(--kind); color: var(--on-kind); }
  .dot:hover .mark { box-shadow: 0 0 0 2px color-mix(in srgb, var(--kind-line) 30%, transparent); }
  .blocked { cursor: not-allowed; }
  .blocked .mark { border-style: dashed; opacity: 0.55; }
  .locked { cursor: not-allowed; }
  .locked .mark { box-shadow: 0 0 0 1px var(--surface), 0 0 0 2px var(--kind-line); }
  .lit .mark { box-shadow: 0 0 0 2px var(--surface), 0 0 0 4px var(--kind-line); }
  .dot:has(input:focus-visible) .mark { outline: 2px solid var(--text); outline-offset: 1px; }
</style>
