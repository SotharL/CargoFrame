<script lang="ts">
  // Tabs, like a binder: the names on one side, and the page of the open tab. The window draws
  // the page (the blocks of every tab, the closed ones hidden so what is typed in them stays).
  import type { Snippet } from 'svelte';
  import type { TabsBlock } from '../../lib/tabs';

  let {
    block,
    open = $bindable(0),
    lit = [],
    children,
  }: {
    block: TabsBlock;
    /** The index of the open tab. */
    open?: number;
    /** The tabs holding a block the sharing button under the pointer sends: their names light up. */
    lit?: boolean[];
    children: Snippet;
  } = $props();

  const side = $derived(block.side ?? 'top');
  const across = $derived(side === 'top' || side === 'bottom');
  let names = $state<HTMLButtonElement[]>([]);

  /** The arrow keys go from tab to tab, as in any list of tabs. */
  function onKey(e: KeyboardEvent, i: number) {
    const back = across ? 'ArrowLeft' : 'ArrowUp';
    const next = across ? 'ArrowRight' : 'ArrowDown';
    const n = block.tabs.length;
    const to = e.key === back ? (i - 1 + n) % n : e.key === next ? (i + 1) % n : e.key === 'Home' ? 0 : e.key === 'End' ? n - 1 : -1;
    if (to < 0) return;
    e.preventDefault();
    open = to;
    names[to]?.focus();
  }
</script>

<div class="tabs {side}">
  <div class="names" role="tablist" aria-label="Tabs" aria-orientation={across ? 'horizontal' : 'vertical'}>
    {#each block.tabs as t, i (i)}
      <button
        bind:this={names[i]}
        type="button"
        role="tab"
        id="{block.id}-tab-{i}"
        aria-selected={open === i}
        aria-controls="{block.id}-page"
        tabindex={open === i ? 0 : -1}
        class:lit={lit[i]}
        title={t.name || `Tab ${i + 1}`}
        onclick={() => (open = i)}
        onkeydown={(e) => onKey(e, i)}
      >{t.name || `Tab ${i + 1}`}</button>
    {/each}
  </div>
  <div class="page" id="{block.id}-page" role="tabpanel" aria-labelledby="{block.id}-tab-{open}">
    {@render children()}
  </div>
</div>

<style>
  .tabs { height: 100%; min-height: 0; display: grid; }
  .top { grid-template-rows: auto minmax(0, 1fr); }
  .bottom { grid-template-rows: minmax(0, 1fr) auto; }
  .left { grid-template-columns: auto minmax(0, 1fr); }
  .right { grid-template-columns: minmax(0, 1fr) auto; }
  .bottom .names, .right .names { order: 2; }

  .names { display: flex; gap: 2px; min-width: 0; min-height: 0; overflow: auto; scrollbar-width: none; }
  .left .names, .right .names { flex-direction: column; }
  /* A name is a binder's tab: it joins the page when open. */
  .names button {
    flex: none; max-width: 16ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    padding: 4px 12px; font-size: 13px; color: var(--quiet); background: var(--surface-2);
    border: 1px solid var(--border); position: relative;
  }
  .top .names button { border-radius: var(--radius-sm) var(--radius-sm) 0 0; margin-bottom: -1px; }
  .bottom .names button { border-radius: 0 0 var(--radius-sm) var(--radius-sm); margin-top: -1px; }
  .left .names button { border-radius: var(--radius-sm) 0 0 var(--radius-sm); margin-right: -1px; }
  .right .names button { border-radius: 0 var(--radius-sm) var(--radius-sm) 0; margin-left: -1px; }
  .names button[aria-selected='true'] { color: var(--text); font-weight: 600; background: var(--surface); z-index: 1; }
  .top .names button[aria-selected='true'] { border-bottom-color: var(--surface); }
  .bottom .names button[aria-selected='true'] { border-top-color: var(--surface); }
  .left .names button[aria-selected='true'] { border-right-color: var(--surface); }
  .right .names button[aria-selected='true'] { border-left-color: var(--surface); }
  /* A closed tab that holds what the hovered sharing button sends. */
  .names button.lit { box-shadow: inset 0 -3px 0 var(--kind-line, var(--accent)); }

  .page {
    min-width: 0; min-height: 0; overflow: auto; padding: 8px;
    border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface);
  }
</style>
