<script lang="ts">
  // Data & fields → Fill fields: a field of a service (Jira's Labels, Priority, a custom field) and
  // the steps that fill it on every share to that service, from values found in files or anything
  // else the capture knows: "If {{rule.gpu}} is found → add gpu-{{rule.gpu}}". Sending a block in a
  // field stays on the sharing button in the layout; this page is for what fills a field by itself.
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Combobox from '../../lib/components/Combobox.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { ComboOption, ConnectionView, FillStep, InfoKind, InfoType, Layout, VariableInfo } from '../../lib/types';
  import { TESTS, blankFill, cleaned, needsText, newStep, newWhen, stepLines, stepsOf } from '../fills';

  const fills = createItems<InfoType>('infotype');
  const layouts = createItems<Layout>('layout');
  let connections = $state<ConnectionView[]>([]);
  let variables = $state<VariableInfo[]>([]);
  type Draft = { id: string | null; value: InfoType; steps: FillStep[]; listKind: 'none' | 'fixed' | 'api'; some: boolean };
  let draft = $state<Draft | null>(null);
  let formError = $state('');
  let confirming = $state<string | null>(null);
  let busy = $state(false);

  const KINDS: { value: InfoKind; label: string }[] = [
    { value: 'text', label: 'Text' },
    { value: 'number', label: 'Number' },
    { value: 'flag', label: 'On / off' },
    { value: 'choice', label: 'One choice from a list' },
    { value: 'multi_choice', label: 'Several choices from a list' },
  ];
  const isList = (k: InfoKind) => k === 'choice' || k === 'multi_choice';

  onMount(async () => {
    await Promise.all([fills.reload(), layouts.reload()]);
    const api = await getApi();
    connections = (await api.listConnections().catch(() => null))?.items ?? [];
    variables = await api.listVariables().catch(() => []);
  });
  const connectionOf = (id: string) => connections.find((c) => c.id === id);
  const nameOf = (id: string) => connectionOf(id)?.name ?? id;
  const fieldLabel = (t: InfoType) => connectionOf(t.connection_id)?.describe.fields.find((f) => f.key === t.field)?.label ?? t.field;
  const layoutName = (id: string) => layouts.items.find((l) => l.id === id)?.item.name ?? id;
  const where = (t: InfoType) =>
    (t.layouts ?? []).length === 0 ? 'In every layout that shares there' : `Only in ${(t.layouts ?? []).map(layoutName).join(', ')}`;

  /** The service's own fields, label first, the key as a note. */
  const knownFields = $derived<ComboOption[]>(
    (connectionOf(draft?.value.connection_id ?? '')?.describe.fields ?? [])
      .filter((f) => f.key !== 'issue')
      .map((f) => ({ value: f.key, label: f.label, note: f.key })),
  );
  const knownLists = $derived<ComboOption[]>(
    (connectionOf(draft?.value.connection_id ?? '')?.describe.lists ?? []).map((l) => ({ value: l.id, label: l.label, note: l.id })),
  );
  /** What a step can look at: this field, then the values read from files, then the rest. */
  const subjects = $derived<ComboOption[]>([
    { value: '', label: 'this field' },
    ...[...variables]
      .sort((a, b) => Number(!a.name.startsWith('rule.')) - Number(!b.name.startsWith('rule.')))
      .map((v) => ({ value: v.name, label: `{{${v.name}}}`, note: v.description, group: v.name.startsWith('rule.') ? 'From files' : 'From the capture' })),
  ]);
  /** The layouts that can share to the chosen service. */
  const sharingLayouts = $derived(
    layouts.items.filter((l) => l.item.blocks.some((b) => b.type === 'sharing_button' && b.connection_id === draft?.value.connection_id)),
  );

  function add() {
    formError = '';
    draft = { id: null, value: blankFill(), steps: [newStep()], listKind: 'none', some: false };
  }
  function edit(id: string, t: InfoType) {
    formError = '';
    const v = structuredClone($state.snapshot(t)) as InfoType;
    draft = { id, value: v, steps: stepsOf(v), listKind: v.options.type, some: (v.layouts ?? []).length > 0 };
  }

  function setListKind(k: 'none' | 'fixed' | 'api') {
    if (!draft) return;
    draft.listKind = k;
    draft.value.options =
      k === 'none' ? { type: 'none' } : k === 'fixed' ? { type: 'fixed', options: [{ value: '', label: '' }] } : { type: 'api', options_id: '' };
  }
  function onKind() {
    if (!draft) return;
    if (!isList(draft.value.kind) && draft.listKind !== 'none') setListKind('none');
    if (isList(draft.value.kind) && draft.listKind === 'none') setListKind('fixed');
  }
  function toggleLayout(id: string, on: boolean) {
    if (!draft) return;
    const now = new Set(draft.value.layouts ?? []);
    if (on) now.add(id);
    else now.delete(id);
    draft.value.layouts = [...now];
  }

  async function save() {
    if (!draft) return;
    busy = true;
    formError = '';
    try {
      const v = cleaned($state.snapshot(draft.value) as InfoType, $state.snapshot(draft.steps) as FillStep[]);
      if (!draft.some) v.layouts = [];
      if (!v.name.trim()) v.name = fieldLabel(v) || v.field;
      if (v.options.type === 'fixed') {
        v.options.options = v.options.options
          .map((o) => ({ value: o.value.trim(), label: o.label.trim() || o.value.trim() }))
          .filter((o) => o.value !== '' || o.label !== '');
      }
      await fills.save(draft.id, v);
      draft = null;
    } catch (e) {
      formError = String(e);
    }
    busy = false;
  }

  async function remove(id: string) {
    await fills.remove(id);
    confirming = null;
  }
