<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { ConnectionView, InfoType, IssueView, Layout, LayoutTemplate, Reach, Rule, Trigger, VariableInfo } from '../../lib/types';
  import Canvas from '../layout/Canvas.svelte';
  import Properties from '../layout/Properties.svelte';
  import { canPlace, moveTo, removeBlock, resizeTo, tidy } from '../layout/model';
  import { offersFor, type TemplateOffer } from '../layout/templates';
  import { go, route } from '../route.svelte';

  const layouts = createItems<Layout>('layout');
  const triggers = createItems<Trigger>('trigger');
  const rules = createItems<Rule>('rule');
  const infos = createItems<InfoType>('infotype');

  let variables = $state<VariableInfo[]>([]);
  let currentId = $state<string | null>(null);
  let draft = $state<Layout | null>(null);
  let connections = $state<ConnectionView[]>([]);
  let saved = $state('');
  let selected = $state<string | null>(null);
  let issues = $state<IssueView[]>([]);
  /** What each sharing button of the layout sends, worked out by the code that does the sharing. */
  let reach = $state<Record<string, Reach>>({});
  let message = $state<{ kind: 'ok' | 'err'; text: string } | null>(null);
  let confirmingDelete = $state(false);
  let busy = $state(false);
  let templates = $state<LayoutTemplate[]>([]);
  let picking = $state(false);
  /** Set while the builder shows a template that has not been saved: saving makes a copy. */
  let fromTemplate = $state<{ name: string } | null>(null);

  const dirty = $derived(draft !== null && JSON.stringify(tidy($state.snapshot(draft))) !== saved);
  const usedBy = $derived(currentId ? triggers.items.filter((t) => t.item.layout_id === currentId) : []);
  const block = $derived(draft?.blocks.find((b) => b.id === selected) ?? null);
  const groups = $derived(offersFor(templates, connections));

  onMount(async () => {
    await Promise.all([layouts.reload(), triggers.reload(), rules.reload(), infos.reload()]);
    variables = await (await getApi()).listVariables();
    connections = (await (await getApi()).listConnections().catch(() => null))?.items ?? [];
    templates = await (await getApi()).listLayoutTemplates().catch(() => []);
    if (route.arg && layouts.find(route.arg)) open(route.arg);
    else if (layouts.items.length === 1) open(layouts.items[0]!.id);
  });

  // Follow "Edit layout" requests that arrive while this page is already open.
  $effect(() => {
    const target = route.arg;
    untrack(() => {
      if (target && target !== currentId && layouts.find(target)) open(target);
    });
  });

  function open(id: string) {
    const l = layouts.find(id);
    if (!l) return;
    currentId = id;
    fromTemplate = null;
    draft = structuredClone($state.snapshot(l));
    draft.id = id;
    saved = JSON.stringify(tidy(draft));
    selected = null;
    message = null;
    confirmingDelete = false;
  }

  async function create() {
    const name = 'New layout';
    const api = await getApi();
    const id = await api.newItemId('layout', name);
    const fresh: Layout = { id, name, blocks: [] };
    try {
      await api.saveItem('layout', id, fresh);
      await layouts.reload();
      open(id);
    } catch (e) {
      message = { kind: 'err', text: String(e) };
    }
  }

  /** Show a template in the builder. Nothing is written until the person saves a copy. */
  async function useTemplate(offer: TemplateOffer) {
    message = null;
    try {
      const layout = await (await getApi()).layoutFromTemplate(offer.key, offer.connectionId);
      currentId = null;
      draft = structuredClone(layout);
      fromTemplate = { name: layout.name };
      saved = '';
      selected = null;
      confirmingDelete = false;
      picking = false;
    } catch (e) {
      message = { kind: 'err', text: String(e) };
    }
  }

  /** Write the layout in the builder as a new file, leaving any existing one as it was saved. */
  async function saveAsNew(name: string) {
    if (!draft) return;
    busy = true;
    message = null;
    try {
      const api = await getApi();
      const out = tidy($state.snapshot(draft));
      out.name = name;
      const id = await api.newItemId('layout', name);
      out.id = id;
      await api.saveItem('layout', id, out);
      await layouts.reload();
      go('layouts', id);
      open(id);
      message = { kind: 'ok', text: fromTemplate ? 'Saved as your own layout' : 'Saved as a copy' };
    } catch (e) {
      message = { kind: 'err', text: String(e) };
    }
    busy = false;
  }

  async function save() {
    if (!draft) return;
    // A template is never changed: saving it makes a layout of the person's own.
    if (fromTemplate) return saveAsNew(draft.name.trim() || fromTemplate.name);
    if (!currentId) return;
    busy = true;
    message = null;
    try {
      const out = tidy($state.snapshot(draft));
      await (await getApi()).saveItem('layout', currentId, out);
      saved = JSON.stringify(out);
      await layouts.reload();
      message = { kind: 'ok', text: 'Saved' };
    } catch (e) {
      message = { kind: 'err', text: String(e) };
    }
    busy = false;
  }

  const saveAsCopy = () => saveAsNew(`${draft?.name.trim() || 'Layout'} (copy)`);

  function revert() {
    if (currentId) open(currentId);
  }

  function discardTemplate() {
    fromTemplate = null;
    draft = null;
    saved = '';
    issues = [];
    reach = {};
  }

  async function remove() {
    if (!currentId) return;
    await layouts.remove(currentId);
    currentId = null;
    draft = null;
    confirmingDelete = false;
    go('layouts');
  }

  function setRect(rect: { x: number; y: number; w: number; h: number }): boolean {
    if (!draft || !block) return false;
    if (!canPlace(draft.blocks, rect, block.id)) return false;
    const moved = moveTo(draft.blocks, block.id, rect.x, rect.y);
    const sized = moved && resizeTo(moved, block.id, rect.w, rect.h);
    if (!sized) return false;
    draft.blocks = sized;
    return true;
  }

  function removeSelected() {
    if (!draft || !selected) return;
    draft.blocks = removeBlock(draft.blocks, selected);
    selected = null;
  }

  // Re-check the layout shortly after each edit; the backend knows about connections and rules.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!draft) return;
    const snapshot = $state.snapshot(draft);
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        const api = await getApi();
        const [found, worked] = await Promise.all([api.validateLayout(snapshot), api.buttonReach(snapshot)]);
        issues = found;
        reach = worked;
      } catch {
        issues = [];
        reach = {};
      }
    }, 150);
    return () => clearTimeout(timer);
  });

  const counts = $derived({
    error: issues.filter((i) => i.severity === 'error').length,
    warning: issues.filter((i) => i.severity === 'warning').length,
    debug: issues.filter((i) => i.severity === 'debug').length,
  });

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's' && draft) {
      e.preventDefault();
      void save();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="page">
  {#if layouts.failure}<Chip kind="err" label={layouts.failure} />{/if}
  {#each layouts.problems as p (p)}<Chip kind="warn" label={`Could not read a layout file: ${p}`} />{/each}

  <div class="bar">
    <label class="pick">
      <span class="quiet">Layout</span>
      <select
        value={currentId ?? ''}
        onchange={(e) => { const v = e.currentTarget.value; if (v) { go('layouts', v); open(v); } }}
        aria-label="Layout to edit"
      >
        <option value="" disabled>{layouts.items.length ? 'Choose a layout…' : 'No layouts yet'}</option>
        {#each layouts.items as l (l.id)}<option value={l.id}>{l.item.name}</option>{/each}
      </select>
    </label>
    <button onclick={create}>New layout</button>
    <button onclick={() => (picking = !picking)} aria-expanded={picking}>
      {picking ? 'Hide templates' : 'Start from a template'}
    </button>
  </div>

  {#if picking}
    <section class="templates" aria-label="Templates">
      <p class="quiet">
        Ready-made windows for the connections you have. A template opens here as an unsaved layout:
        change anything, then save, and your own copy is made. The template itself never changes.
      </p>
      {#each groups as g (g.id)}
        <div class="group">
          <h3>
            {g.title}
            <Chip kind={g.state === 'ready' || g.state === 'local' ? 'ok' : 'warn'} label={g.detail} />
          </h3>
          {#each g.offers as o (o.key)}
            <div class="offer">
              <div>
                <strong>{o.name}</strong>
                <p class="quiet small">{o.description}</p>
              </div>
              <button onclick={() => useTemplate(o)} aria-label={`Use ${o.name}${o.connectionId ? ` for ${g.title}` : ''}`}>Use</button>
            </div>
          {/each}
        </div>
      {/each}
      {#if !connections.length}
        <p class="quiet small">
          Add a connection to see layouts for Jira, Google, Reddit, Claude, Slack or your own API.
          <button class="link" onclick={() => go('connections')}>Go to Connections</button>
        </p>
      {/if}
    </section>
  {/if}

  {#if layouts.loaded && layouts.items.length === 0 && !draft}
    <div class="empty">
      <h2>No layouts yet</h2>
      <p class="quiet">A layout is the window a trigger opens. Start from a template made for your connections, or from an empty one and add the screenshot, text boxes, files, dropdowns and a sharing button you want.</p>
      <div class="buttons">
        <button class="primary" onclick={() => (picking = true)}>Start from a template</button>
        <button onclick={create}>Create an empty layout</button>
      </div>
    </div>
  {/if}

  {#if draft}
    <div class="titlebar">
      <input class="name" type="text" bind:value={draft.name} aria-label="Layout name" />
      <div class="chips">
        {#if counts.error}<Chip kind="err" label={`${counts.error} to fix`} />{/if}
        {#if counts.warning}<Chip kind="warn" label={`${counts.warning} to check`} />{/if}
        {#if counts.debug}<Chip kind="dbg" label={`${counts.debug} unknown`} />{/if}
        {#if !issues.length}<Chip kind="ok" label="No problems" />{/if}
        {#if fromTemplate}
          <span class="quiet used">
            Template “{fromTemplate.name}”: saving makes your own copy, the template stays as it is
          </span>
        {:else if usedBy.length}
          <span class="quiet used">Used by {usedBy.map((t) => t.item.name).join(', ')}</span>
        {:else}
          <span class="quiet used">Not used by any trigger yet</span>
        {/if}
      </div>
      <div class="actions">
        {#if fromTemplate}
          <Chip kind="dbg" label="Not saved yet" />
          <button class="primary" onclick={save} disabled={busy}>Save as my layout</button>
          <button onclick={discardTemplate}>Discard</button>
        {:else}
          {#if dirty}<Chip kind="warn" label="Unsaved changes" />{/if}
          <button class="primary" onclick={save} disabled={busy || !dirty}>Save</button>
          <button onclick={saveAsCopy} disabled={busy}>Save as copy</button>
          <button onclick={revert} disabled={!dirty}>Revert</button>
          {#if confirmingDelete}
            <button class="danger" onclick={remove}>Delete for good</button>
            <button onclick={() => (confirmingDelete = false)}>Keep</button>
          {:else}
            <button onclick={() => (confirmingDelete = true)}>Delete</button>
          {/if}
        {/if}
      </div>
    </div>
    {#if message}<Chip kind={message.kind} label={message.text} />{/if}

    <div class="workspace">
      <Canvas bind:blocks={draft.blocks} bind:selected {issues} {reach} {connections} />
      <aside class="side" aria-label="Block settings">
        {#if block}
          {#key block.id}
            <Properties
              {block}
              blocks={draft.blocks}
              rules={rules.items}
              infoTypes={infos.items}
              {connections}
              {variables}
              {issues}
              {reach}
              onrect={setRect}
              onremove={removeSelected}
            />
          {/key}
        {:else}
          <div class="hint">
            <h2>Nothing selected</h2>
            <p class="quiet">Add blocks from the strip above the grid, drag them where you want them, and drag the corner to resize. Select a block to change what it does.</p>
            {#if issues.length}
              <h3>To look at</h3>
              {#each issues as i, n (n)}
                <p class="issue {i.severity}"><span aria-hidden="true">{i.severity === 'error' ? '✕' : i.severity === 'warning' ? '!' : '?'}</span> {i.message}</p>
              {/each}
            {/if}
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .page { display: grid; gap: 12px; min-width: 0; }
  .bar { display: flex; gap: 12px; align-items: end; flex-wrap: wrap; }
  .pick { display: grid; gap: 2px; }
  .pick select { min-width: 220px; }
  .empty { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px; display: grid; gap: 8px; justify-items: start; }
  .templates { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 14px; display: grid; gap: 12px; }
  .group { display: grid; gap: 6px; }
  .group h3 { font-size: 13.5px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .offer { display: flex; gap: 12px; justify-content: space-between; align-items: center; padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm); }
  .offer p { margin: 2px 0 0; }
  .small { font-size: 12.5px; }
  .buttons { display: flex; gap: 8px; flex-wrap: wrap; }
  .link { border: 0; background: transparent; padding: 0; color: var(--accent); text-decoration: underline; }
  .titlebar { display: grid; gap: 8px; }
  .name { font-size: 16px; font-weight: 600; max-width: 420px; }
  .chips { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .used { font-size: 13px; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .workspace { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 16px; align-items: start; }
  .side { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 14px; position: sticky; top: 0; max-height: calc(100vh - 48px); overflow: auto; }
  .hint { display: grid; gap: 8px; }
  .hint h3 { font-size: 13px; margin-top: 6px; }
  .issue { font-size: 12.5px; padding: 6px 8px; border-radius: var(--radius-sm); border: 1px solid var(--quiet); }
  .issue.error { border-color: var(--err); background: color-mix(in srgb, var(--err) 12%, transparent); }
  .issue.warning { border-color: var(--warn); background: color-mix(in srgb, var(--warn) 12%, transparent); }
  .issue.debug { border-color: var(--dbg); background: color-mix(in srgb, var(--dbg) 12%, transparent); }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  @media (max-width: 1100px) { .workspace { grid-template-columns: 1fr; } .side { position: static; max-height: none; } }
</style>
