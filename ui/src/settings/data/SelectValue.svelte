<script lang="ts">
  import { t, tn } from '../../lib/i18n/index.svelte';
  // New value: select it in a file. The selection suggests how to find it again (in the lines with
  // the label before it, up to what follows it); the person can change each part and sees at once
  // what each file gives. Saving adds the value to the rule of each ticked file (values.ts).
  import { onMount, untrack } from 'svelte';
  import type { RuleAcross, RuleValue, RuleWhich } from '../../lib/types';
  import Choices from './Choices.svelte';
  import FileViewer, { type Sel } from './FileViewer.svelte';
  import { baseName, captureFiles, changedAt, readCaptureFile, readDropped, type ViewFile } from './files';
  import { after, choose, hits, labelBefore, signAfter, snap, suggest, valueOf, type Hit, type Pick } from './pick';
  import { keyOf, nameProblem, type Source } from './values';

  export type Mode =
    | { kind: 'new' }
    /** Read a value that exists in another file too. */
    | { kind: 'add'; key: string; name: string; across: RuleAcross; fallback: string | null }
    /** Find a value of one rule another way. */
    | { kind: 'change'; source: Source; across: RuleAcross };

  let {
    mode,
    taken,
    initial = [],
    onsave,
    oncancel,
  }: {
    mode: Mode;
    /** The keys of the other values. */
    taken: string[];
    /** Files dropped on the page, shown first. */
    initial?: File[];
    /** The value, and the names of the files to read it in (`replace`: the rule whose step it replaces). */
    onsave: (step: RuleValue, files: string[], replace: string | null) => Promise<void>;
    oncancel: () => void;
  } = $props();

  // the mode does not change while the editor is open
  const fixed = untrack(() => mode);
  const fixedKey = fixed.kind === 'add' ? fixed.key : fixed.kind === 'change' ? fixed.source.step.key : null;
  const fixedFile = fixed.kind === 'change' ? fixed.source.rule.file.trim() : null;

  let files = $state<ViewFile[]>([]);
  let current = $state('');
  let loaded = $state(false);
  let sel = $state<Sel | null>(null);
  let find = $state<Pick | null>(null);
  let which = $state<RuleWhich>(fixed.kind === 'change' ? fixed.source.step.which : 'last');
  let across = $state<RuleAcross>(fixed.kind === 'new' ? 'newest' : fixed.across);
  let name = $state(fixed.kind === 'add' ? fixed.name : fixed.kind === 'change' ? fixed.source.step.name : '');
  /** Files ticked or unticked by hand; the others follow what they give. */
  let ticks = $state<Record<string, boolean>>({});
  let editing = $state<'anchor' | 'stop' | null>(null);
  let busy = $state(false);
  let failure = $state('');
  let viewer = $state<ReturnType<typeof FileViewer> | null>(null);

  onMount(async () => {
    const got = await captureFiles();
    files = got.files;
    // when changing a value, its file first
    const mine = fixedFile ? files.find((f) => f.name === fixedFile) : undefined;
    current = mine?.id ?? files[0]?.id ?? '';
    loaded = true;
    if (initial.length) await open(initial);
    if (current) await load(current);
    if (fixed.kind === 'change' && fixed.source.step.find.type === 'pick') startFrom(fixed.source.step.find);
  });

  /** Read a file of the capture if it is not yet. */
  async function load(id: string) {
    const f = files.find((x) => x.id === id);
    if (!f || f.lines !== null) return;
    const read = await readCaptureFile(f);
    files = files.map((x) => (x.id === id ? read : x));
  }

  /** Once something is selected, the other files are read too, to show what each gives. */
  async function loadOthers() {
    for (const f of files.slice(0, 12)) if (f.lines === null) await load(f.id);
  }

  $effect(() => {
    if (current) void load(current);
  });

  /** A value of an existing rule: selected where its file gives it. */
  function startFrom(p: Pick) {
    find = { ...p };
    const f = files.find((x) => x.id === current);
    const kept = f?.lines ? choose(hits(f.lines, p), which) : [];
    const h = kept[kept.length - 1];
    if (f && h) {
      sel = { file: f.id, line: h.line, s: h.s, e: h.e };
      setTimeout(() => viewer?.reveal(h.line, h.s), 30);
    }
    void loadOthers();
  }

  async function open(list: File[]) {
    const read = await Promise.all(list.map(readDropped));
    files = [...read, ...files.filter((f) => !read.some((r) => r.id === f.id))];
    current = read[0]!.id;
  }

  const fileOf = (id: string | undefined) => files.find((f) => f.id === id);
  const selLine = $derived(sel ? (fileOf(sel.file)?.lines?.[sel.line] ?? '') : '');
  const selText = $derived(sel ? selLine.slice(sel.s, sel.e) : '');

  function onselect(file: string, line: number, s: number, e: number) {
    const text = fileOf(file)?.lines?.[line] ?? '';
    if (editing && sel && find) {
      if (file !== sel.file || line !== sel.line) return;
      const p = snap(text, s, e, true);
      if (!p) return;
      const t = text.slice(p.s, p.e);
      if (editing === 'anchor' && p.e <= sel.s) find = { ...find, lines_with: t };
      else if (editing === 'anchor') find = { ...find, lines_with: t, from_line_start: true };
      else if (p.s >= sel.e) find = { ...find, stop: t };
      else return;
      editing = null;
      return;
    }
    const p = snap(text, s, e);
    if (!p) return;
    sel = { file, line, s: p.s, e: p.e };
    find = suggest(text, p.s, p.e);
    ticks = {};
    if (fixed.kind === 'new') name = labelBefore(text, p.s).replace(/[^\p{L}\p{N}_ ]+/gu, ' ').trim();
    void loadOthers();
  }

  // what the shown file gives, for the marks
  const shownFile = $derived(fileOf(current));
  const found = $derived(find && shownFile?.lines ? hits(shownFile.lines, find) : []);
  const kept = $derived(choose(found, which));

  // the choices, made from what the selection has around it
  const q = (t: string) => `“${t}”`;
  const startOptions = $derived([
    { value: 'after', label: find?.lines_with ? t('right after {q}', { q: q(find.lines_with) }) : t('right after a text'), disabled: !find?.lines_with },
    { value: 'line', label: t('at the start of the line') },
  ]);
  const stopOptions = $derived.by(() => {
    if (!sel) return [];
    const sign = signAfter(selLine, sel.e);
    const out: { value: string; label: string }[] = [];
    if (sign) out.push({ value: sign, label: t('at the next {q}', { q: q(sign) }) });
    if (after(selLine, sel.e) === 'space') out.push({ value: ' ', label: t('at the next space') });
    out.push({ value: '', label: t('at the end of the line') });
    const stop = find?.stop ?? '';
    if (stop && !out.some((o) => o.value === stop)) out.push({ value: stop, label: `at ${q(stop)}` });
    return out;
  });
  const WHICH: { value: RuleWhich; label: string }[] = [
    { value: 'last', label: t('the most recent line') },
    { value: 'first', label: t('the first line') },
    { value: 'all', label: 'all' },
  ];
  const ACROSS: { value: RuleAcross; label: string }[] = [
    { value: 'newest', label: t('the newest file') },
    { value: 'all', label: t('all, when they differ') },
  ];

  // the files it is read in, and what it gives
  type Read = { f: ViewFile; got: Hit[] | null; on: boolean; locked: boolean };
  const reads = $derived.by((): Read[] =>
    files.map((f) => {
      const got = find && f.lines ? choose(hits(f.lines, find), which) : null;
      const locked = fixedFile !== null && f.name === fixedFile;
      const auto = fixed.kind === 'new' ? (got?.length ?? 0) > 0 || sel?.file === f.id : sel?.file === f.id;
      return { f, got, locked, on: locked || (ticks[f.id] ?? auto) };
    }),
  );
  const having = $derived(reads.filter((r) => r.on && r.got && r.got.length > 0).sort((a, b) => b.f.modified_ms - a.f.modified_ms));
  const distinct = $derived([...new Set(having.map((r) => valueOf(r.got!)))]);
  const used = (r: Read) => !!sel && r.on && !!r.got?.length && (across === 'all' || r === having[0]);
  const gives = $derived(!having.length ? '' : across === 'all' ? distinct.join(', ') : valueOf(having[0]!.got!));
  const why = $derived(
    !having.length
      ? t('no ticked file has it')
      : across === 'all'
        ? distinct.length > 1 ? t('every different value, the newest file first') : t('the files agree')
        : having.length > 1 ? t('from {file}, changed last', { file: having[0]!.f.name }) : t('from {file}', { file: having[0]!.f.name }),
  );
  const summary = $derived(
    !shownFile || !find
      ? ''
      : kept.length
        ? found.length === 0
          ? t('In {file}: nothing found, gives {value}.', { file: shownFile.name, value: q(valueOf(kept)) })
          : tn('In {file}: found on {n} line, gives {value}.', 'In {file}: found on {n} lines, gives {value}.', found.length, { file: shownFile.name, value: q(valueOf(kept)) })
        : found.length === 0
          ? t('In {file}: nothing found.', { file: shownFile.name })
          : tn('In {file}: found on {n} line.', 'In {file}: found on {n} lines.', found.length, { file: shownFile.name }),
  );

  const key = $derived(fixedKey ?? keyOf(name));
  const problem = $derived(fixedKey ? '' : nameProblem(name, taken));
  const ticked = $derived([...new Set(reads.filter((r) => r.on).map((r) => r.f.name))]);
  const canSave = $derived(!!sel && !!find && !problem && ticked.length > 0 && !(find && !find.lines_with && !find.from_line_start));

  const editHint = $derived(
    !editing || !sel ? '' : editing === 'anchor'
      ? t('On line {n}, select the text a line must have, before the value.', { n: sel.line + 1 })
      : t('On line {n}, select the text the value stops at.', { n: sel.line + 1 }),
  );

  function edit(what: 'anchor' | 'stop') {
    if (!sel) return;
    editing = what;
    current = sel.file;
    setTimeout(() => viewer?.reveal(sel!.line, sel!.s), 30);
  }

  function clear() {
    sel = null;
    find = null;
    editing = null;
    ticks = {};
    if (fixed.kind === 'new') name = '';
  }

  async function save() {
    if (!canSave || !find || busy) return;
    busy = true;
    failure = '';
    const step: RuleValue = {
      key,
      name: name.trim() || key,
      find: { ...find, stop: find.stop || null },
      which,
      fallback: fixed.kind === 'add' ? fixed.fallback : fixed.kind === 'change' ? fixed.source.step.fallback : null,
      across,
    };
    try {
      await onsave(step, ticked.filter((f) => f !== fixedFile), fixed.kind === 'change' ? fixed.source.ruleId : null);
    } catch (e) {
      failure = String(e);
    }
    busy = false;
  }
