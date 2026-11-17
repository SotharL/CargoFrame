<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import Tr from '../../lib/i18n/Tr.svelte';
  // Data & fields: values read from the files of a capture (`{map}`, `{gpu}`), and the files they
  // are read in. A value is made by selecting it in a file; it can be read in several files, the
  // newest winning. Layouts write the values in titles, descriptions, tags and fields (Fill fields
  // is a tab of the Layouts page). Stored as data rules, one per file (settings/data/values.ts).
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import PageTabs from '../../lib/components/PageTabs.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Layout, Rule, RuleValue, ValuesPreview, VariableInfo } from '../../lib/types';
  import Combined from '../data/Combined.svelte';
  import FilesTab from '../data/FilesTab.svelte';
  import SelectValue, { type Mode } from '../data/SelectValue.svelte';
  import ValueSources from '../data/ValueSources.svelte';
  import ValuesTab from '../data/ValuesTab.svelte';
  import { addSource, replaceStep, setAll, valueList, type Changes, type ValueEntry } from '../data/values';
  import { go, route } from '../route.svelte';

  const rules = createItems<Rule>('rule');
  const layouts = createItems<Layout>('layout');
  let preview = $state<ValuesPreview | null>(null);
  let variables = $state<VariableInfo[]>([]);

  type Tab = 'values' | 'files';
  type View =
    | { kind: 'list' }
    | { kind: 'select'; mode: Mode; initial: File[]; back: string | null }
    | { kind: 'sources'; key: string }
    | { kind: 'combined'; id: string | null };
  let tab = $state<Tab>(route.arg === 'files' ? 'files' : 'values');
  let view = $state<View>({ kind: 'list' });
  let q = $state('');

  // Fill fields moved to the Layouts page in 0.5.0: an old address goes there
  $effect(() => {
    if (route.page !== 'rules') return;
    if (route.arg === 'fields') go('layouts', 'fields');
    else if (route.arg === 'files' || route.arg === 'values') {
      tab = route.arg;
      view = { kind: 'list' };
    }
  });

  const entries = $derived(valueList(rules.items));
  const fileCount = $derived(new Set(rules.items.map((r) => r.item.file.trim()).filter(Boolean)).size);
  const current = $derived.by(() => {
    const v = view;
    return v.kind === 'sources' ? (entries.find((e) => e.key === v.key) ?? null) : null;
  });

  async function refresh() {
    const api = await getApi();
    [preview, variables] = await Promise.all([api.previewValues().catch(() => null), api.listVariables().catch(() => [])]);
  }

  onMount(async () => {
    await Promise.all([rules.reload(), layouts.reload()]);
    await refresh();
  });

  /** Save what changed, one rule after the other. */
  async function apply(c: Changes) {
    for (const s of c.save) await rules.save(s.id, s.rule);
    for (const id of c.remove) await rules.remove(id);
    await refresh();
  }

  /** A value picked in a file: in the rule of each file named, the newest/all choice the same everywhere. */
  async function saveSelected(step: RuleValue, files: string[], replace: string | null) {
    if (replace) await apply(replaceStep(rules.items, replace, step));
    for (const file of files) await apply(addSource(rules.items, file, step));
    await apply(setAll(rules.items, step.key, { across: step.across ?? 'newest' }));
    view = view.kind === 'select' && view.back ? { kind: 'sources', key: view.back } : { kind: 'list' };
  }

  function select(mode: Mode, initial: File[] = [], back: string | null = null) {
    view = { kind: 'select', mode, initial, back };
  }

  function open(v: ValueEntry) {
    if (v.combined) view = { kind: 'combined', id: v.sources[0]!.ruleId };
    else view = { kind: 'sources', key: v.key };
  }

  const keysBut = (ruleId: string | null) =>
    entries.filter((e) => !e.sources.every((s) => s.ruleId === ruleId)).map((e) => e.key);
</script>

