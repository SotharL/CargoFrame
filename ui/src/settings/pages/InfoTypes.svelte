<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { ConnectionView, InfoKind, InfoType } from '../../lib/types';

  const infos = createItems<InfoType>('infotype');
  let connections = $state<ConnectionView[]>([]);
  let draft = $state<{ id: string | null; value: InfoType; listKind: 'none' | 'fixed' | 'api' } | null>(null);
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
  const kindLabel = (k: InfoKind) => KINDS.find((x) => x.value === k)?.label ?? k;
  const isList = (k: InfoKind) => k === 'choice' || k === 'multi_choice';

  onMount(async () => {
    await infos.reload();
    connections = (await (await getApi()).listConnections().catch(() => null))?.items ?? [];
  });
  const connectionOf = (id: string) => connections.find((c) => c.id === id);
  const nameOf = (id: string) => connectionOf(id)?.name ?? id;

  const blank = (): InfoType => ({
    name: '', connection_id: '', field: '', kind: 'text', options: { type: 'none' }, required: false, default_template: '',
  });

  function add() {
    formError = '';
    draft = { id: null, value: blank(), listKind: 'none' };
  }
  function edit(id: string, t: InfoType) {
    formError = '';
    draft = { id, value: structuredClone($state.snapshot(t)), listKind: t.options.type };
  }

  /** Keep the stored options in step with the "where do the options come from" choice. */
  function setListKind(k: 'none' | 'fixed' | 'api') {
    if (!draft) return;
    draft.listKind = k;
    draft.value.options =
      k === 'none' ? { type: 'none' }
      : k === 'fixed' ? { type: 'fixed', options: [{ value: '', label: '' }] }
      : { type: 'api', options_id: '' };
  }

  function onKind() {
    if (!draft) return;
    if (!isList(draft.value.kind) && draft.listKind !== 'none') setListKind('none');
    if (isList(draft.value.kind) && draft.listKind === 'none') setListKind('fixed');
  }

  async function save() {
    if (!draft) return;
    busy = true;
    formError = '';
    try {
      const v = $state.snapshot(draft.value);
      if (v.options.type === 'fixed') {
        v.options.options = v.options.options
          .map((o) => ({ value: o.value.trim(), label: o.label.trim() || o.value.trim() }))
          .filter((o) => o.value !== '' || o.label !== '');
      }
      await infos.save(draft.id, v);
      draft = null;
    } catch (e) {
      formError = String(e);
    }
    busy = false;
  }

  async function remove(id: string) {
    await infos.remove(id);
    confirming = null;
  }
</script>

<div class="page">
  {#if infos.failure}<Chip kind="err" label={infos.failure} />{/if}
  {#each infos.problems as p (p)}<Chip kind="warn" label={`Could not read an info type file: ${p}`} />{/each}

  {#if infos.loaded && infos.items.length === 0 && !draft}
    <div class="empty">
      <h2>No info types yet</h2>
      <p class="quiet">
        An info type describes one field a service accepts, such as Jira's Priority or a custom
        field, so a sharing button can fill it from a text block, a data rule or a fixed value.
      </p>
      <button class="primary" onclick={add}>Add an info type</button>
    </div>
  {/if}

  {#each infos.items as { id, item } (id)}
    <article class="card">
      <div class="main">
        <h2>{item.name}</h2>
        <p class="quiet">
          {nameOf(item.connection_id)} → <span class="mono">{item.field}</span>
          · {kindLabel(item.kind)}{item.required ? ' · required' : ''}
        </p>
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

  {#if infos.items.length > 0 && !draft}
    <div><button class="primary" onclick={add}>Add an info type</button></div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? 'Edit info type' : 'New info type'}</h2>
      <Field label="Name"><input type="text" bind:value={draft.value.name} placeholder="Priority" /></Field>
      <Field label="Connection" hint={connections.length === 0 ? 'Add a connection on the Connections page first.' : ''}>
        <select bind:value={draft.value.connection_id}>
          <option value="">Choose…</option>
          {#if draft.value.connection_id && !connectionOf(draft.value.connection_id)}
            <option value={draft.value.connection_id}>{draft.value.connection_id} (missing)</option>
          {/if}
          {#each connections as c (c.id)}<option value={c.id}>{c.name} ({c.kind_label})</option>{/each}
        </select>
      </Field>
      <Field label="Field key" hint="The field's key in that service's API, such as priority or customfield_10042. Pick one the service offers, or type your own.">
        <input type="text" bind:value={draft.value.field} placeholder="priority" list="known-fields" />
        <datalist id="known-fields">
          {#each connectionOf(draft.value.connection_id)?.describe.fields ?? [] as f (f.key)}
            <option value={f.key}>{f.label}</option>
          {/each}
        </datalist>
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
          <Field label="API list" hint="The list the service supplies. For a list that needs a project key, write list:KEY, for example components:PROJ.">
            <input type="text" bind:value={draft.value.options.options_id} placeholder="priorities" list="known-lists" />
            <datalist id="known-lists">
              {#each connectionOf(draft.value.connection_id)?.describe.lists ?? [] as l (l.id)}
                <option value={l.id}>{l.label}</option>
              {/each}
            </datalist>
          </Field>
        {/if}
      {/if}

      <Field label="Starting value" hint="Optional. May use variables such as {'{{project.name}}'}.">
        <input type="text" bind:value={draft.value.default_template} />
      </Field>
      <label class="check"><input type="checkbox" bind:checked={draft.value.required} /><span>A value is required before sharing</span></label>

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
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .editor { display: grid; gap: 14px; }
  .check { display: flex; gap: 8px; align-items: center; }
  .options { display: grid; gap: 6px; }
  .opt { display: grid; grid-template-columns: 1fr 1fr auto; gap: 6px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
</style>
