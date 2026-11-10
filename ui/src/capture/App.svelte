<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../lib/components/Chip.svelte';
  import { getApi, type Api } from '../lib/ipc';
  import type { Block, ClipEdit, SessionView, ShareResult } from '../lib/types';
  import ScreenshotBlock from './blocks/ScreenshotBlock.svelte';
  import VideoBlock from './blocks/VideoBlock.svelte';
  import TextBlock from './blocks/TextBlock.svelte';
  import DropdownBlock from './blocks/DropdownBlock.svelte';
  import FilesBlock from './blocks/FilesBlock.svelte';
  import SharingBlock from './blocks/SharingBlock.svelte';
  import { collectShare, promptIsEmpty } from './collect';
  import { initialValues, type FormValues } from './values';
  import { loadDropdowns, type Memory } from './options';
  import { nowhereToPost } from './destination';
  import { recentChoices, type Recent } from '../lib/recent';
  import { rememberChoices, remembers } from './remembering';
  import { marksFor } from './blocks/formatting';
  import ResultPanel from './ResultPanel.svelte';
  import TickBoxes from './TickBoxes.svelte';
  import { boxesFor, initialTicks, isTicked, setTick, type Ticks as TickState } from './ticks';
  import { chatToOpen, pairedWithRecording, willSend } from './willSend';
  import { kindStyle } from '../lib/connector';
  import { afterPaint } from '../lib/paint';
  import { applyAccent, applyTheme } from '../lib/theme';

  let api = $state<Api>();
  let view = $state<SessionView | null>(null);
  let image = $state<Blob | null>(null);
  let values = $state<FormValues>({ texts: {}, dropdowns: {}, deeper: {}, files: {} });
  let imageEdited = $state(false);
  /** The part of the recording to send; `null` sends all of it. */
  let clipEdit = $state<ClipEdit | null>(null);
  let failure = $state<string | null>(null);
  let notice = $state<string | null>(null);
  /** The sharing button being sent, if any; every button waits while one is sending. */
  let sending = $state<string | null>(null);
  let result = $state<ShareResult | null>(null);
  /** The blocks each sharing button sends this time: the boxes under the blocks. */
  let ticks = $state<TickState>({});
  /** The sharing button under the pointer or keyboard focus: the blocks it sends light up. */
  let hovered = $state<string | null>(null);
  /** The Claude chat the last share opened, kept to say "Opened: …" once it is done. */
  let opened = $state<string | null>(null);
  /** The choices made before in each dropdown block (newest first), read when the capture opens. */
  let recents = $state<Record<string, Recent[]>>({});
  let screenshot = $state<ReturnType<typeof ScreenshotBlock>>();
  /** Read the text in a part of the screenshot, on this computer. */
  const readText = (png: Blob, attempt: number) => {
    if (!api) return Promise.reject(new Error('CargoFrame is not ready yet'));
    return api.readText(png, attempt);
  };
  /** With a recording and a screenshot block: the picture taken from it, or `null` while there is none. */
  let still = $state<Blob | null>(null);
  let stillSize = $state({ width: 0, height: 0 });
  /** The picture is the one taken when the recording stopped, which the app already holds. */
  let stillIsPoster = $state(false);
  /** Counts the pictures taken, so the editor starts again on each. */
  let stillKey = $state(0);
  let clip = $state<ReturnType<typeof VideoBlock>>();

  function reportPainted() {
    void api?.reportPainted('capture').catch(() => {});
  }

  async function load() {
    if (!api) return;
    // A window kept ready between captures picks up a theme or accent changed in Settings.
    void api
      .getGeneral()
      .then((g) => {
        applyTheme(g.theme);
        applyAccent(g.accent);
      })
      .catch(() => {});
    try {
      const [v, img] = await Promise.all([api.getSession(), api.getSessionImage()]);
      view = v;
      image = img;
      values = initialValues(v);
      imageEdited = false;
      still = null;
      stillIsPoster = false;
      clipEdit = null;
      failure = null;
      notice = null;
      result = null;
      opened = null;
      hovered = null;
      ticks = initialTicks(v);
      recents = Object.fromEntries(
        (v.layout?.blocks ?? [])
          .filter((b) => b.type === 'dropdown')
          .map((b) => [b.id, recentChoices(v.layout?.id, b.id)]),
      );
      const sessionId = v.session_id;
      // A recording's screenshot block starts empty, or with the picture taken when it stopped.
      const paired = pairedWithRecording(v);
      const atEnd = paired && v.layout?.blocks.some((b) => b.type === 'video' && b.screenshot_at_end === true);
      if (atEnd) {
        still = img;
        stillSize = v.image;
        stillIsPoster = true;
        stillKey++;
      }
      // Without a picture to draw the window is usable as soon as it has drawn.
      if (!v.layout?.blocks.some((b) => b.type === 'screenshot') || (paired && !atEnd)) afterPaint(reportPainted);
      // Lists that come from a service arrive after the window is already usable.
      void loadDropdowns(api, v, () => view?.session_id === sessionId, (blockId, next) => {
        if (!view) return;
        const state = next(view.dropdowns[blockId]!);
        view.dropdowns[blockId] = state;
        values.dropdowns[blockId] = state.selected;
      }, memoryOf);
    } catch (e) {
      failure = String(e);
    }
  }

  const dropdownBlock = (id: string) => {
    const b = view?.layout?.blocks.find((x) => x.id === id);
    return b?.type === 'dropdown' ? b : null;
  };
  function memoryOf(blockId: string): Memory {
    const b = dropdownBlock(blockId);
    return { prefer: b && remembers(b) ? (recents[blockId] ?? []).map((r) => r.value) : [], optional: b?.optional === true };
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
  const paired = $derived(view !== null && pairedWithRecording(view));

  function takeShot(frame: Blob) {
    if (!view?.video) return;
    still = frame;
    stillSize = { width: view.video.width, height: view.video.height };
    stillIsPoster = false;
    imageEdited = false;
    stillKey++;
  }
  /** The layout's columns: more than 12 make it wider, with more side by side. */
  const cols = $derived(Math.min(24, Math.max(12, view?.layout?.columns ?? 12)));
  const rows = $derived(blocks.reduce((m, b) => Math.max(m, b.rect.y + b.rect.h), 0));
  const empty = $derived(view !== null && blocks.length === 0);
  // "This button does not send X" is shown on the block itself, so the header does not repeat it.
  const problems = $derived(view?.issues.filter((i) => i.severity !== 'debug' && i.code !== 'cannot_send') ?? []);
  const debugs = $derived(view?.issues.filter((i) => i.severity === 'debug') ?? []);

  function place(b: Block): string {
    const r = b.rect;
    return `grid-column:${r.x + 1} / span ${r.w};grid-row:${r.y + 1} / span ${r.h};`;
  }

  async function share(button: Extract<Block, { type: 'sharing_button' }>) {
    if (!view || !api || sending) return;
    const request = collectShare(view, button, values, imageEdited, clipEdit, ticks, paired && still !== null);
    result = null;
    opened = null;
    if (promptIsEmpty(request)) {
      notice = 'Write the prompt first.';
      return;
    }
    notice = null;
    sending = button.id;
    try {
      // The edits live in this window; the backend sends them as they are now.
      const edited = imageEdited && screenshot ? await screenshot.exportImage() : null;
      // A picture taken from the recording is only here, so it goes up even when it is not edited.
      await api.setEditedImage(view.session_id, paired ? (still && !stillIsPoster ? (edited ?? still) : edited) : edited);
      // The mark on a recording is drawn here too; the backend lays it over the recording.
      if (view.video) await api.setClipMark(view.session_id, clipEdit?.mark && clip ? await clip.exportMark() : null);
      const chat = chatToOpen(view, button, values);
      result = await api.share(request);
      if (result.status === 'sent' || result.status === 'saved_locally') {
        if (view) rememberChoices(view, values, result.status === 'sent' ? (result.place ?? null) : null);
      }
      if (result.status === 'sent' && chat) opened = chat;
      if (result.status === 'sent' && result.close && !result.url) {
        // Nothing left to use from the result: let the confirmation be read, then go.
        setTimeout(() => void api?.closeCapture(), 1500);
      }
      if (result.status === 'saved_locally' && result.close) {
        setTimeout(() => void api?.closeCapture(), 2500);
      }
    } catch (e) {
      result = { status: 'blocked', reasons: [String(e)] };
    } finally {
      sending = null;
    }
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
      <main class="grid" style:grid-template-columns={`repeat(${cols}, minmax(0, 1fr))`} style:grid-template-rows={`repeat(${rows}, minmax(44px, 1fr))`} style={hovered ? kindStyle(view.sharing[hovered]?.kind ?? null) : ''}>
        {#each blocks as b (b.id)}
          <div class="slot" style={place(b)}>
            <section class="cell" class:lit={hovered !== null && isTicked(ticks, hovered, b.id)}>
              <div class="body">
              {#if b.type === 'screenshot' && paired}
                {#if still}
                  {#key stillKey}<ScreenshotBlock bind:this={screenshot} original={still} size={stillSize} bind:edited={imageEdited} onpainted={reportPainted} reader={readText} />{/key}
                {:else}
                  <p class="quiet videonote">No screenshot yet. Press “Take screenshot” under the recording to keep the picture on screen.</p>
                {/if}
              {:else if b.type === 'screenshot' && image}
                <ScreenshotBlock bind:this={screenshot} original={image} size={view.image} bind:edited={imageEdited} onpainted={reportPainted} reader={readText} />
              {:else if b.type === 'video'}
                {#if view.video}
                  {#key view.session_id}<VideoBlock bind:this={clip} info={view.video} bind:edit={clipEdit} onshot={paired ? takeShot : undefined} />{/key}
                {:else}
                  <p class="quiet videonote">The recording of a video trigger appears here. This capture is a screenshot.</p>
                {/if}
              {:else if b.type === 'text'}
                {@const formats = marksFor(view, b)}
                <TextBlock
                  block={b}
                  bind:value={values.texts[b.id]!}
                  unresolved={view.texts[b.id]?.unresolved ?? []}
                  marks={formats.marks}
                  services={formats.services}
                />
              {:else if b.type === 'dropdown' && view.dropdowns[b.id]}
                <DropdownBlock
                  block={b}
                  info={view.dropdowns[b.id]!}
                  bind:value={values.dropdowns[b.id]!}
                  recent={recents[b.id] ?? []}
                  narrowText={b.narrow_by ? (values.texts[b.narrow_by] ?? '') : ''}
                  sessionId={view.session_id}
                  layoutId={view.layout?.id ?? null}
                  bind:deeper={values.deeper[b.id]}
                />
              {:else if b.type === 'files' && view.files[b.id]}
                <FilesBlock block={b} info={view.files[b.id]!} bind:selected={values.files[b.id]!} />
              {:else if b.type === 'sharing_button'}
                <SharingBlock
                  block={b}
                  enabled={(view.sharing[b.id]?.enabled ?? false) && sending === null}
                  sending={sending === b.id}
                  reasons={view.sharing[b.id]?.reasons ?? []}
                  mode={nowhereToPost(view, b) ? 'save_locally' : (view.sharing[b.id]?.mode ?? 'send')}
                  note={view.sharing[b.id]?.note ?? null}
                  kind={view.sharing[b.id]?.kind ?? null}
                  parts={willSend(view, b, values, imageEdited, clipEdit, ticks, still !== null)}
                  opening={chatToOpen(view, b, values)}
                  lit={hovered === b.id}
                  onshare={() => share(b)}
                  onhover={(on) => (hovered = on ? b.id : hovered === b.id ? null : hovered)}
                />
              {/if}
              </div>
            </section>
            {#if b.type !== 'sharing_button'}
              <TickBoxes
                boxes={boxesFor(view, b.id, ticks)}
                active={hovered}
                onchange={(buttonId, on) => (ticks = setTick(ticks, buttonId, b.id, on))}
                onhover={(buttonId) => (hovered = buttonId)}
              />
            {/if}
          </div>
        {/each}
      </main>
    {/if}
    {#if notice || result}
      <footer>
        {#if notice}<div role="status"><Chip kind="warn" label={notice} /></div>{/if}
        {#if result}
          <ResultPanel
            {result}
            onopenlink={(url) => void api?.openLink(url)}
            onreconnect={() => void api?.openSettings('connections')}
            onclose={() => void api?.closeCapture()}
            {opened}
          />
        {/if}
      </footer>
    {/if}
  {/if}
</div>

<style>
  .videonote { margin: 0; padding: 12px; font-size: 13px; }
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
  /* The slot holds the block and its boxes: the boxes sit on the block's edge, so they must not
     be clipped by the block's own scrolling. */
  .slot { position: relative; min-width: 0; min-height: 0; display: grid; container-type: size; }
  /* A block never paints over its neighbour; the padding keeps focus rings from being clipped. */
  .cell { min-width: 0; min-height: 0; overflow: auto; padding: 3px; margin: -3px; display: grid; grid-template-rows: minmax(0, 1fr); border-radius: var(--radius); }
  .body { min-width: 0; min-height: 0; }
  /* The blocks the hovered button sends. A ring, so layout never shifts. */
  .cell.lit { box-shadow: 0 0 0 2px var(--kind-line, var(--accent)); }
  .center {
    display: grid; place-content: center; justify-items: center; gap: 10px; text-align: center;
    padding: 24px;
  }
  footer { padding: 8px 14px; border-top: 1px solid var(--border); display: grid; gap: 6px; }
</style>
