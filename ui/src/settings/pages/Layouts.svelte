<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { InfoType, IssueView, Layout, Rule, Trigger, VariableInfo } from '../../lib/types';
  import Canvas from '../layout/Canvas.svelte';
  import Properties from '../layout/Properties.svelte';
  import { canPlace, moveTo, removeBlock, resizeTo, tidy } from '../layout/model';
  import { go, route } from '../route.svelte';

  const layouts = createItems<Layout>('layout');
  const triggers = createItems<Trigger>('trigger');
  const rules = createItems<Rule>('rule');
  const infos = createItems<InfoType>('infotype');

  let variables = $state<VariableInfo[]>([]);
  let currentId = $state<string | null>(null);
  let draft = $state<Layout | null>(null);
  let saved = $state('');
  let selected = $state<string | null>(null);
  let issues = $state<IssueView[]>([]);
  let message = $state<{ kind: 'ok' | 'err'; text: string } | null>(null);
  let confirmingDelete = $state(false);
  let busy = $state(false);

  const dirty = $derived(draft !== null && JSON.stringify(tidy($state.snapshot(draft))) !== saved);
  const usedBy = $derived(currentId ? triggers.items.filter((t) => t.item.layout_id === currentId) : []);
  const block = $derived(draft?.blocks.find((b) => b.id === selected) ?? null);

  onMount(async () => {
    await Promise.all([layouts.reload(), triggers.reload(), rules.reload(), infos.reload()]);
    variables = await (await getApi()).listVariables();
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

  async function save() {
    if (!draft || !currentId) return;
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

  function revert() {
    if (currentId) open(currentId);
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
        issues = await (await getApi()).validateLayout(snapshot);
      } catch {
        issues = [];
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
  </div>

  {#if layouts.loaded && layouts.items.length === 0}
    <div class="empty">
      <h2>No layouts yet</h2>
      <p class="quiet">A layout is the window a trigger opens. It starts empty: add the screenshot, text boxes, files, dropdowns and a sharing button you want.</p>
      <button class="primary" onclick={create}>Create your first layout</button>
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
        {#if usedBy.length}
          <span class="quiet used">Used by {usedBy.map((t) => t.item.name).join(', ')}</span>
        {:else}
          <span class="quiet used">Not used by any trigger yet</span>
        {/if}
      </div>
      <div class="actions">
        {#if dirty}<Chip kind="warn" label="Unsaved changes" />{/if}
        <button class="primary" onclick={save} disabled={busy || !dirty}>Save</button>
        <button onclick={revert} disabled={!dirty}>Revert</button>
        {#if confirmingDelete}
          <button class="danger" onclick={remove}>Delete for good</button>
          <button onclick={() => (confirmingDelete = false)}>Keep</button>
        {:else}
          <button onclick={() => (confirmingDelete = true)}>Delete</button>
        {/if}
      </div>
    </div>
    {#if message}<Chip kind={message.kind} label={message.text} />{/if}

    <div class="workspace">
      <Canvas bind:blocks={draft.blocks} bind:selected {issues} />
      <aside class="side" aria-label="Block settings">
        {#if block}
          {#key block.id}
            <Properties
              {block}
              blocks={draft.blocks}
              rules={rules.items}
              infoTypes={infos.items}
              {variables}
              {issues}
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