<div class="page">
  {#if rules.failure}<Chip kind="err" label={rules.failure} />{/if}
  {#each rules.problems as p (p)}<Chip kind="warn" label={t('Could not read a data file: {problem}', { problem: p })} />{/each}

  {#if view.kind === 'select'}
    <SelectValue
      mode={view.mode}
      initial={view.initial}
      taken={entries.map((e) => e.key)}
      onsave={saveSelected}
      oncancel={() => (view = view.kind === 'select' && view.back ? { kind: 'sources', key: view.back } : { kind: 'list' })}
    />
  {:else if view.kind === 'sources' && current}
    {@const entry = current}
    <ValueSources
      {entry}
      rules={rules.items}
      layouts={layouts.items}
      {preview}
      {apply}
      onadd={() => select({ kind: 'add', key: entry.key, name: entry.name, across: entry.across, fallback: entry.fallback }, [], entry.key)}
      onchangesource={(s) => select({ kind: 'change', source: s, across: entry.across }, [], entry.key)}
      onback={() => (view = { kind: 'list' })}
    />
  {:else if view.kind === 'combined'}
    {@const id = view.id}
    <Combined
      {id}
      rule={rules.find(id) ?? null}
      taken={keysBut(id)}
      {variables}
      onsave={async (rid, rule) => { await rules.save(rid, rule); await refresh(); view = { kind: 'list' }; }}
      oncancel={() => (view = { kind: 'list' })}
    />
  {:else}
    <PageTabs
      label={t('Data & fields')}
      bind:value={tab}
      tabs={[
        { id: 'values', label: t('Values'), count: entries.length, tour: 'data.values' },
        { id: 'files', label: t('Files'), count: fileCount, tour: 'data.files' },
      ]}
    />
    <div id="panel-{tab}" role="tabpanel" aria-labelledby="tab-{tab}" class="panel">
      {#if tab === 'values'}
        <p class="tried quiet">
          {#if preview?.folder}<Tr text={t('Results are from the last capture, in {folder}.')}>{#snippet folder()}<span class="mono">{preview!.folder}</span>{/snippet}</Tr>{:else}{t('Results show after the first capture.')}{/if}
          {t('A value with nothing found stays empty.')}
        </p>
        <div class="buttons">
          <button type="button" class="primary" data-tour="data.new" onclick={() => select({ kind: 'new' })}>{t('＋ New value: select it in a file')}</button>
          <button type="button" data-tour="data.combined" onclick={() => (view = { kind: 'combined', id: null })}>{t('＋ Made from other values')}</button>
          {#if entries.length > 0}<input type="search" class="find" bind:value={q} placeholder={t('Find a value')} aria-label={t('Find a value')} />{/if}
        </div>
        {#if rules.loaded && entries.length === 0}
          <div class="empty">
            <h2>{t('No values yet')}</h2>
            <p class="quiet">
              <Tr text={t("Select a value in a file of a capture, like the map in a game's log or the version in package.json. CargoFrame finds it again in each capture, and layouts write it as {token} in a title, a description, a tag or a field.")}>{#snippet token()}<code>{'{map}'}</code>{/snippet}</Tr>
            </p>
          </div>
        {:else}
          <ValuesTab {entries} {preview} {q} layouts={layouts.items} onopen={open} />
          <p class="quiet small"><Tr text={t('Names written the older way, like {old}, keep working.')}>{#snippet old()}<code>{'{{rule.gpu}}'}</code>{/snippet}</Tr></p>
        {/if}
      {:else}
        <p class="tried quiet">{t('Files looked for in a capture\'s project folder. Each can be added to the window\'s files, and values are read in it.')}</p>
        <FilesTab
          rules={rules.items}
          onsave={async (id, rule) => { await rules.save(id, rule); await refresh(); }}
          onremove={async (id) => { await rules.remove(id); await refresh(); }}
          ondrop={(list) => select({ kind: 'new' }, list)}
        />
      {/if}
    </div>
  {/if}
</div>

<style>
  .page { display: grid; gap: 12px; max-width: 1280px; }
  .panel { display: grid; gap: 12px; }
  .tried { margin: 0; font-size: 13px; }
  .mono { font-family: var(--mono); }
  .buttons { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .find { margin-left: auto; width: 240px; }
  .empty { display: grid; gap: 6px; background: var(--surface); border: 1px dashed var(--border-strong); border-radius: var(--radius); padding: 16px; }
  .empty h2 { margin: 0; font-size: 15px; }
  .empty p { margin: 0; }
  code { font: 12.5px var(--mono); background: var(--surface-2); padding: 1px 6px; border-radius: 5px; }
  .small { font-size: 12.5px; margin: 0; }
</style>
