<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import GuideLink from '../guide/GuideLink.svelte';
  import Field from '../../lib/components/Field.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import ShortcutInput from '../../lib/components/ShortcutInput.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import { keycaps } from '../../lib/shortcut';
  import type { CaptureMode, Layout, Media, Trigger } from '../../lib/types';
  import { go } from '../route.svelte';

  const triggers = createItems<Trigger>('trigger');
  const layouts = createItems<Layout>('layout');
  let errors = $state<Record<string, string>>({});
  let draft = $state<{ id: string | null; value: Trigger } | null>(null);
  let formError = $state('');
  let confirming = $state<string | null>(null);
  let busy = $state(false);

  const MODES: { value: CaptureMode; label: string; hint: string }[] = [
    { value: 'active_window', label: 'The program in focus', hint: 'Falls back to the screen it is on when no window can be read.' },
    { value: 'display', label: 'The screen under the mouse', hint: 'The whole display where the pointer is.' },
    { value: 'all_displays', label: 'All screens', hint: 'Every display as one picture.' },
  ];
  const modeLabel = (m: CaptureMode) => MODES.find((x) => x.value === m)?.label ?? m;

  async function refresh() {
    await Promise.all([triggers.reload(), layouts.reload()]);
    errors = await (await getApi()).getShortcutErrors();
  }
  onMount(refresh);

  const blank = (): Trigger => ({ name: '', enabled: true, shortcut: '', capture: 'active_window', media: 'screenshot', layout_id: null });

  const MEDIA: { value: Media; label: string; hint: string }[] = [
    { value: 'screenshot', label: 'A screenshot', hint: 'One picture, taken when you press the shortcut.' },
    { value: 'video', label: 'A video', hint: 'The first press starts recording the screen, the next one stops it and opens the window with the clip. Needs ffmpeg: see Settings, Video.' },
  ];

  /** What the layout of a trigger has to say about the capture it will get, if anything. */
  function layoutHint(t: Trigger): string | null {
    const layout = t.layout_id ? layouts.find(t.layout_id) : null;
    if (!layout) return null;
    const blocks = layout.blocks;
    if (t.media === 'video' && !blocks.some((b) => b.type === 'video')) return 'This layout has no Video block, so the clip would not show';
    if (t.media === 'screenshot' && blocks.some((b) => b.type === 'video') && !blocks.some((b) => b.type === 'screenshot')) {
      return 'This layout has a Video block but this trigger takes a screenshot';
    }
    return null;
  }

  function add() {
    formError = '';
    draft = { id: null, value: blank() };
  }
  function edit(id: string, t: Trigger) {
    formError = '';
    draft = { id, value: { ...t } };
  }

  async function save() {
    if (!draft) return;
    busy = true;
    formError = '';
    try {
      await triggers.save(draft.id, draft.value);
      errors = await (await getApi()).getShortcutErrors();
      draft = null;
    } catch (e) {
      formError = String(e);
    }
    busy = false;
  }

  async function remove(id: string) {
    await triggers.remove(id);
    confirming = null;
    errors = await (await getApi()).getShortcutErrors();
  }

  async function toggle(id: string, t: Trigger, enabled: boolean) {
    try {
      await triggers.save(id, { ...t, enabled });
      errors = await (await getApi()).getShortcutErrors();
    } catch (e) {
      formError = String(e);
    }
  }

  const layoutName = (id: string | null) => (id ? layouts.find(id)?.name ?? null : null);
</script>

