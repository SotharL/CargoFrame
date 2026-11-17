<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import Tr from '../../lib/i18n/Tr.svelte';
  // The files Data & fields looks for in a capture's project folder: whether each is added to the
  // window's files, how its lines write their time, and the values read in it.
  import Field from '../../lib/components/Field.svelte';
  import TimeFormatField from '../../lib/components/TimeFormatField.svelte';
  import type { ItemEntry, Rule } from '../../lib/types';
  import { token } from '../../lib/valueNames';
  import { blankRule, onlyCombines, valuesOf } from '../ruleValues';

  let {
    rules,
    onsave,
    onremove,
    ondrop,
  }: {
    rules: ItemEntry<Rule>[];
    onsave: (id: string | null, rule: Rule) => Promise<void>;
    onremove: (id: string) => Promise<void>;
    /** Files dropped here: pick a value in them. */
    ondrop: (files: File[]) => void;
  } = $props();

  const files = $derived(rules.filter(({ item }) => !(onlyCombines(item) && !item.file.trim())));
  let draft = $state<{ id: string | null; rule: Rule } | null>(null);
  let confirming = $state<string | null>(null);
  let failure = $state('');
  let over = $state(false);

  function edit(id: string | null, r: Rule) {
    failure = '';
    draft = { id, rule: { ...structuredClone($state.snapshot(r) as Rule), several: r.several ?? 'newest', add_to_files: r.add_to_files ?? false, log_time: r.log_time ?? '' } };
  }

  async function save() {
    if (!draft) return;
    failure = '';
    const rule = $state.snapshot(draft.rule) as Rule;
    if (!rule.file.trim()) {
      failure = t('Write the file’s name, or part of it.');
      return;
    }
    if (!rule.name.trim()) rule.name = rule.file.trim();
    try {
      await onsave(draft.id, rule);
      draft = null;
    } catch (e) {
      failure = String(e);
    }
  }

  function drop(e: DragEvent) {
    e.preventDefault();
    over = false;
    const list = [...(e.dataTransfer?.files ?? [])];
    if (list.length) ondrop(list);
  }
</script>

<div class="files">
  {#each files as { id, item } (id)}
    {@const values = valuesOf(id, item)}
    {#if draft && draft.id === id}
      {@render editor()}
    {:else}
      <article class="card">
        <div class="main">
          <h3 class="mono">{item.file}</h3>
          {#if item.name.trim() && item.name.trim() !== item.file.trim()}<p class="quiet small">{item.name}</p>{/if}
          <p class="small">
            {item.add_to_files ? t('Added to the window’s files') : t('Only read')}, {item.several === 'all' ? t('every file with this name') : t('the newest if several have this name')}{#if item.log_time}, <Tr text={t('lines’ time {format}')}>{#snippet format()}<span class="mono">{item.log_time}</span>{/snippet}</Tr>{/if}
          </p>
          {#if values.length}
            <p class="tokens">{#each values as v (v.key)}<code>{token(v.key)}</code>{/each}</p>
          {/if}
        </div>
        <div class="actions">
          <button type="button" onclick={() => edit(id, item)}>{t('Edit')}</button>
          {#if confirming === id}
            <button type="button" class="danger" onclick={async () => { await onremove(id); confirming = null; }}>{t('Delete for good')}</button>
            <button type="button" onclick={() => (confirming = null)}>{t('Keep')}</button>
          {:else}
            <button type="button" onclick={() => (confirming = id)}>{t('Delete')}</button>
          {/if}
        </div>
      </article>
    {/if}
  {/each}

  {#if draft && draft.id === null}{@render editor()}{/if}

  {#if !draft}
    <div class="bottom">
      <button type="button" data-tour="rules.add" onclick={() => edit(null, { ...blankRule(), add_to_files: true })}>{t('＋ Add a file')}</button>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="drop" class:over ondragover={(e) => { e.preventDefault(); over = true; }} ondragleave={() => (over = false)} ondrop={drop}>
        {t('Drop a file here to pick a value in it')}
      </div>
    </div>
  {/if}
</div>

{#snippet editor()}
  {#if draft}
    <form class="card editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <Field label={t('File')} hint={t('Its name or part of it, looked for in every folder of the project: Game.log, .log, package.json. Or a path: logs/*.log.')} tour="rules.file">
        <input type="text" class="mono" bind:value={draft.rule.file} placeholder="Game.log" />
      </Field>
      <label class="check">
        <span>{t('If several files have this name, read')}</span>
        <select bind:value={draft.rule.several}>
          <option value="newest">{t('the newest')}</option>
          <option value="all">{t('all of them')}</option>
        </select>
      </label>
      <label class="check" data-tour="rules.add_files">
        <input type="checkbox" bind:checked={draft.rule.add_to_files} />
        <span>{t('Add it to the window’s files')} <span class="quiet">{t('(ticked in the Files box, ready to send)')}</span></span>
      </label>
      <div class="time" data-tour="rules.log_time">
        <TimeFormatField id="rule-log-time" label={t('In a log viewer, its lines’ time looks like')} bind:value={draft.rule.log_time} />
      </div>
      {#if failure}<p class="error" role="alert">{failure}</p>{/if}
      <div class="actions">
        <button type="submit" class="primary" data-tour="rules.save">{t('Save')}</button>
        <button type="button" onclick={() => (draft = null)}>{t('Cancel')}</button>
      </div>
    </form>
  {/if}
{/snippet}

<style>
  .files { display: grid; gap: 10px; }
  .card { display: flex; flex-wrap: wrap; gap: 12px 16px; justify-content: space-between; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 14px; }
  .main { display: grid; gap: 4px; min-width: 0; }
  h3 { margin: 0; font-size: 14px; }
  .mono { font-family: var(--mono); }
  .small { font-size: 13px; margin: 0; }
  .tokens { display: flex; flex-wrap: wrap; gap: 6px; margin: 2px 0 0; }
  code { font: 12px var(--mono); padding: 1px 6px; border-radius: 5px; background: color-mix(in srgb, var(--accent) 14%, var(--surface)); }
  .actions { display: flex; gap: 8px; align-items: center; align-self: start; flex-wrap: wrap; }
  .editor { display: grid; gap: 12px; justify-content: stretch; }
  .check { display: flex; gap: 8px; align-items: center; font-size: 13px; }
  .check select { width: auto; }
  .time { max-width: 440px; }
  .bottom { display: flex; flex-wrap: wrap; gap: 10px; align-items: stretch; }
  .drop { flex: 1 1 260px; display: grid; place-items: center; padding: 10px; border: 1px dashed var(--border-strong); border-radius: var(--radius); color: var(--quiet); font-size: 13px; }
  .drop.over { border-color: var(--accent); color: var(--text); }
  .error { color: var(--err); font-size: 13px; margin: 0; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
</style>
