<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../lib/components/Chip.svelte';
  import { getApi, type Api } from '../lib/ipc';
  import type { Block, SessionView } from '../lib/types';
  import ScreenshotBlock from './blocks/ScreenshotBlock.svelte';
  import TextBlock from './blocks/TextBlock.svelte';
  import DropdownBlock from './blocks/DropdownBlock.svelte';
  import FilesBlock from './blocks/FilesBlock.svelte';
  import SharingBlock from './blocks/SharingBlock.svelte';
  import { collectShare, promptIsEmpty } from './collect';
  import { initialValues, type FormValues } from './values';

  let api = $state<Api>();
  let view = $state<SessionView | null>(null);
  let image = $state<Blob | null>(null);
  let values = $state<FormValues>({ texts: {}, dropdowns: {}, files: {} });
  let imageEdited = $state(false);
  let failure = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let screenshot = $state<ReturnType<typeof ScreenshotBlock>>();

  async function load() {
    if (!api) return;
    try {
      const [v, img] = await Promise.all([api.getSession(), api.getSessionImage()]);
      view = v;
      image = img;
      values = initialValues(v);
      imageEdited = false;
      failure = null;
      notice = null;
    } catch (e) {
      failure = String(e);
    }
  }

  onMount(() => {
    let stop: (() => void) | undefined;
    getApi().then(async (a) => {
      api = a;
      await load();
      stop = await a.onSessionChanged(() => void load());
    });
    return () => stop?.();
  });

  const blocks = $derived(view?.layout?.blocks ?? []);
  const rows = $derived(blocks.reduce((m, b) => Math.max(m, b.rect.y + b.rect.h), 0));
  const empty = $derived(view !== null && blocks.length === 0);
  const problems = $derived(view?.issues.filter((i) => i.severity !== 'debug') ?? []);
  const debugs = $derived(view?.issues.filter((i) => i.severity === 'debug') ?? []);

  function place(b: Block): string {
    const r = b.rect;
    return `grid-column:${r.x + 1} / span ${r.w};grid-row:${r.y + 1} / span ${r.h};`;
  }

  async function share(button: Extract<Block, { type: 'sharing_button' }>) {
    if (!view) return;
    const request = collectShare(view, button, values, imageEdited);
    if (promptIsEmpty(request)) {
      notice = 'Write the prompt first.';
      return;
    }
    // Delivery is added with the connectors; until a connection exists the button stays disabled.
    notice = `Ready to share to ${request.connection_id ?? 'nowhere'}.`;
  }

  function onKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    const typing = target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName);
    if (e.key === 'Escape' && !typing) void api?.closeCapture();
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'w') {
      e.preventDefault();
      void api?.closeCapture();
    }
  }

  const subjectLabel = $derived(
    view?.subject === 'window' ? 'Window' : view?.subject === 'display' ? 'Display' : 'All displays',
  );
</script>

<svelte:window onkeydown={onKey} />

<div class="frame">
  {#if failure}
    <main class="center">
      <h1>CargoFrame could not open this capture</h1>
      <p class="quiet selectable">{failure}</p>
      <button onclick={() => api?.closeCapture()}>Close</button>
    </main>
  {:else if !view}
    <main class="center"><p class="quiet">Loading…</p></main>
  {:else}
    <header>
      <div class="title">
        <h1>{view.trigger_name}</h1>
        <span class="chips">
          <Chip kind="ok" label={subjectLabel} />
          {#if view.app}<span class="meta selectable">{view.app.exe_name ?? view.app.title}</span>{/if}
          {#if view.project}<span class="meta selectable" title={view.project.how}>Project: {view.project.name}</span>{/if}
        </span>
      </div>
      <div class="status">
        {#each view.notes as n (n)}<Chip kind="warn" label={n} />{/each}
        {#each problems as p, i (i)}
          <Chip kind={p.severity === 'error' ? 'err' : 'warn'} label={p.message} />
        {/each}
        {#each debugs as p, i (i)}<Chip kind="dbg" label={p.message} />{/each}
        {#if view.layout_problem}<Chip kind="err" label={view.layout_problem} />{/if}
      </div>
    </header>

    {#if empty}
      <main class="center">
        <h2>This window is empty</h2>
        <p class="quiet">Add blocks to the layout for “{view.trigger_name}” to choose what appears here.</p>
        <button class="primary" onclick={() => api?.editLayout(view?.layout?.id ?? null)}>Edit layout</button>
      </main>
    {:else}
      <main class="grid" style:grid-template-rows={`repeat(${rows}, minmax(44px, 1fr))`}>
        {#each blocks as b (b.id)}
          <section style={place(b)} class="cell">
            {#if b.type === 'screenshot' && image}
              <ScreenshotBlock bind:this={screenshot} original={image} size={view.image} bind:edited={imageEdited} />
            {:else if b.type === 'text'}
              <TextBlock block={b} bind:value={values.texts[b.id]!} unresolved={view.texts[b.id]?.unresolved ?? []} />
            {:else if b.type === 'dropdown' && view.dropdowns[b.id]}
              <DropdownBlock block={b} info={view.dropdowns[b.id]!} bind:value={values.dropdowns[b.id]!} />
            {:else if b.type === 'files' && view.files[b.id]}
              <FilesBlock block={b} info={view.files[b.id]!} bind:selected={values.files[b.id]!} />
            {:else if b.type === 'sharing_button'}
              <SharingBlock
                block={b}
                enabled={view.sharing[b.id]?.enabled ?? false}
                reasons={view.sharing[b.id]?.reasons ?? []}
                onshare={() => share(b)}
              />
            {/if}
          </section>
        {/each}
      </main>
    {/if}
    {#if notice}<footer role="status"><Chip kind="warn" label={notice} /></footer>{/if}
  {/if}
</div>

<style>
  .frame { height: 100%; display: grid; grid-template-rows: auto 1fr auto; min-height: 0; }
  header {
    display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px 16px;
    padding: 10px 14px; border-bottom: 1px solid var(--border); background: var(--surface);
  }
  .title { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .chips { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .meta { color: var(--quiet); font-size: 12.5px; }
  .status { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; }
  .grid {
    display: grid; grid-template-columns: repeat(12, minmax(0, 1fr)); gap: 10px;
    padding: 12px 14px; overflow: auto; min-height: 0;
  }
  /* A block never paints over its neighbour; the padding keeps focus rings from being clipped. */
  .cell { min-width: 0; min-height: 0; overflow: auto; padding: 3px; margin: -3px; }
  .center {
    display: grid; place-content: center; justify-items: center; gap: 10px; text-align: center;
    padding: 24px;
  }
  footer { padding: 8px 14px; border-top: 1px solid var(--border); }
</style>
