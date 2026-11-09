<script lang="ts">
  // Data & fields: read values from the project's files, and fill the service's fields with them.
  // Two tabs on one page, because the second mostly uses what the first finds.
  import Rules from './Rules.svelte';
  import Fields from './Fields.svelte';
  import { route } from '../route.svelte';

  type Tab = 'files' | 'fields';
  const TABS: { id: Tab; label: string }[] = [
    { id: 'files', label: 'Read from files' },
    { id: 'fields', label: 'Fill fields' },
  ];
  let tab = $state<Tab>(route.arg === 'fields' ? 'fields' : 'files');
  $effect(() => {
    if (route.page === 'rules' && (route.arg === 'fields' || route.arg === 'files')) tab = route.arg;
  });

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return;
    e.preventDefault();
    const i = TABS.findIndex((t) => t.id === tab);
    tab = TABS[(i + (e.key === 'ArrowRight' ? 1 : TABS.length - 1)) % TABS.length]!.id;
    document.getElementById(`tab-${tab}`)?.focus();
  }
</script>

<div class="tabs" role="tablist" aria-label="Data & fields" tabindex="-1" onkeydown={onKey}>
  <button
    id="tab-{TABS[0]!.id}"
    type="button"
    role="tab"
    data-tour="data.files"
    aria-selected={tab === TABS[0]!.id}
    aria-controls="panel-{TABS[0]!.id}"
    tabindex={tab === TABS[0]!.id ? 0 : -1}
    onclick={() => (tab = TABS[0]!.id)}
  >{TABS[0]!.label}</button>
  <button
    id="tab-{TABS[1]!.id}"
    type="button"
    role="tab"
    data-tour="data.fields"
    aria-selected={tab === TABS[1]!.id}
    aria-controls="panel-{TABS[1]!.id}"
    tabindex={tab === TABS[1]!.id ? 0 : -1}
    onclick={() => (tab = TABS[1]!.id)}
  >{TABS[1]!.label}</button>
</div>
<div id="panel-{tab}" role="tabpanel" aria-labelledby="tab-{tab}">
  {#if tab === 'files'}<Rules />{:else}<Fields />{/if}
</div>

<style>
  .tabs { display: flex; gap: 4px; border-bottom: 1px solid var(--border); margin-bottom: 14px; }
  [role='tab'] {
    background: none; border: none; border-bottom: 2px solid transparent; border-radius: 0;
    padding: 8px 14px; font-weight: 600; color: var(--quiet); cursor: pointer; margin-bottom: -1px;
  }
  [role='tab'][aria-selected='true'] { color: var(--text); border-bottom-color: var(--accent); }
  [role='tab']:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
</style>
