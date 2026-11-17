<script lang="ts" generics="T extends string">
  // The tabs at the top of a Settings page: Values | Files, Layouts | Fill fields. Arrow keys move
  // between them. `tour` names a tab for the guides (settings/guide/tours.ts).
  let {
    tabs,
    value = $bindable(),
    label,
  }: {
    tabs: { id: T; label: string; count?: number; tour?: string }[];
    value: T;
    label: string;
  } = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return;
    e.preventDefault();
    const i = tabs.findIndex((t) => t.id === value);
    value = tabs[(i + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length]!.id;
    document.getElementById(`tab-${value}`)?.focus();
  }
</script>

<div class="tabs" role="tablist" aria-label={label} tabindex="-1" onkeydown={onKey}>
  {#each tabs as item (item.id)}
    <button
      id="tab-{item.id}"
      type="button"
      role="tab"
      data-tour={item.tour}
      aria-selected={value === item.id}
      aria-controls="panel-{item.id}"
      tabindex={value === item.id ? 0 : -1}
      onclick={() => (value = item.id)}
    >{item.label}{#if item.count !== undefined} <span class="count">{item.count}</span>{/if}</button>
  {/each}
</div>

<style>
  .tabs { display: flex; gap: 4px; border-bottom: 1px solid var(--border); margin-bottom: 14px; }
  [role='tab'] {
    background: none; border: none; border-bottom: 2px solid transparent; border-radius: 0;
    padding: 8px 14px; font-weight: 600; color: var(--quiet); cursor: pointer; margin-bottom: -1px;
  }
  [role='tab'][aria-selected='true'] { color: var(--text); border-bottom-color: var(--accent); }
  [role='tab']:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .count { font-weight: 400; color: var(--quiet); margin-left: 4px; }
</style>