<div class="page">
  <div class="guide-link"><GuideLink tour="first-capture" /></div>
  {#if triggers.failure}<Chip kind="err" label={triggers.failure} />{/if}
  {#each triggers.problems as p (p)}<Chip kind="warn" label={`Could not read a trigger file: ${p}`} />{/each}

  {#if triggers.loaded && triggers.items.length === 0 && !draft}
    <div class="empty">
      <h2>No triggers yet</h2>
      <p class="quiet">A trigger is a shortcut that captures your screen and opens a CargoFrame window with the layout you choose.</p>
      <button class="primary" data-tour="triggers.add" onclick={add}>Add a trigger</button>
    </div>
  {/if}

  {#each triggers.items as { id, item } (id)}
    <article class="card" class:off={!item.enabled}>
      <div class="main">
        <div class="head">
          <h2>{item.name}</h2>
          {#if item.shortcut}
            <span class="keys">{#each keycaps(item.shortcut) as k, i (i)}<kbd>{k}</kbd>{/each}</span>
          {:else}
            <span class="quiet">Tray menu only</span>
          {/if}
        </div>
        <p class="quiet">
          {#if item.media === 'video'}Records a video of the screen{:else}Captures {modeLabel(item.capture).toLowerCase()}{/if}
          · Layout:
          {#if item.layout_id}
            {layoutName(item.layout_id) ?? item.layout_id}
          {:else}
            none (opens an empty window)
          {/if}
        </p>
        <div class="chips">
          {#if errors[id]}<Chip kind="err" label={errors[id]!} />{/if}
          {#if item.layout_id && layouts.loaded && !layouts.find(item.layout_id)}
            <Chip kind="warn" label="This layout no longer exists" />
          {/if}
          {#if item.enabled && !item.layout_id}<Chip kind="dbg" label="No layout chosen" />{/if}
          {#if layouts.loaded && layoutHint(item)}<Chip kind="warn" label={layoutHint(item)!} />{/if}
        </div>
      </div>
      <div class="actions">
        <Toggle label="On" checked={item.enabled} onchange={(v) => toggle(id, item, v)} />
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

  {#if triggers.items.length > 0 && !draft}
    <div><button class="primary" data-tour="triggers.add" onclick={add}>Add a trigger</button></div>
  {/if}

  {#if draft}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? 'Edit trigger' : 'New trigger'}</h2>
      <Field label="Name"><input type="text" bind:value={draft.value.name} placeholder="Bug report" /></Field>
      <div class="field" data-tour="triggers.shortcut">
        <span class="label">Shortcut</span>
        <ShortcutInput bind:value={draft.value.shortcut} label="Shortcut" />
        <p class="quiet hint">Click the box, then press the keys. Leave it empty to start this trigger from the tray menu only.</p>
      </div>
      <Field label="Makes" hint={MEDIA.find((m) => m.value === draft!.value.media)?.hint}>
        <select bind:value={draft.value.media}>
          {#each MEDIA as m (m.value)}<option value={m.value}>{m.label}</option>{/each}
        </select>
      </Field>
      {#if draft.value.media === 'screenshot'}
        <Field label="Capture" hint={MODES.find((m) => m.value === draft!.value.capture)?.hint}>
          <select bind:value={draft.value.capture}>
            {#each MODES as m (m.value)}<option value={m.value}>{m.label}</option>{/each}
          </select>
        </Field>
      {/if}
      <Field label="Layout" hint="The window this trigger opens." tour="triggers.layout">
        <select bind:value={draft.value.layout_id}>
          <option value={null}>None (empty window)</option>
          {#each layouts.items as l (l.id)}<option value={l.id}>{l.item.name}</option>{/each}
        </select>
      </Field>
      <button type="button" class="link" onclick={() => go('layouts')}>Build a layout…</button>
      <Toggle label="Enabled" bind:checked={draft.value.enabled} />
      {#if formError}<p class="error" role="alert">{formError}</p>{/if}
      <div class="buttons">
        <button class="primary" type="submit" data-tour="triggers.save" disabled={busy}>Save</button>
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
  .card.off .main { opacity: 0.6; }
  .main { display: grid; gap: 4px; min-width: 0; }
  .head { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .keys { display: inline-flex; gap: 4px; }
  kbd {
    font: 600 12px var(--mono); padding: 1px 7px; border-radius: 5px;
    background: var(--surface-2); border: 1px solid var(--border-strong); border-bottom-width: 2px;
  }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .editor { display: grid; gap: 14px; }
  .field { display: grid; gap: 4px; }
  .label { font-weight: 600; font-size: 13px; }
  .hint { font-size: 12.5px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  .link { justify-self: start; border: 0; background: transparent; color: var(--accent); padding: 0; text-decoration: underline; }
</style>
