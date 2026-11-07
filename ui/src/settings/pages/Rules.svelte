<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import GuideLink from '../guide/GuideLink.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import { layoutsUsingRule, usedIn } from '../usage';
  import type { Layout, Rule, RuleFormat, RuleTest } from '../../lib/types';

  const rules = createItems<Rule>('rule');
  const layouts = createItems<Layout>('layout');
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
    { value: 'text', label: 'Text or log file (.txt, .log)', query: 'tail: 20', help: 'One instruction: line: 3, line: -1 (the last line), lines: 5-9, head: 20, tail: 20, contains: timeout, after: Version:, between: build= | ; (what sits between two texts), or level: error, warn' },
    { value: 'regex', label: 'Text pattern (regex)', query: 'version = "(.+?)"', help: 'A regular expression. The value is the first (group), or the whole match when there is none. Use “Build the value from groups” for more' },
    { value: 'file', label: 'Any file: size, date, lines, fingerprint', query: 'size', help: 'name, size, size: human, modified, modified: date, lines, or sha256 (sha256: short for 12 characters). Works on any file, not only text' },
  ];
  const format = (f: RuleFormat) => FORMATS.find((x) => x.value === f)!;

  onMount(() => Promise.all([rules.reload(), layouts.reload()]));

  const blank = (): Rule => ({
    name: '', file: '', format: 'json', query: '', multiple: 'first', separator: ', ', nth: 1, output: '', fallback: null, max_bytes: 1048576,
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

  // The preview: the rule is tried on a real file a moment after the last change, so the value,
  // the file it came from and the start of that file are always the ones for what is on screen.
  // An answer that arrives late for an older version of the rule is ignored.
  let latest = 0;
  $effect(() => {
    if (!draft) return;
    const rule = $state.snapshot(draft.value) as Rule;
    const useFallback = draft.useFallback;
    const folder = testFolder.trim() || null;
    const mine = ++latest;
    if (!rule.file.trim()) {
      test = null;
      testing = false;
      return;
    }
    testing = true;
    const timer = setTimeout(async () => {
      let result: RuleTest;
      try {
        const api = await getApi();
        result = await api.testRule({ ...rule, fallback: useFallback ? rule.fallback : null }, folder);
      } catch (e) {
        result = { folder, value: null, problem: String(e), file: null, excerpt: null };
      }
      if (mine === latest) {
        test = result;
        testing = false;
      }
    }, 400);
    return () => clearTimeout(timer);
  });

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
  <div class="guide-link"><GuideLink tour="data-from-files" /></div>
  {#if rules.failure}<Chip kind="err" label={rules.failure} />{/if}
  {#each rules.problems as p (p)}<Chip kind="warn" label={`Could not read a file-value file: ${p}`} />{/each}

  {#if rules.loaded && rules.items.length === 0 && !draft}
    <div class="empty">
      <h2>No values from files yet</h2>
      <p class="quiet">
        CargoFrame can read one value out of a file in the project folder, such as the version in
        package.json or the last line of a log, and give it a name. You then use that name in a text
        block, or pick it as the source for a destination field.
      </p>
      <p class="example">
        Example: read <code>version</code> from <code>package.json</code>, call it “version”, and write
        <code>{'{{rule.version}}'}</code> in a text block to have it filled in on every capture.
      </p>
      <button class="primary" data-tour="rules.add" onclick={add}>Add a value from a file</button>
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
        <p class="use">Write <code class="selectable">{`{{rule.${id}}}`}</code> in a text block to use it.</p>
        <p class="use quiet">{usedIn(layoutsUsingRule(layouts.items, id))}</p>
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
    <div><button class="primary" data-tour="rules.add" onclick={add}>Add a value from a file</button></div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? 'Edit value from a file' : 'New value from a file'}</h2>
      <Field label="Name" hint="Also the name you write in a text block, and shown when the value cannot be found." tour="rules.name">
        <input type="text" bind:value={draft.value.name} placeholder="App version" />
      </Field>
      <Field label="File" tour="rules.file" hint="Relative to the project folder. Use * or ** to match several; the first match is read.">
        <input type="text" bind:value={draft.value.file} placeholder={draft.value.format === 'text' ? 'logs/*.log' : 'package.json'} />
      </Field>
      <Field label="Format" tour="rules.format">
        <select bind:value={draft.value.format}>
          {#each FORMATS as f (f.value)}<option value={f.value}>{f.label}</option>{/each}
        </select>
      </Field>
      <Field label="Look for" tour="rules.query" hint={format(draft.value.format).help}>
        <input type="text" bind:value={draft.value.query} placeholder={format(draft.value.format).query} />
      </Field>
      {#if draft.value.format === 'regex'}
        <Field label="Build the value from groups" hint="Optional. $1 and $2 are the pattern's (groups), ${'{'}name{'}'} a named one, $0 the whole match: for example $1.$2. Empty gives the first group.">
          <input type="text" bind:value={draft.value.output} placeholder="$1.$2" />
        </Field>
      {/if}
      {#if draft.value.format !== 'file'}
        <Field label="If there are several values">
          <select bind:value={draft.value.multiple}>
            <option value="first">Use the first</option>
            <option value="last">Use the last</option>
            <option value="nth">Use the one at a number</option>
            <option value="join">Join them all</option>
          </select>
        </Field>
        {#if draft.value.multiple === 'nth'}
          <Field label="Which one" hint="1 is the first, 2 the second; -1 is the last, -2 the one before it.">
            <input type="number" step="1" bind:value={draft.value.nth} aria-label="Which one" />
          </Field>
        {/if}
        {#if draft.value.multiple === 'join'}
          <Field label="Separator" hint="Write \n for a new line and \t for a tab.">
            <input type="text" bind:value={draft.value.separator} />
          </Field>
        {/if}
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

      <div class="test" aria-live="polite">
        <h3>Preview{#if testing}<span class="quiet small"> updating…</span>{/if}</h3>
        <div class="row">
          <input type="text" bind:value={testFolder} placeholder="Project folder (empty: the last capture's)" aria-label="Project folder to test against" />
          <button type="button" onclick={choose}>Choose…</button>
        </div>
        {#if !draft.value.file.trim()}
          <p class="quiet small">Choose a file above to see what this reads from it.</p>
        {:else if test}
          {#if test.problem}
            <Chip kind="warn" label={test.problem} />
          {:else}
            <div class="result"><Chip kind="ok" label="Found" /> <code class="selectable">{test.value}</code></div>
          {/if}
          {#if test.file}
            <p class="quiet small">Reading <span class="mono">{test.file}</span>{#if test.folder} in <span class="mono">{test.folder}</span>{/if}</p>
            {#if test.excerpt}
              <details>
                <summary class="small">Start of the file</summary>
                <pre class="excerpt selectable">{test.excerpt}</pre>
              </details>
            {/if}
          {:else if test.folder}
            <p class="quiet small mono">in {test.folder}</p>
          {/if}
        {/if}
      </div>

      {#if formError}<p class="error" role="alert">{formError}</p>{/if}
      <div class="buttons">
        <button class="primary" type="submit" data-tour="rules.save" disabled={busy}>Save</button>
        <button type="button" onclick={() => (draft = null)}>Cancel</button>
      </div>
    </form>
  {/if}
</div>

<style>
  .guide-link { justify-self: end; }
  .page { display: grid; gap: 12px; }
  .empty, .card, .editor {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px;
  }
  .empty { display: grid; gap: 8px; justify-items: start; }
  .card { display: flex; gap: 16px; justify-content: space-between; flex-wrap: wrap; }
  .main { display: grid; gap: 4px; min-width: 0; }
  .use { font-size: 13px; }
  .example { font-size: 13px; }
  code { font: 12.5px var(--mono); background: var(--surface-2); padding: 1px 6px; border-radius: 5px; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .editor { display: grid; gap: 14px; }
  .check { display: flex; gap: 8px; align-items: center; }
  .test { display: grid; gap: 8px; padding: 12px; background: var(--surface-2); border-radius: var(--radius-sm); }
  .test h3 { font-size: 13px; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; min-width: 0; }
  .result { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .excerpt { margin: 4px 0 0; padding: 8px; max-height: 180px; overflow: auto; font: 12px var(--mono); background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-sm); white-space: pre; }
  .small { font-size: 12px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
</style>
