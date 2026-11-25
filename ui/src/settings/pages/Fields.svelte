<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import Tr from '../../lib/i18n/Tr.svelte';
  // Layouts → Fill fields: a field of a service (Jira's Labels, Priority, a custom field) and
  // the steps that fill it on every share to that service, from values found in files or anything
  // else the capture knows: "If {gpu} is found → add gpu-{gpu}". Sending a block in a
  // field stays on the sharing button in the layout; this page is for what fills a field by itself.
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Combobox from '../../lib/components/Combobox.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { ComboOption, ConnectionView, FillStep, InfoKind, InfoType, Layout, VariableInfo } from '../../lib/types';
  import { token } from '../../lib/valueNames';
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
    { value: 'text', label: t('Text') },
    { value: 'number', label: t('Number') },
    { value: 'flag', label: t('On / off') },
    { value: 'choice', label: t('One choice from a list') },
    { value: 'multi_choice', label: t('Several choices from a list') },
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
  const fieldLabel = (it: InfoType) => connectionOf(it.connection_id)?.describe.fields.find((f) => f.key === it.field)?.label ?? it.field;
  const layoutName = (id: string) => layouts.items.find((l) => l.id === id)?.item.name ?? id;
  const where = (it: InfoType) =>
    (it.layouts ?? []).length === 0
      ? t('In every layout that shares there')
      : t('Only in {layouts}', { layouts: (it.layouts ?? []).map(layoutName).join(', ') });

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
    { value: '', label: t('this field') },
    ...[...variables]
      .sort((a, b) => Number(!a.name.startsWith('rule.')) - Number(!b.name.startsWith('rule.')))
      .map((v) => ({ value: v.name, label: token(v.name), note: v.description, group: v.name.startsWith('rule.') ? t('From files') : t('From the capture') })),
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
  {#each fills.problems as p (p)}<Chip kind="warn" label={t('Could not read a field fill: {problem}', { problem: p })} />{/each}

  {#if fills.loaded && fills.items.length === 0 && !draft}
    <div class="empty">
      <h2>{t('No fields filled yet')}</h2>
      <p class="quiet">
        {t('Fill a field of the service on every share, from what was found in the files or anything the capture knows. For a box of the layout, choose its field on the sharing button instead.')}
      </p>
      <p class="example">
        <Tr text={t('Example: {field}: if {value} is found, add {tag} and add {other}.')}>
          {#snippet field()}<strong>{t('Jira → Labels')}</strong>{/snippet}
          {#snippet value()}<span class="mono">{'{gpu}'}</span>{/snippet}
          {#snippet tag()}<span class="mono">{'gpu-{gpu}'}</span>{/snippet}
          {#snippet other()}<span class="mono">needs-triage</span>{/snippet}
        </Tr>
      </p>
      <button class="primary" data-tour="fields.add" onclick={add}>{t('Fill a field')}</button>
    </div>
  {/if}

  {#each fills.items as { id, item } (id)}
    <article class="card">
      <div class="main">
        <h2>{nameOf(item.connection_id)} → {fieldLabel(item)}</h2>
        {#each stepLines(item) as line, i (i)}<p class="step-line">{line}</p>{:else}
          <p class="quiet">{t('No steps: offered to the layout builder, where a block fills it.')}</p>
        {/each}
        <p class="quiet use">{where(item)}{item.required ? ' · required' : ''}</p>
      </div>
      <div class="actions">
        <button onclick={() => edit(id, item)}>{t('Edit')}</button>
        {#if confirming === id}
          <button class="danger" onclick={() => remove(id)}>{t('Delete for good')}</button>
          <button onclick={() => (confirming = null)}>{t('Keep')}</button>
        {:else}
          <button onclick={() => (confirming = id)}>{t('Delete')}</button>
        {/if}
      </div>
    </article>
  {/each}

  {#if fills.items.length > 0 && !draft}
    <div><button class="primary" data-tour="fields.add" onclick={add}>{t('Fill a field')}</button></div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? t('Edit the field fill') : t('Fill a field')}</h2>
      <div class="sentence">
        <Field label={t('On')} tour="fields.connection" hint={connections.length === 0 ? t('Add a connection first (Connections).') : ''}>
          <select bind:value={draft.value.connection_id} aria-label={t('Service')}>
            <option value="">{t('Choose a service…')}</option>
            {#if draft.value.connection_id && !connectionOf(draft.value.connection_id)}
              <option value={draft.value.connection_id}>{t('{connection_id} (missing)', { connection_id: draft.value.connection_id })}</option>
            {/if}
            {#each connections as c (c.id)}<option value={c.id}>{c.name} ({c.kind_label})</option>{/each}
          </select>
        </Field>
        <Field label={t('Fill')} tour="fields.field" hint={t('A field the service offers, or a custom field\'s key such as customfield_10042.')}>
          <Combobox label={t('Field')} options={knownFields} free bind:value={draft.value.field} placeholder={t('Labels, Priority…')} />
        </Field>
      </div>

      <fieldset class="steps" data-tour="fields.steps">
        <legend>{t('Steps, in order')}</legend>
        {#each draft.steps as step, si (si)}
          <div class="step">
            <div class="row">
              <span class="quiet lead">{t('When')}</span>
              <select
                aria-label={t('When')}
                value={step.when ? 'if' : 'always'}
                onchange={(e) => (step.when = e.currentTarget.value === 'if' ? newWhen() : null)}
              >
                <option value="always">{t('Always')}</option>
                <option value="if">{t('If')}</option>
              </select>
              <div class="test">
              {#if step.when}
                <div class="subject"><Combobox label={t('What to look at')} options={subjects} free bind:value={step.when.subject} placeholder={'{gpu}'} /></div>
                <select aria-label={t('Test')} bind:value={step.when.test}>
                  {#each TESTS as item (item.value)}<option value={item.value}>{step.when.subject === '' && item.value === 'missing' ? 'is empty' : step.when.subject === '' && item.value === 'found' ? 'has a value' : item.label}</option>{/each}
                </select>
                {#if needsText(step.when.test)}<input type="text" aria-label={t('Text')} bind:value={step.when.text} placeholder="text" />{/if}
              {/if}
              </div>
              <button type="button" class="x" aria-label={t('Remove this step')} onclick={() => draft!.steps.splice(si, 1)}>✕</button>
            </div>
            {#each step.then as a, ai (ai)}
              <div class="row">
                <span class="quiet lead">{ai === 0 ? t('then') : t('and')}</span>
                <select aria-label={t('Add or set')} bind:value={a.do}>
                  <option value="add">{t('add')}</option>
                  <option value="set">{t('set it to')}</option>
                </select>
                <input type="text" class="grow" aria-label={t('Text to add')} bind:value={a.text} placeholder={t('gpu-{gpu}')} />
                {#if step.then.length > 1}
                  <button type="button" class="x" aria-label={t('Remove this action')} onclick={() => step.then.splice(ai, 1)}>✕</button>
                {:else}<span></span>{/if}
              </div>
            {/each}
            <button type="button" class="link" onclick={() => step.then.push({ do: 'add', text: '' })}>{t('+ and')}</button>
          </div>
        {/each}
        <div><button type="button" onclick={() => draft!.steps.push(newStep())}>{t('Add a step')}</button></div>
        <p class="quiet hint">
          <Tr text={t('Add puts one more value in a list (a label), a new line in a description, or words at the end of a title. Values such as {value} come from {page}.')}>
            {#snippet value()}{'{gpu}'}{/snippet}
            {#snippet page()}<em>{t('Data parsing')}</em>{/snippet}
          </Tr>
        </p>
      </fieldset>

      <Field label={t('In which layouts')} tour="fields.layouts">
        <div class="where">
          <label class="check"><input type="radio" name="where" checked={!draft.some} onchange={() => (draft!.some = false)} /> {t('Every layout that shares there')}</label>
          <label class="check"><input type="radio" name="where" checked={draft.some} onchange={() => (draft!.some = true)} /> {t('Only these')}</label>
          {#if draft.some}
            <div class="layouts">
              {#each sharingLayouts as l (l.id)}
                <label class="check">
                  <input type="checkbox" checked={(draft.value.layouts ?? []).includes(l.id)} onchange={(e) => toggleLayout(l.id, e.currentTarget.checked)} />
                  {l.item.name}
                </label>
              {:else}
                <p class="quiet">{t('No layout has a sharing button to this service yet.')}</p>
              {/each}
            </div>
          {/if}
        </div>
      </Field>

      <details class="about">
        <summary>{t('About this field (for a custom field)')}</summary>
        <Field label={t('Name')} hint={t('What it is called in CargoFrame. Empty: the field\'s own name.')}>
          <input type="text" bind:value={draft.value.name} placeholder={fieldLabel(draft.value) || 'Priority'} />
        </Field>
        <Field label={t('Kind')}>
          <select bind:value={draft.value.kind} onchange={onKind}>
            {#each KINDS as k (k.value)}<option value={k.value}>{k.label}</option>{/each}
          </select>
        </Field>
        {#if isList(draft.value.kind)}
          <Field label={t('Options come from')}>
            <select value={draft.listKind} onchange={(e) => setListKind(e.currentTarget.value as 'fixed' | 'api')}>
              <option value="fixed">{t('A list I type in')}</option>
              <option value="api">{t('The service\'s API')}</option>
            </select>
          </Field>
          {#if draft.value.options.type === 'fixed'}
            <div class="options">
              {#each draft.value.options.options as o, i (i)}
                <div class="opt">
                  <input type="text" bind:value={o.value} placeholder={t('Value sent')} aria-label={t('Option value')} />
                  <input type="text" bind:value={o.label} placeholder={t('Label shown')} aria-label={t('Option label')} />
                  <button type="button" aria-label={t('Remove option')} onclick={() => draft!.value.options.type === 'fixed' && draft!.value.options.options.splice(i, 1)}>✕</button>
                </div>
              {/each}
              <button type="button" onclick={() => draft!.value.options.type === 'fixed' && draft!.value.options.options.push({ value: '', label: '' })}>{t('Add an option')}</button>
            </div>
          {:else if draft.value.options.type === 'api'}
            <Field label={t('API list')} hint={t('For a list that needs a project key, write list:KEY, for example components:PROJ.')}>
              <Combobox label={t('API list')} options={knownLists} free bind:value={draft.value.options.options_id} placeholder="priorities" />
            </Field>
          {/if}
        {/if}
        <label class="check"><input type="checkbox" bind:checked={draft.value.required} /><span>{t('A value is required before sharing')}</span></label>
      </details>

      {#if formError}<p class="error" role="alert">{formError}</p>{/if}
      <div class="buttons">
        <button class="primary" type="submit" data-tour="fields.save" disabled={busy}>{t('Save')}</button>
        <button type="button" onclick={() => (draft = null)}>{t('Cancel')}</button>
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
  /* The two boxes side by side, on one line whatever hint is under either of them. */
  .sentence { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 12px; align-items: start; }
  .editor select, .editor input[type='text'], .editor :global(.combo input) { box-sizing: border-box; height: 34px; }
  .steps { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px 12px; margin: 0; display: grid; gap: 10px; }
  .steps legend { font-weight: 600; padding: 0 4px; }
  .step { display: grid; gap: 6px; padding: 8px; border-radius: var(--radius-sm); background: color-mix(in srgb, var(--text) 4%, transparent); }
  /* Every row of a step on the same columns: a word, the choice, what it applies to, its ✕. */
  .row { display: grid; grid-template-columns: 44px 120px minmax(0, 1fr) 34px; gap: 6px; align-items: center; }
  .lead { text-align: right; font-size: 13px; }
  .test { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; min-width: 0; }
  .subject { min-width: 160px; flex: 1 1 180px; }
  .test input { flex: 1 1 120px; min-width: 0; }
  .grow { min-width: 0; width: 100%; }
  .x { width: 34px; height: 34px; padding: 0; }
  .link { justify-self: start; background: none; border: none; color: var(--accent); padding: 0 0 0 50px; cursor: pointer; }
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
  @media (max-width: 640px) {
    .sentence { grid-template-columns: 1fr; }
    .row { grid-template-columns: 36px 100px minmax(0, 1fr) 34px; }
  }
</style>