</script>

<div class="page">
  {#if fills.failure}<Chip kind="err" label={fills.failure} />{/if}
  {#each fills.problems as p (p)}<Chip kind="warn" label={`Could not read a field fill: ${p}`} />{/each}

  {#if fills.loaded && fills.items.length === 0 && !draft}
    <div class="empty">
      <h2>No fields filled yet</h2>
      <p class="quiet">
        Fill a field of the service on every share, from what was found in the files or anything the
        capture knows. For a box of the layout, choose its field on the sharing button instead.
      </p>
      <p class="example">
        Example: <strong>Jira → Labels</strong>: if <span class="mono">{'{{rule.gpu}}'}</span> is found,
        add <span class="mono">gpu-{'{{rule.gpu}}'}</span> and add <span class="mono">needs-triage</span>.
      </p>
      <button class="primary" data-tour="fields.add" onclick={add}>Fill a field</button>
    </div>
  {/if}

  {#each fills.items as { id, item } (id)}
    <article class="card">
      <div class="main">
        <h2>{nameOf(item.connection_id)} → {fieldLabel(item)}</h2>
        {#each stepLines(item) as line, i (i)}<p class="step-line">{line}</p>{:else}
          <p class="quiet">No steps: offered to the layout builder, where a block fills it.</p>
        {/each}
        <p class="quiet use">{where(item)}{item.required ? ' · required' : ''}</p>
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

  {#if fills.items.length > 0 && !draft}
    <div><button class="primary" data-tour="fields.add" onclick={add}>Fill a field</button></div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? 'Edit the field fill' : 'Fill a field'}</h2>
      <div class="sentence">
        <Field label="On" tour="fields.connection" hint={connections.length === 0 ? 'Add a connection in Setup first.' : ''}>
          <select bind:value={draft.value.connection_id} aria-label="Service">
            <option value="">Choose a service…</option>
            {#if draft.value.connection_id && !connectionOf(draft.value.connection_id)}
              <option value={draft.value.connection_id}>{draft.value.connection_id} (missing)</option>
            {/if}
            {#each connections as c (c.id)}<option value={c.id}>{c.name} ({c.kind_label})</option>{/each}
          </select>
        </Field>
        <Field label="Fill" tour="fields.field" hint="A field the service offers, or a custom field's key such as customfield_10042.">
          <Combobox label="Field" options={knownFields} free bind:value={draft.value.field} placeholder="Labels, Priority…" />
        </Field>
      </div>

      <fieldset class="steps" data-tour="fields.steps">
        <legend>Steps, in order</legend>
        {#each draft.steps as step, si (si)}
          <div class="step">
            <div class="row">
              <select
                aria-label="When"
                value={step.when ? 'if' : 'always'}
                onchange={(e) => (step.when = e.currentTarget.value === 'if' ? newWhen() : null)}
              >
                <option value="always">Always</option>
                <option value="if">If</option>
              </select>
              {#if step.when}
                <div class="subject"><Combobox label="What to look at" options={subjects} free bind:value={step.when.subject} placeholder={'{{rule.gpu}}'} /></div>
                <select aria-label="Test" bind:value={step.when.test}>
                  {#each TESTS as t (t.value)}<option value={t.value}>{step.when.subject === '' && t.value === 'missing' ? 'is empty' : step.when.subject === '' && t.value === 'found' ? 'has a value' : t.label}</option>{/each}
                </select>
                {#if needsText(step.when.test)}<input type="text" aria-label="Text" bind:value={step.when.text} placeholder="text" />{/if}
              {/if}
              <button type="button" class="x" aria-label="Remove this step" onclick={() => draft!.steps.splice(si, 1)}>✕</button>
            </div>
            {#each step.then as a, ai (ai)}
              <div class="row then">
                <span class="quiet">{ai === 0 ? 'then' : 'and'}</span>
                <select aria-label="Add or set" bind:value={a.do}>
                  <option value="add">add</option>
                  <option value="set">set it to</option>
                </select>
                <input type="text" class="grow" aria-label="Text to add" bind:value={a.text} placeholder="gpu-{'{{rule.gpu}}'}" />
                {#if step.then.length > 1}
                  <button type="button" class="x" aria-label="Remove this action" onclick={() => step.then.splice(ai, 1)}>✕</button>
                {/if}
              </div>
            {/each}
            <button type="button" class="link" onclick={() => step.then.push({ do: 'add', text: '' })}>+ and</button>
          </div>
        {/each}
        <div><button type="button" onclick={() => draft!.steps.push(newStep())}>Add a step</button></div>
        <p class="quiet hint">
          Add puts one more value in a list (a label), a new line in a description, or words at the end
          of a title. Variables such as {'{{rule.gpu}}'} come from <em>Read from files</em>.
        </p>
      </fieldset>

      <Field label="In which layouts" tour="fields.layouts">
        <div class="where">
          <label class="check"><input type="radio" name="where" checked={!draft.some} onchange={() => (draft!.some = false)} /> Every layout that shares there</label>
          <label class="check"><input type="radio" name="where" checked={draft.some} onchange={() => (draft!.some = true)} /> Only these</label>
          {#if draft.some}
            <div class="layouts">
              {#each sharingLayouts as l (l.id)}
                <label class="check">
                  <input type="checkbox" checked={(draft.value.layouts ?? []).includes(l.id)} onchange={(e) => toggleLayout(l.id, e.currentTarget.checked)} />
                  {l.item.name}
                </label>
              {:else}
                <p class="quiet">No layout has a sharing button to this service yet.</p>
              {/each}
            </div>
          {/if}
        </div>
      </Field>

      <details class="about">
        <summary>About this field (for a custom field)</summary>
        <Field label="Name" hint="What it is called in CargoFrame. Empty: the field's own name.">
          <input type="text" bind:value={draft.value.name} placeholder={fieldLabel(draft.value) || 'Priority'} />
        </Field>
        <Field label="Kind">
          <select bind:value={draft.value.kind} onchange={onKind}>
            {#each KINDS as k (k.value)}<option value={k.value}>{k.label}</option>{/each}
          </select>
        </Field>
        {#if isList(draft.value.kind)}
          <Field label="Options come from">
            <select value={draft.listKind} onchange={(e) => setListKind(e.currentTarget.value as 'fixed' | 'api')}>
              <option value="fixed">A list I type in</option>
              <option value="api">The service's API</option>
            </select>
          </Field>
          {#if draft.value.options.type === 'fixed'}
            <div class="options">
              {#each draft.value.options.options as o, i (i)}
                <div class="opt">
                  <input type="text" bind:value={o.value} placeholder="Value sent" aria-label="Option value" />
                  <input type="text" bind:value={o.label} placeholder="Label shown" aria-label="Option label" />
                  <button type="button" aria-label="Remove option" onclick={() => draft!.value.options.type === 'fixed' && draft!.value.options.options.splice(i, 1)}>✕</button>
                </div>
              {/each}
              <button type="button" onclick={() => draft!.value.options.type === 'fixed' && draft!.value.options.options.push({ value: '', label: '' })}>Add an option</button>
            </div>
          {:else if draft.value.options.type === 'api'}
            <Field label="API list" hint="For a list that needs a project key, write list:KEY, for example components:PROJ.">
              <Combobox label="API list" options={knownLists} free bind:value={draft.value.options.options_id} placeholder="priorities" />
            </Field>
          {/if}
        {/if}
        <label class="check"><input type="checkbox" bind:checked={draft.value.required} /><span>A value is required before sharing</span></label>
      </details>

      {#if formError}<p class="error" role="alert">{formError}</p>{/if}
      <div class="buttons">
        <button class="primary" type="submit" data-tour="fields.save" disabled={busy}>Save</button>
        <button type="button" onclick={() => (draft = null)}>Cancel</button>
      </div>
    </form>
  {/if}
</div>

<style>
  .page { display: grid; gap: 12px; }
  .empty, .card, .editor { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px; }
  .empty { display: grid; gap: 8px; justify-items: start; }
  .card { display: flex; gap: 16px; justify-content: space-between; flex-wrap: wrap; }
  .main { display: grid; gap: 4px; min-width: 0; }
  .step-line { font-size: 13px; margin: 0; overflow-wrap: anywhere; }
  .use, .example { font-size: 13px; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .editor { display: grid; gap: 14px; }
  .sentence { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 12px; }
  .steps { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px 12px; margin: 0; display: grid; gap: 10px; }
  .steps legend { font-weight: 600; padding: 0 4px; }
  .step { display: grid; gap: 6px; padding: 8px; border-radius: var(--radius-sm); background: color-mix(in srgb, var(--text) 4%, transparent); }
  .row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .row.then { padding-left: 16px; }
  .subject { min-width: 180px; flex: 1 1 200px; }
  .grow { flex: 1 1 200px; min-width: 0; }
  .x { padding: 2px 8px; }
  .link { justify-self: start; background: none; border: none; color: var(--accent); padding: 0 0 0 16px; cursor: pointer; }
  .hint { margin: 0; font-size: 12.5px; }
  .where, .layouts { display: grid; gap: 4px; }
  .layouts { padding-left: 22px; }
  .about { display: grid; gap: 10px; }
  .about summary { cursor: pointer; font-weight: 600; margin-bottom: 8px; }
  .check { display: flex; gap: 8px; align-items: center; }
  .options { display: grid; gap: 6px; }
  .opt { display: grid; grid-template-columns: 1fr 1fr auto; gap: 6px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  @media (max-width: 640px) { .sentence { grid-template-columns: 1fr; } }
</style>
