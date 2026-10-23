<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { getApi } from '../../lib/ipc';
  import type {
    ImportAction, ImportPreview, ImportPreviewItem, ImportReport, ItemEntry, Layout, Rule,
    TransferGroup, InfoType,
  } from '../../lib/types';

  const GROUPS: { group: TransferGroup; label: string; kind: 'layout' | 'rule' | 'infotype' }[] = [
    { group: 'layouts', label: 'Layouts', kind: 'layout' },
    { group: 'rules', label: 'Data rules', kind: 'rule' },
    { group: 'infotypes', label: 'Info types', kind: 'infotype' },
  ];

  type Row = { id: string; name: string; on: boolean };
  let rows = $state<Record<TransferGroup, Row[]>>({ layouts: [], rules: [], infotypes: [] });
  let exported = $state<{ kind: 'ok' | 'err'; text: string; path?: string } | null>(null);
  let busy = $state(false);

  let preview = $state<ImportPreview | null>(null);
  let actions = $state<Record<string, ImportAction>>({});
  let report = $state<ImportReport | null>(null);
  let importError = $state('');

  const keyOf = (i: { group: TransferGroup; id: string }) => `${i.group}/${i.id}`;
  const selectedCount = $derived(GROUPS.reduce((n, g) => n + rows[g.group].filter((r) => r.on).length, 0));
  const total = $derived(GROUPS.reduce((n, g) => n + rows[g.group].length, 0));
  const toImport = $derived(preview?.items.filter((i) => !i.error && actions[keyOf(i)] !== 'skip').length ?? 0);

  async function loadRows() {
    const api = await getApi();
    const nameOf = (e: ItemEntry<Layout | Rule | InfoType>) => (e.item as { name?: string }).name?.trim() || e.id;
    // Fetch every group first so the lists appear together, not one after another.
    const lists = await Promise.all(
      GROUPS.map((g) => api.listItems<Layout | Rule | InfoType>(g.kind).catch(() => null)),
    );
    GROUPS.forEach((g, n) => {
      const before = new Map(rows[g.group].map((r) => [r.id, r.on]));
      rows[g.group] = (lists[n]?.items ?? []).map((e) => ({
        id: e.id,
        name: nameOf(e),
        on: before.get(e.id) ?? true,
      }));
    });
  }

  onMount(loadRows);

  function setGroup(group: TransferGroup, on: boolean) {
    for (const r of rows[group]) r.on = on;
  }

  async function doExport() {
    busy = true;
    exported = null;
    try {
      const selection = GROUPS.map((g) => ({
        group: g.group,
        ids: rows[g.group].filter((r) => r.on).map((r) => r.id),
      })).filter((s) => s.ids.length > 0);
      const path = await (await getApi()).exportSettings(selection);
      if (path) exported = { kind: 'ok', text: 'Exported', path };
    } catch (e) {
      exported = { kind: 'err', text: String(e) };
    }
    busy = false;
  }

  /** New items are brought in by default; ones that exist are kept next to the old one when they can be. */
  function defaultAction(i: ImportPreviewItem): ImportAction {
    if (i.error) return 'skip';
    if (!i.exists) return 'replace';
    return i.can_copy ? 'copy' : 'skip';
  }

  async function chooseFile() {
    busy = true;
    importError = '';
    report = null;
    try {
      const p = await (await getApi()).openImport();
      if (p) {
        preview = p;
        actions = Object.fromEntries(p.items.map((i) => [keyOf(i), defaultAction(i)]));
      }
    } catch (e) {
      preview = null;
      importError = String(e);
    }
    busy = false;
  }

  async function doImport() {
    if (!preview) return;
    busy = true;
    importError = '';
    try {
      const picks = preview.items
        .filter((i) => !i.error)
        .map((i) => ({ group: i.group, id: i.id, action: actions[keyOf(i)] ?? 'skip' }));
      report = await (await getApi()).applyImport(picks);
      preview = null;
      await loadRows();
    } catch (e) {
      importError = String(e);
    }
    busy = false;
  }

  async function cancelImport() {
    preview = null;
    importError = '';
    await (await getApi()).discardImport().catch(() => {});
  }

  const groupLabel = (g: TransferGroup) => GROUPS.find((x) => x.group === g)!.label;
</script>

<Section
  title="Export and import"
  hint="Back up layouts, data rules and info types, or move them to another computer. Connections, passwords, keys and shortcuts are never included."
