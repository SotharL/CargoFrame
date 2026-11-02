<script lang="ts">
  // A drop down you can search: type to narrow the list, arrows and Enter to pick, Esc to close.
  // `free` lets the person keep what they typed when it is not in the list (a field key, say);
  // without it the value is always one of the options.
  import type { ComboOption } from '../types';

  let {
    options,
    value = $bindable(''),
    free = false,
    placeholder = 'Type to search…',
    label,
    id,
    disabled = false,
    emptyText = 'Nothing matches',
    onchange,
  }: {
    options: readonly ComboOption[];
    value: string;
    free?: boolean;
    placeholder?: string;
    /** The accessible name when there is no visible <label for={id}>. */
    label?: string;
    id?: string;
    disabled?: boolean;
    emptyText?: string;
    onchange?: (value: string) => void;
  } = $props();

  const uid = `cb-${Math.random().toString(36).slice(2, 8)}`;
  const listId = `${uid}-list`;

  let open = $state(false);
  let typing = $state(false);
  let query = $state('');
  let active = $state(0);
  let list = $state<HTMLUListElement>();
  let wrap = $state<HTMLDivElement>();
  let pos = $state({ left: 0, top: 0, width: 0, maxH: 240, up: false });

  // The list is fixed to the screen, not to the block that holds the box: a block clips what
  // overflows it, and the list must be able to reach past a small block.
  function place() {
    if (!wrap) return;
    const r = wrap.getBoundingClientRect();
    const below = window.innerHeight - r.bottom - 8;
    const above = r.top - 8;
    const up = below < 160 && above > below;
    pos = {
      left: r.left,
      width: r.width,
      top: up ? r.top - 2 : r.bottom + 2,
      maxH: Math.max(100, Math.min(240, up ? above : below)),
      up,
    };
  }

  $effect(() => {
    if (!open) return;
    place();
    const again = () => place();
    window.addEventListener('resize', again);
    window.addEventListener('scroll', again, true);
    return () => {
      window.removeEventListener('resize', again);
      window.removeEventListener('scroll', again, true);
    };
  });

  const shown = $derived(options.find((o) => o.value === value)?.label ?? (free ? value : ''));
  const matches = $derived.by(() => {
    const q = (typing ? query : '').trim().toLowerCase();
    if (!q) return options;
    return options.filter((o) => `${o.label} ${o.value} ${o.note ?? ''}`.toLowerCase().includes(q));
  });

  $effect(() => {
    // Keep the highlighted entry in view while arrowing through a long list.
    if (open) (list?.children[active] as HTMLElement | undefined)?.scrollIntoView?.({ block: 'nearest' });
  });

  function pick(o: ComboOption) {
    value = o.value;
    typing = false;
    open = false;
    onchange?.(o.value);
  }

  function onInput(e: Event & { currentTarget: HTMLInputElement }) {
    typing = true;
    query = e.currentTarget.value;
    open = true;
    active = 0;
    if (free) {
      value = query;
      onchange?.(query);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!open) {
        open = true;
        return;
      }
      const n = matches.length;
      if (n) active = (active + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
    } else if (e.key === 'Enter' && open) {
      const o = matches[active];
      if (o) {
        e.preventDefault();
        pick(o);
      } else if (free) {
        open = false;
      }
    } else if (e.key === 'Escape' && open) {
      e.preventDefault();
      e.stopPropagation();
      open = false;
      typing = false;
    }
  }

  function onBlur() {
    open = false;
    typing = false;
  }
</script>

<div class="combo" bind:this={wrap}>
  <input
    {id}
    type="text"
    role="combobox"
    aria-label={label}
    aria-expanded={open}
    aria-controls={listId}
    aria-autocomplete="list"
    aria-activedescendant={open && matches[active] ? `${uid}-o${active}` : undefined}
    autocomplete="off"
    spellcheck="false"
    {placeholder}
    {disabled}
    value={typing ? query : shown}
    oninput={onInput}
    onkeydown={onKey}
    onfocus={(e) => { open = true; active = Math.max(0, options.findIndex((o) => o.value === value)); e.currentTarget.select(); }}
    onblur={onBlur}
    onclick={() => (open = true)}
  />
  <span class="caret" aria-hidden="true">▾</span>
  {#if open}
    <ul
      id={listId}
      role="listbox"
      bind:this={list}
      aria-label={label}
      style:left="{pos.left}px"
      style:top="{pos.top}px"
      style:width="{pos.width}px"
      style:max-height="{pos.maxH}px"
      style:transform={pos.up ? 'translateY(-100%)' : undefined}
    >
      {#each matches as o, i (i)}
        <li
          id="{uid}-o{i}"
          role="option"
          aria-selected={o.value === value}
          class:active={i === active}
          onpointerdown={(e) => { e.preventDefault(); pick(o); }}
          onpointerenter={() => (active = i)}
        >
          <span class="text">{o.label}</span>
          {#if o.note}<span class="note">{o.note}</span>{/if}
        </li>
      {:else}
        <li class="none" role="presentation">{free && typing && query.trim() ? `Use “${query.trim()}”` : emptyText}</li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .combo { position: relative; width: 100%; }
  input { width: 100%; padding-right: 26px; }
  .caret { position: absolute; right: 9px; top: 50%; transform: translateY(-50%); color: var(--quiet); pointer-events: none; font-size: 12px; }
  ul {
    position: fixed; z-index: 50; box-sizing: border-box;
    margin: 0; padding: 4px; list-style: none; overflow-y: auto;
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--radius-sm);
    box-shadow: 0 6px 18px color-mix(in srgb, #000 22%, transparent);
  }
  li { display: flex; gap: 8px; justify-content: space-between; align-items: baseline; padding: 5px 8px; border-radius: 5px; cursor: pointer; }
  li.active { background: color-mix(in srgb, var(--accent) 18%, transparent); }
  li[aria-selected='true'] .text { font-weight: 600; }
  .text { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .note { flex: none; font-size: 11.5px; color: var(--quiet); }
  .none { color: var(--quiet); cursor: default; }
</style>
