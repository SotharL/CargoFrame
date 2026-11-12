<script lang="ts">
  // Data from files: "If this file is found, read these values (and add it to the files)".
  // The file is named by its name or part of it; its kind (JSON, a log…) is worked out from its
  // name, so nobody chooses a format. Each value is used in text as {{rule.<key>}}.
  // A value can also be made from other values ("{{rule.stream}}_{{rule.config}}"), with choices
  // tried from the top; a rule of only such values reads no file.
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import { layoutsUsingRule, usedIn } from '../usage';
  import { blankCombined, blankRule, describe, keysOfOthers, newValue, onlyCombines, slug, unique, valuesOf } from '../ruleValues';
  import type { Layout, Rule, RuleChoice, RuleCondTest, RuleFind, RuleTest, RuleValue, VariableInfo } from '../../lib/types';

  const rules = createItems<Rule>('rule');
  const layouts = createItems<Layout>('layout');
  /** `auto`: the value's variable name follows its name (only for values made in this edit). */
  let draft = $state<{ id: string | null; value: Rule; auto: Record<string, boolean> } | null>(null);
  let formError = $state('');
  let confirming = $state<string | null>(null);
  let busy = $state(false);
  let testFolder = $state('');
  let test = $state<RuleTest | null>(null);
  let testing = $state(false);
  let copied = $state<string | null>(null);

  /** The values a condition can look at. */
  let variables = $state<VariableInfo[]>([]);
  onMount(async () => {
    await Promise.all([rules.reload(), layouts.reload()]);
    variables = await (await getApi()).listVariables().catch(() => []);
  });

  const FINDS: { type: RuleFind['type']; label: string }[] = [
    { type: 'value_of', label: 'the value written after' },
    { type: 'between', label: 'the text between' },
    { type: 'line_with', label: 'the whole line with' },
    { type: 'combine', label: 'made from other values' },
  ];
  const COND_TESTS: { value: RuleCondTest; label: string; needsText: boolean }[] = [
    { value: 'found', label: 'is found', needsText: false },
    { value: 'missing', label: 'is not found', needsText: false },
    { value: 'is', label: 'is', needsText: true },
    { value: 'is_not', label: 'is not', needsText: true },
    { value: 'contains', label: 'contains', needsText: true },
  ];
  const needsText = (t: RuleCondTest) => COND_TESTS.find((x) => x.value === t)?.needsText ?? false;

  /** A rule of values made from other values only, with no file: the editor has no file step. */
  const reads = $derived(!draft || !(onlyCombines(draft.value) && !draft.value.file.trim()));

  function addCombined() {
    formError = '';
    test = null;
    const value = blankCombined(keysOfOthers(rules.items, null));
    draft = { id: null, value, auto: { [value.values![0]!.key]: true } };
  }

  function moveChoice(choices: RuleChoice[], i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= choices.length) return;
    [choices[i], choices[j]] = [choices[j]!, choices[i]!];
  }

  function add() {
    formError = '';
    test = null;
    const value = blankRule();
    const first = newValue(keysOfOthers(rules.items, null));
    value.values = [first];
    draft = { id: null, value, auto: { [first.key]: true } };
  }

  function edit(id: string, r: Rule) {
    formError = '';
    test = null;
    // a rule from before 0.3.0 shows its one value, named as it always was
    const value: Rule = { ...r, several: r.several ?? 'newest', add_to_files: r.add_to_files ?? false, values: valuesOf(id, r).map((v) => ({ ...v })) };
    draft = { id, value, auto: {} };
  }

  /** Variable names already used, by other rules and by this rule's other values. */
  const taken = (except?: RuleValue) => [
    ...keysOfOthers(rules.items, draft?.id ?? null),
    ...(draft?.value.values ?? []).filter((v) => v !== except).map((v) => v.key),
  ];

  function addValue() {
    if (!draft) return;
    const v = newValue(taken());
    // in a rule that reads no file, a new value is made from others too
    if (!reads) v.find = { type: 'combine', choices: [{ text: '' }] };
    draft.value.values = [...(draft.value.values ?? []), v];
    draft.auto[v.key] = true;
  }

  function removeValue(i: number) {
    if (!draft?.value.values) return;
    draft.value.values = draft.value.values.filter((_, n) => n !== i);
  }

  /** A new value's variable follows its name, and "after" looks for that name until changed. */
  function renamed(v: RuleValue, before: string) {
    if (!draft || !draft.auto[v.key]) return;
    const old = v.key;
    v.key = unique(slug(v.name), taken(v));
    delete draft.auto[old];
    draft.auto[v.key] = true;
    if (v.find.type === 'value_of' && (v.find.name === '' || v.find.name === before)) v.find.name = v.name;
  }

  function setFind(v: RuleValue, type: RuleFind['type']) {
    const near = v.find.type === 'value_of' ? v.find.name : v.find.type === 'between' ? v.find.start : v.find.type === 'line_with' ? v.find.text : '';
    v.find =
      type === 'value_of' ? { type, name: near }
      : type === 'between' ? { type, start: near, end: null }
      : type === 'combine' ? { type, choices: [{ text: '' }] }
      : { type: 'line_with', text: near };
  }

  async function save() {
    if (!draft) return;
    busy = true;
    formError = '';
    try {
      const value = $state.snapshot(draft.value) as Rule;
      // a rule from before 0.3.0 keeps its format and query only while its value still uses them
      if (!(value.values ?? []).some((v) => v.find.type === 'advanced')) value.query = '';
      await rules.save(draft.id, value);
      draft = null;
    } catch (e) {
      formError = String(e);
    }
    busy = false;
  }

  // The preview: the rule is tried on a real folder a moment after the last change. An answer that
  // arrives late for an older version of the rule is ignored.
  let latest = 0;
  $effect(() => {
    if (!draft) return;
    const rule = $state.snapshot(draft.value) as Rule;
    const folder = testFolder.trim() || null;
    const mine = ++latest;
    if (!rule.file.trim() && !onlyCombines(rule)) {
      test = null;
      testing = false;
      return;
    }
    testing = true;
    const timer = setTimeout(async () => {
      let result: RuleTest;
      try {
        result = await (await getApi()).testRule(rule, folder);
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

  const found = (key: string) => test?.values?.find((v) => v.key === key);

  async function choose() {
    const picked = await (await getApi()).pickFolder('Choose a project folder to test against');
    if (picked) testFolder = picked;
  }

  async function copy(key: string) {
    try {
      await navigator.clipboard.writeText(`{{rule.${key}}}`);
      copied = key;
      setTimeout(() => (copied = null), 2000);
    } catch {
      // the name is on the page to select by hand
    }
  }

  async function remove(id: string) {
    await rules.remove(id);
    confirming = null;
  }
</script>

<div class="page">
  {#if rules.failure}<Chip kind="err" label={rules.failure} />{/if}
  {#each rules.problems as p (p)}<Chip kind="warn" label={`Could not read a file-value file: ${p}`} />{/each}

  {#if rules.loaded && rules.items.length === 0 && !draft}
    <div class="empty">
      <h2>No data from files yet</h2>
      <p class="quiet">
        When a capture's project folder has a file you name, CargoFrame can read values from it, like a
        machine's graphics card in a system report or the version in package.json, and add the file to the
        ones sent. Each value is a name you write in a title or a description.
      </p>
      <p class="example">
        Example: if <code>system_info</code> is found, read the value after <code>GPU</code> and write
        <code>{'{{rule.gpu}}'}</code> in the description.
      </p>
      <div class="buttons">
        <button class="primary" data-tour="rules.add" onclick={add}>Add data from a file</button>
        <button onclick={addCombined}>Make a value from other values</button>
      </div>
    </div>
  {/if}

  {#each rules.items as { id, item } (id)}
    {@const values = valuesOf(id, item)}
    <article class="card">
      <div class="main">
        <h2>{item.name}</h2>
        {#if onlyCombines(item) && !item.file.trim()}
          <p class="quiet">Made from other values, no file read</p>
        {:else}
          <p class="quiet">
            If <span class="mono">{item.file}</span> is found{#if item.several === 'all'} (all of them){/if}:
            {#if item.add_to_files}add it to the files{#if values.length}, and {/if}{/if}
            {#if values.length}read {values.length === 1 ? 'one value' : `${values.length} values`}{/if}
          </p>
        {/if}
        <ul class="values">
          {#each values as v (v.key)}
            <li><code class="selectable">{`{{rule.${v.key}}}`}</code> <span class="quiet">{v.name}: {describe(v.find, item)}</span></li>
          {/each}
        </ul>
        <p class="use quiet">{usedIn(layoutsUsingRule(layouts.items, id, values.map((v) => v.key)))}</p>
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
    <div class="buttons">
      <button class="primary" data-tour="rules.add" onclick={add}>Add data from a file</button>
      <button onclick={addCombined}>Make a value from other values</button>
    </div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{!reads ? (draft.id ? 'Edit values made from other values' : 'New value made from other values') : draft.id ? 'Edit data from a file' : 'New data from a file'}</h2>
      <Field label="Name" hint="What this is, like Machine config. Shown when something cannot be found." tour="rules.name">
        <input type="text" bind:value={draft.value.name} placeholder="Machine config" />
      </Field>

      {#if reads}
      <section class="step">
        <h3>If this file is found</h3>
        <div class="line" data-tour="rules.file">
          <input
            type="text"
            class="grow"
            bind:value={draft.value.file}
            placeholder="system_info"
            aria-label="File name, or part of it"
          />
          <select bind:value={draft.value.several} aria-label="If several files match">
            <option value="newest">the newest, if several</option>
            <option value="all">all of them, if several</option>
          </select>
        </div>
        <p class="hint quiet">
          Its name or part of it, looked for in every folder of the project: <code>system_info</code>,
          <code>.log</code>, <code>package.json</code>. Or a path: <code>logs/*.log</code>.
        </p>
      </section>
      {/if}

      <section class="step">
        <h3>{reads ? 'Then' : 'Values'}</h3>
        {#if reads}
          <label class="check" data-tour="rules.add_files">
            <input type="checkbox" bind:checked={draft.value.add_to_files} />
            <span>Add it to the files <span class="quiet">(ticked in the window's Files box, ready to send)</span></span>
          </label>
        {/if}

        <div class="list" data-tour="rules.values">
          {#each draft.value.values ?? [] as v, i (i)}
            {@const result = found(v.key)}
            <div class="value">
              <div class="line">
                <span class="word">{v.find.type === 'combine' ? 'Make' : 'Read'}</span>
                <input
                  type="text"
                  class="name"
                  value={v.name}
                  oninput={(e) => { const before = v.name; v.name = e.currentTarget.value; renamed(v, before); }}
                  placeholder="GPU"
                  aria-label="Name of the value"
                />
                <button type="button" class="token" title="Copy, then paste it in a title or a description" onclick={() => copy(v.key)}>
                  {copied === v.key ? 'Copied' : `{{rule.${v.key}}}`}
                </button>
                <button type="button" class="remove" aria-label={`Remove ${v.name || 'this value'}`} onclick={() => removeValue(i)}>×</button>
              </div>
              {#if v.find.type === 'advanced'}
                <p class="quiet small">
                  Made before 0.3.0: <span class="mono">{describe(v.find, draft.value)}</span>.
                  <button type="button" class="link" onclick={() => setFind(v, 'value_of')}>Make it simpler</button>
                </p>
              {:else}
                <div class="line">
                  {#if v.find.type !== 'combine'}
                    <select bind:value={v.which} aria-label="Which one">
                      <option value="first">the first</option>
                      <option value="last">the last</option>
                      <option value="all">all</option>
                    </select>
                  {/if}
                  <select value={v.find.type} onchange={(e) => setFind(v, e.currentTarget.value as RuleFind['type'])} aria-label="How to find it">
                    {#each FINDS as f (f.type)}<option value={f.type}>{f.label}</option>{/each}
                  </select>
                  {#if v.find.type === 'value_of'}
                    <input type="text" class="grow" bind:value={v.find.name} placeholder="GPU" aria-label="The name written before the value" />
                  {:else if v.find.type === 'between'}
                    {@const between = v.find}
                    <input type="text" class="grow" bind:value={between.start} placeholder="GPU:" aria-label="The text before the value" />
                    <span class="word">and</span>
                    {#if between.end === null}
                      <button type="button" class="eol" onclick={() => (between.end = '')} title="Click to stop at a text instead">the end of the line</button>
                    {:else}
                      <input type="text" class="grow" bind:value={between.end} placeholder="]" aria-label="The text after the value" />
                      <button type="button" class="link" onclick={() => (between.end = null)}>the end of the line</button>
                    {/if}
                  {:else if v.find.type === 'line_with'}
                    <input type="text" class="grow" bind:value={v.find.text} placeholder="ERROR" aria-label="A text the line holds" />
                  {/if}
                </div>
                {#if v.find.type === 'combine'}
                  {@const choices = v.find.choices}
                  <div class="choices" role="group" aria-label={`Choices for ${v.name || 'this value'}`}>
                    <p class="hint quiet small">
                      Tried from the top: the first that fits gives the value. A choice with no condition fits
                      when every value it names is found, so put the one with the most values first.
                    </p>
                    {#each choices as c, ci (ci)}
                      <div class="choice">
                        <div class="line">
                          <span class="word">{ci === 0 ? 'Use' : 'else'}</span>
                          <input type="text" class="grow mono" bind:value={c.text} list="rule-variables" placeholder={'{{rule.stream}}_{{rule.change}}_{{rule.config}}'} aria-label={`Choice ${ci + 1}`} />
                          <button type="button" class="tiny" aria-label={`Move choice ${ci + 1} up`} disabled={ci === 0} onclick={() => moveChoice(choices, ci, -1)}>↑</button>
                          <button type="button" class="tiny" aria-label={`Move choice ${ci + 1} down`} disabled={ci === choices.length - 1} onclick={() => moveChoice(choices, ci, 1)}>↓</button>
                          <button type="button" class="tiny" aria-label={`Remove choice ${ci + 1}`} disabled={choices.length === 1} onclick={() => choices.splice(ci, 1)}>×</button>
                        </div>
                        {#each c.when ?? [] as w, wi (wi)}
                          <div class="line cond">
                            <span class="word quiet">{wi === 0 ? 'if' : 'and'}</span>
                            <input type="text" class="name mono" bind:value={w.value} list="rule-variables" placeholder="rule.shelf" aria-label={`Value condition ${wi + 1} of choice ${ci + 1} looks at`} />
                            <select bind:value={w.test} aria-label={`Test of condition ${wi + 1} of choice ${ci + 1}`}>
                              {#each COND_TESTS as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
                            </select>
                            {#if needsText(w.test)}
                              <input type="text" class="grow" bind:value={w.text} placeholder="Release" aria-label={`Text of condition ${wi + 1} of choice ${ci + 1}`} />
                            {/if}
                            <button type="button" class="tiny" aria-label={`Remove condition ${wi + 1} of choice ${ci + 1}`} onclick={() => c.when?.splice(wi, 1)}>×</button>
                          </div>
                        {/each}
                        <div><button type="button" class="link" onclick={() => (c.when = [...(c.when ?? []), { value: '', test: 'found', text: '' }])}>Add a condition</button></div>
                      </div>
                    {/each}
                    <div><button type="button" onclick={() => choices.push({ text: '' })}>Add a choice</button></div>
                  </div>
                {/if}
                {#if v.find.type === 'value_of'}
                  <p class="hint quiet small">Finds <code>GPU: …</code>, <code>GPU = …</code>, or the key GPU in a JSON, YAML, TOML, INI or XML file (<code>build.number</code> reaches inside).</p>
                {/if}
              {/if}
              <div class="line">
                <span class="word quiet">If not found</span>
                <input
                  type="text"
                  class="grow"
                  value={v.fallback ?? ''}
                  oninput={(e) => (v.fallback = e.currentTarget.value === '' ? null : e.currentTarget.value)}
                  placeholder="leave it out"
                  aria-label="Value if not found"
                />
              </div>
              {#if result}
                <div class="result" aria-live="polite">
                  {#if result.value !== null && !result.used_fallback}
                    <Chip kind="ok" label="Found" /> <code class="selectable shown">{result.value}</code>
                  {:else if result.value !== null}
                    <Chip kind="warn" label="Not found, fallback used" /> <code class="selectable">{result.value}</code>
                  {:else}
                    <Chip kind="dbg" label={result.problem ?? 'Not found'} />
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
          <div><button type="button" onclick={addValue}>{reads ? 'Add a value to read' : 'Add a value'}</button></div>
        </div>
      </section>

      <datalist id="rule-variables">
        {#each variables as x (x.name)}<option value={`{{${x.name}}}`}>{x.description}</option>{/each}
      </datalist>

      <div class="test" aria-live="polite">
        <h3>Preview{#if testing}<span class="quiet small updating">updating…</span>{/if}</h3>
        <div class="row">
          <input type="text" bind:value={testFolder} placeholder="Project folder (empty: the last capture's)" aria-label="Project folder to test against" />
          <button type="button" onclick={choose}>Choose…</button>
        </div>
        {#if !reads}
          <p class="quiet small">Made with the values of the last capture and the other data from files.</p>
        {:else if !draft.value.file.trim()}
          <p class="quiet small">Write a file name above to see what is found.</p>
        {:else if test}
          {#if test.problem}<Chip kind="warn" label={test.problem} />{/if}
          {#if test.file}
            <p class="quiet small">
              Found <span class="mono">{test.file}</span>{#if (test.files?.length ?? 0) > 1} (the newest of {test.files?.length}){/if}{#if test.kind}, read as {test.kind}{/if}{#if test.folder} in <span class="mono">{test.folder}</span>{/if}
            </p>
            {#if test.excerpt}
              <details>
                <summary class="small">Start of the file</summary>
                <pre class="excerpt selectable">{test.excerpt}</pre>
              </details>
            {/if}
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
  .page { display: grid; gap: 12px; }
  .empty, .card, .editor {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px;
  }
  .empty { display: grid; gap: 8px; justify-items: start; }
  .card { display: flex; gap: 16px; justify-content: space-between; flex-wrap: wrap; }
  .main { display: grid; gap: 4px; min-width: 0; }
  .values { margin: 0; padding: 0; list-style: none; display: grid; gap: 2px; font-size: 13px; }
  .use { font-size: 13px; }
  .example { font-size: 13px; }
  code { font: 12.5px var(--mono); background: var(--surface-2); padding: 1px 6px; border-radius: 5px; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .editor { display: grid; gap: 14px; }
  .step { display: grid; gap: 8px; padding-left: 12px; border-left: 3px solid var(--accent); }
  .step h3 { font-size: 13px; margin: 0; }
  .line { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .line select { width: auto; }
  .grow { flex: 1; min-width: 120px; }
  .name { width: 140px; }
  .word { font-size: 13px; }
  .hint { font-size: 12.5px; margin: 0; }
  .list { display: grid; gap: 10px; }
  .value { display: grid; gap: 6px; padding: 10px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-2); }
  .token { font: 12px var(--mono); padding: 2px 8px; }
  .remove { margin-left: auto; width: 28px; padding: 0; }
  .eol { font-size: 12.5px; padding: 2px 8px; border-style: dashed; }
  .link { border: 0; background: transparent; color: var(--accent); padding: 0; text-decoration: underline; font-size: 12.5px; }
  .check { display: flex; gap: 8px; align-items: center; font-size: 13px; }
  .test { display: grid; gap: 8px; padding: 12px; background: var(--surface-2); border-radius: var(--radius-sm); }
  .test h3 { font-size: 13px; }
  .updating { margin-left: 6px; font-weight: 400; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; min-width: 0; }
  .result { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .shown { white-space: pre-wrap; }
  .excerpt { margin: 4px 0 0; padding: 8px; max-height: 180px; overflow: auto; font: 12px var(--mono); background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-sm); white-space: pre; }
  .small { font-size: 12px; }
  .choices { display: grid; gap: 6px; }
  .choice { display: grid; gap: 4px; padding: 8px; border: 1px dashed var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); }
  .cond { padding-left: 18px; }
  .tiny { width: 26px; height: 26px; padding: 0; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
</style>