>
  <div class="panel">
    <h3>Export</h3>
    {#if total === 0}
      <p class="quiet">Nothing to export yet. Layouts, data rules and info types you create show up here.</p>
    {:else}
      {#each GROUPS as g (g.group)}
        {#if rows[g.group].length > 0}
          <details class="group">
            <summary>
              <input
                type="checkbox"
                aria-label={`All ${g.label.toLowerCase()}`}
                checked={rows[g.group].every((r) => r.on)}
                indeterminate={rows[g.group].some((r) => r.on) && !rows[g.group].every((r) => r.on)}
                onclick={(e) => e.stopPropagation()}
                onchange={(e) => setGroup(g.group, e.currentTarget.checked)}
              />
              <span>{g.label}</span>
              <span class="quiet">{rows[g.group].filter((r) => r.on).length} of {rows[g.group].length}</span>
            </summary>
            <ul>
              {#each rows[g.group] as r (r.id)}
                <li><label><input type="checkbox" bind:checked={r.on} /> {r.name}</label></li>
              {/each}
            </ul>
          </details>
        {/if}
      {/each}
      <div class="line">
        <button type="button" onclick={doExport} disabled={busy || selectedCount === 0}>
          Export {selectedCount} {selectedCount === 1 ? 'item' : 'items'}…
        </button>
        {#if exported}
          <Chip kind={exported.kind} label={exported.text} />
          {#if exported.path}<span class="mono selectable path">{exported.path}</span>{/if}
        {/if}
      </div>
    {/if}
  </div>

  <div class="panel">
    <h3>Import</h3>
    {#if !preview}
      <p class="quiet">
        Choose a file made by Export. You see what is in it, and what is already here, before anything changes.
      </p>
      <div class="line">
        <button type="button" onclick={chooseFile} disabled={busy}>Choose a file…</button>
        {#if importError}<Chip kind="err" label={importError} />{/if}
      </div>
    {:else}
      <p>From <span class="mono selectable">{preview.file_name}</span></p>
      <ul class="items">
        {#each preview.items as i (keyOf(i))}
          <li>
            <div class="what">
              <strong>{i.name}</strong>
              <span class="quiet">{groupLabel(i.group)}</span>
              {#if i.error}
                <Chip kind="err" label={`Cannot be imported: ${i.error}`} />
              {:else if i.exists}
                <Chip kind="warn" label={`Already here${i.existing_name ? ` as “${i.existing_name}”` : ''}`} />
              {:else}
                <Chip kind="ok" label="New" />
              {/if}
              {#each i.notes as n (n)}<p class="quiet note">{n}</p>{/each}
            </div>
            {#if !i.error}
              <select bind:value={actions[keyOf(i)]} aria-label={`What to do with ${i.name}`}>
                {#if i.exists}
                  <option value="skip">Leave mine</option>
                  <option value="replace">Replace mine</option>
                  {#if i.can_copy}<option value="copy">Import as a copy</option>{/if}
                {:else}
                  <option value="replace">Import</option>
                  <option value="skip">Skip</option>
                {/if}
              </select>
            {/if}
          </li>
        {/each}
      </ul>
      <div class="line">
        <button class="primary" type="button" onclick={doImport} disabled={busy || toImport === 0}>
          Import {toImport} {toImport === 1 ? 'item' : 'items'}
        </button>
        <button type="button" onclick={cancelImport} disabled={busy}>Cancel</button>
        {#if importError}<Chip kind="err" label={importError} />{/if}
      </div>
    {/if}
    {#if report}
      <div class="report" role="status">
        <Chip
          kind={report.imported.length ? 'ok' : 'warn'}
          label={`Imported ${report.imported.length}${report.skipped ? `, left ${report.skipped} as they were` : ''}`}
        />
        {#each report.failed as f (f)}<Chip kind="err" label={f} />{/each}
      </div>
    {/if}
  </div>
</Section>

<style>
  .panel { display: grid; gap: 8px; }
  .panel + .panel { border-top: 1px solid var(--border); padding-top: 12px; }
  h3 { font-size: 13.5px; }
  .group { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 6px 10px; }
  .group summary { display: flex; gap: 8px; align-items: center; cursor: pointer; }
  .group ul { list-style: none; margin: 8px 0 2px; padding: 0 0 0 26px; display: grid; gap: 4px; }
  .line { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .items { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .items li {
    display: flex; gap: 12px; justify-content: space-between; align-items: start;
    padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  }
  .what { display: flex; gap: 6px 8px; align-items: center; flex-wrap: wrap; min-width: 0; }
  .note { flex-basis: 100%; margin: 0; font-size: 12.5px; }
  .report { display: flex; gap: 6px; flex-wrap: wrap; }
  .path { font-size: 12.5px; overflow-wrap: anywhere; }
</style>