</script>

<div class="teach">
  <header>
    <button type="button" class="back" onclick={oncancel}>{t('← Values')}</button>
    <h2>{fixed.kind === 'new' ? t('New value') : fixed.kind === 'add' ? t('Read {token} in another file', { token: `{${fixedKey}}` }) : t('Find {token} another way', { token: `{${fixedKey}}` })}</h2>
    <p class="quiet">{t('Click a word in a file, or hold and drag over the text you want.')}</p>
  </header>

  <div class="split">
    {#if loaded}
      <FileViewer
        bind:this={viewer}
        {files}
        bind:current
        marks={{ sel, anchor: sel && find ? find.lines_with : '', found, kept }}
        editing={editHint}
        oncancel={() => (editing = null)}
        {onselect}
        onopen={open}
      />
    {:else}
      <p class="quiet viewer-wait">{t('Reading the last capture\'s files…')}</p>
    {/if}

    <aside aria-label={t('The value')}>
      {#if !sel || !find}
        <section class="box empty">
          <h3>{t('Nothing selected yet')}</h3>
          <p>{t('A click takes the whole word: letters and numbers up to a space or a sign. Hold and drag to take more, like “Spawned 12 crew on Harbor_P”.')}</p>
          {#if loaded && files.length === 0}<p class="quiet">{t('The last capture has no text file: open or drop one.')}</p>{/if}
        </section>
      {:else}
        <section class="box">
          <div class="head">
            <h3>{t('Name')}</h3>
            <span class="for" title={selText}>{t('for “{text}”', { text: selText })}</span>
            <button type="button" class="small" onclick={clear}>{t('Select again')}</button>
          </div>
          {#if fixedKey}
            <code class="token">{`{${fixedKey}}`}</code>
          {:else}
            <div class="name">
              <input type="text" bind:value={name} aria-label={t('Name of the value')} aria-invalid={!!problem} class:bad={!!problem} placeholder={t('Map')} />
              <code class="token">{`{${key || '…'}}`}</code>
            </div>
            <p class="problem" role={problem ? 'alert' : undefined}>{problem}</p>
          {/if}
        </section>

        <section class="box how">
          <h3>{t('How to find it')}</h3>
          <span class="label">{t('Look in')}</span>
          <span class="row">
            <span>{find.lines_with ? t('lines with {q}', { q: q(find.lines_with) }) : t('every line')}</span>
            <button type="button" class="small" onclick={() => edit('anchor')} aria-label={t('Change the lines it looks in: select a text in the file')}>{t('✎ Change')}</button>
            {#if find.lines_with}<button type="button" class="small" onclick={() => (find = { ...find!, lines_with: '', from_line_start: true })}>{t('Every line')}</button>{/if}
          </span>
          <span class="label">{t('Starts')}</span>
          <Choices label={t('Starts')} value={find.from_line_start ? 'line' : 'after'} options={startOptions} onchange={(v) => (find = { ...find!, from_line_start: v === 'line' })} />
          <span class="label">{t('Stops')}</span>
          <span class="row">
            <Choices label={t('Stops')} value={find.stop ?? ''} options={stopOptions} onchange={(v) => (find = { ...find!, stop: v || null })} />
            <button type="button" class="small" onclick={() => edit('stop')} aria-label={t('Stop at another text: select it in the file')}>{t('✎ Other')}</button>
          </span>
          <span class="label">{t('Several')}</span>
          <Choices label={t('Several times in one file')} value={which} options={WHICH} onchange={(v) => (which = v)} />
          <p class="summary">{summary}</p>
        </section>

        <section class="box">
          <h3>{t('Read it in these files')}</h3>
          <div class="reads">
            {#each reads as r (r.f.id)}
              <label class="read">
                <input type="checkbox" checked={r.on} disabled={r.locked} onchange={() => (ticks = { ...ticks, [r.f.id]: !r.on })} />
                <span class="file" title={r.f.id}><b>{r.f.name}</b> <span class="quiet">{r.f.modified_ms ? t('changed {changedAt}', { changedAt: changedAt(r.f.modified_ms) }) : ''}</span></span>
                <span class="used" class:on={used(r)}>{t('Used')}</span>
                <span class="value" class:none={!r.got?.length} title={r.got?.length ? valueOf(r.got) : ''}>
                  {r.got === null ? t('Not read yet') : r.got.length ? valueOf(r.got) : t('Not found in this file')}
                </span>
              </label>
            {/each}
          </div>
          <div class="row across">
            <span>{t('When several have it, take')}</span>
            <Choices label={t('When several files have it')} value={across} options={ACROSS} onchange={(v) => (across = v)} />
          </div>
          <div class="gives">
            <span class="quiet">{t('Gives')}</span>
            <span class="what" class:none={!gives} title={gives}>{gives || t('Empty')}</span>
            <span class="why quiet">{why}</span>
          </div>
        </section>

        {#if failure}<p class="problem" role="alert">{failure}</p>{/if}
        <div class="buttons">
          <button type="button" onclick={oncancel}>{t('Cancel')}</button>
          <button type="button" class="primary" disabled={!canSave || busy} onclick={save}>{t('Save {token}', { token: `{${key || '…'}}` })}</button>
        </div>
      {/if}
    </aside>
  </div>
</div>

<style>
  .teach { display: grid; gap: 12px; }
  header { display: grid; gap: 4px; justify-items: start; }
  header h2 { margin: 0; font-size: 18px; }
  header p { margin: 0; }
  .back { border: 0; background: none; padding: 0; color: var(--accent); font-size: 13px; }
  .split { display: flex; flex-wrap: wrap; gap: 14px; align-items: flex-start; }
  .split > :global(.viewer), .viewer-wait { flex: 999 1 560px; }
  aside { flex: 1 1 340px; min-width: 0; max-width: 460px; display: grid; gap: 10px; }
  .box { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 14px; display: grid; gap: 8px; min-width: 0; }
  .box h3 { margin: 0; font-size: 14px; }
  .empty { border-style: dashed; }
  .empty p { margin: 0; font-size: 13px; }
  .head { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .for { color: var(--quiet); font-size: 13px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .head .small { margin-left: auto; flex: none; }
  .small { font-size: 12.5px; padding: 2px 8px; }
  .name { display: flex; gap: 8px; align-items: center; }
  .name input { flex: 1; min-width: 0; }
  .name input.bad { border-color: var(--err); box-shadow: 0 0 0 1px var(--err); }
  .token { font: 12.5px var(--mono); padding: 2px 8px; border-radius: 5px; background: color-mix(in srgb, var(--accent) 14%, var(--surface)); border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); justify-self: start; white-space: nowrap; }
  .problem { margin: 0; min-height: 18px; color: var(--err); font-size: 12.5px; }
  .how { grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 8px 12px; }
  .how h3, .summary { grid-column: 1 / -1; }
  .label { color: var(--quiet); font-size: 13px; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; min-width: 0; font-size: 13px; }
  .summary { margin: 0; padding: 6px 10px; border-radius: var(--radius-sm); background: var(--surface-2); font-size: 12.5px; overflow-wrap: anywhere; }
  .reads { display: grid; gap: 6px; max-height: 236px; overflow-y: auto; }
  .read { display: grid; grid-template-columns: auto minmax(0, 1fr) 44px; grid-template-rows: 20px 18px; column-gap: 8px; align-items: center; font-size: 13px; }
  .read input { grid-row: 1; margin: 0; }
  .file, .value { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .value { grid-column: 2 / 4; font: 12.5px var(--mono); }
  .value.none { font-family: var(--font); color: var(--quiet); }
  .used { visibility: hidden; justify-self: end; font-size: 11.5px; font-weight: 600; padding: 0 7px; border-radius: 99px; background: var(--accent); color: var(--on-accent); }
  .used.on { visibility: visible; }
  .across { font-size: 13px; }
  .gives { display: grid; grid-template-columns: auto minmax(0, 1fr); column-gap: 8px; padding: 6px 10px; border-radius: var(--radius-sm); background: var(--surface-2); font-size: 13px; }
  .what { font: 600 13px var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .what.none { font-family: var(--font); font-weight: 400; color: var(--quiet); }
  .why { grid-column: 2; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .buttons { display: flex; gap: 8px; justify-content: flex-end; }
</style>
