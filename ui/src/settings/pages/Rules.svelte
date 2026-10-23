<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Rule, RuleFormat, RuleTest } from '../../lib/types';

  const rules = createItems<Rule>('rule');
  let draft = $state<{ id: string | null; value: Rule; useFallback: boolean } | null>(null);
  let formError = $state('');
  let confirming = $state<string | null>(null);
  let busy = $state(false);
  let testFolder = $state('');
  let test = $state<RuleTest | null>(null);
  let testing = $state(false);

  const FORMATS: { value: RuleFormat; label: string; query: string; help: string }[] = [
    { value: 'json', label: 'JSON', query: 'version', help: 'A path: version, build.number, items[0].name, items[*].id' },
    { value: 'yaml', label: 'YAML', query: 'app.version', help: 'A path, like JSON: app.version, services[0].image' },
    { value: 'toml', label: 'TOML', query: 'package.version', help: 'A path, like JSON: package.version, tool.poetry.name' },
    { value: 'ini', label: 'INI', query: 'build.number', help: 'section.key, or just key for lines before the first section' },
    { value: 'csv', label: 'CSV', query: 'name', help: 'A column name from the header row, or a column number (1 is the first)' },
    { value: 'xml', label: 'XML', query: 'project/version', help: 'Element names separated by /, ending in @attribute if you want one. //name finds it at any depth' },
    { value: 'text', label: 'Text or log file (.txt, .log)', query: 'tail: 20', help: 'One instruction: line: 3, line: -1 (the last line), lines: 5-9, head: 20, tail: 20, contains: timeout, after: Version:, or level: error, warn' },
    { value: 'regex', label: 'Text pattern (regex)', query: 'version = "(.+?)"', help: 'A regular expression. The value is the first (group), or the whole match when there is none' },
  ];
  const format = (f: RuleFormat) => FORMATS.find((x) => x.value === f)!;

  onMount(rules.reload);

  const blank = (): Rule => ({
    name: '', file: '', format: 'json', query: '', multiple: 'first', separator: ', ', fallback: null, max_bytes: 1048576,
  });

  function add() {
    formError = '';
    test = null;
    draft = { id: null, value: blank(), useFallback: false };
  }
  function edit(id: string, r: Rule) {
    formError = '';
    test = null;
    draft = { id, value: { ...r }, useFallback: r.fallback !== null };
  }

  async function save() {
    if (!draft) return;
    busy = true;
    formError = '';
    try {
      const value = { ...draft.value, fallback: draft.useFallback ? (draft.value.fallback ?? '') : null };
      await rules.save(draft.id, value);
      draft = null;
    } catch (e) {
      formError = String(e);
    }
    busy = false;
  }

  async function runTest() {
    if (!draft) return;
    testing = true;
    test = null;
    try {
      const api = await getApi();
      test = await api.testRule({ ...draft.value, fallback: draft.useFallback ? draft.value.fallback : null }, testFolder.trim() || null);
    } catch (e) {
      test = { folder: null, value: null, problem: String(e) };
    }
    testing = false;
  }

  async function choose() {
    const picked = await (await getApi()).pickFolder('Choose a project folder to test against');
    if (picked) testFolder = picked;
  }

  async function remove(id: string) {
    await rules.remove(id);
    confirming = null;
  }
</script>

<div class="page">
  {#if rules.failure}<Chip kind="err" label={rules.failure} />{/if}
  {#each rules.problems as p (p)}<Chip kind="warn" label={`Could not read a rule file: ${p}`} />{/each}

  {#if rules.loaded && rules.items.length === 0 && !draft}
    <div class="empty">
      <h2>No data rules yet</h2>
      <p class="quiet">
        A data rule reads one value from a file in the project folder, such as the version in
        package.json, and gives it a name you can use in text blocks as <code>{'{{rule.version}}'}</code>.
      </p>
      <button class="primary" onclick={add}>Add a rule</button>
    </div>
  {/if}

  {#each rules.items as { id, item } (id)}
    <article class="card">
      <div class="main">
        <h2>{item.name}</h2>
        <p class="quiet">
          <span class="mono">{item.file}</span> · {format(item.format).label}
          · <span class="mono">{item.query || 'whole file'}</span>
        </p>
        <p class="use">Use as <code class="selectable">{`{{rule.${id}}}`}</code></p>
      </div>
      <div class="actions">
        <button onclick={() => edit(id, item)}>Edit</button>
        {#if confirming === id}
          <button class="danger" onclick={() => remove(id)}>Delete for good</button>
          <button onclick={() => (confirming = null)}>Keep</button>
        {:else}
          <button onclick={() => (confirming = id)}>Delete</button>
        {/if}
      </div>
    </article>
  {/each}

  {#if rules.items.length > 0 && !draft}
    <div><button class="primary" onclick={add}>Add a rule</button></div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? 'Edit rule' : 'New rule'}</h2>
      <Field label="Name" hint="Shown when the rule cannot find its value.">
        <input type="text" bind:value={draft.value.name} placeholder="App version" />
      </Field>
      <Field label="File" hint="Relative to the project folder. Use * or ** to match several; the first match is read.">
        <input type="text" bind:value={draft.value.file} placeholder={draft.value.format === 'text' ? 'logs/*.log' : 'package.json'} />
      </Field>
      <Field label="Format">
        <select bind:value={draft.value.format}>
          {#each FORMATS as f (f.value)}<option value={f.value}>{f.label}</option>{/each}
        </select>
      </Field>
      <Field label="Look for" hint={format(draft.value.format).help}>
        <input type="text" bind:value={draft.value.query} placeholder={format(draft.value.format).query} />
      </Field>
      <Field label="If there are several values">
        <select bind:value={draft.value.multiple}>
          <option value="first">Use the first</option>
          <option value="last">Use the last</option>
          <option value="join">Join them all</option>
        </select>
      </Field>
      {#if draft.value.multiple === 'join'}
        <Field label="Separator" hint="Write \n for a new line and \t for a tab.">
          <input type="text" bind:value={draft.value.separator} />
        </Field>
      {/if}
      {#if draft.value.format === 'text'}
        <p class="quiet small">
          Several lines at once: choose “Join them all” (for example tail: 20, joined with \n). A file
          larger than the limit is read from its end for tail:, line: -1, contains:, after: and level:,
          and from its start for head:, line: 3 and lines:.
        </p>
      {/if}
      <label class="check">
        <input type="checkbox" bind:checked={draft.useFallback} />
        <span>Use a fallback value when nothing is found</span>
      </label>
      {#if draft.useFallback}
        <Field label="Fallback value">
          <input type="text" value={draft.value.fallback ?? ''} oninput={(e) => (draft!.value.fallback = e.currentTarget.value)} placeholder="unknown" />
        </Field>
      {/if}

      <div class="test">
        <h3>Try it</h3>
        <div class="row">
          <input type="text" bind:value={testFolder} placeholder="Project folder (empty: the last capture's)" aria-label="Project folder to test against" />
          <button type="button" onclick={choose}>Choose…</button>
          <button type="button" onclick={runTest} disabled={testing}>Test</button>
        </div>
        {#if test}
          {#if test.problem}
            <Chip kind="warn" label={test.problem} />
          {:else}
            <div class="result"><Chip kind="ok" label="Found" /> <code class="selectable">{test.value}</code></div>
          {/if}
          {#if test.folder}<p class="quiet small mono">in {test.folder}</p>{/if}
        {/if}
      </div>

      {#if formError}<p class="error" role="alert">{formError}</p>{/if}
      <div class="buttons">
        <button class="primary" type="submit" disabled={busy}>Save</button>
        <button type="button" onclick={() => (draft = null)}>Cancel</button>
      </div>
    </form>
  {/if}
</div>

<style>
  .page { display: grid; gap: 12px; }
  .empty, .card, .editor {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px;
  }
  .empty { display: grid; gap: 8px; justify-items: start; }
  .card { display: flex; gap: 16px; justify-content: space-between; flex-wrap: wrap; }
  .main { display: grid; gap: 4px; min-width: 0; }
  .use { font-size: 13px; }
  code { font: 12.5px var(--mono); background: var(--surface-2); padding: 1px 6px; border-radius: 5px; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .editor { display: grid; gap: 14px; }
  .check { display: flex; gap: 8px; align-items: center; }
  .test { display: grid; gap: 8px; padding: 12px; background: var(--surface-2); border-radius: var(--radius-sm); }
  .test h3 { font-size: 13px; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; min-width: 0; }
  .result { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .small { font-size: 12px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
</style>
