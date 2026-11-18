<script lang="ts" module>
  /** A selection on one line of a file, from `s` to `e` (end excluded). */
  export interface Sel {
    file: string;
    line: number;
    s: number;
    e: number;
  }
  /** What the viewer marks on a line besides the search. */
  export interface Marks {
    sel: Sel | null;
    /** The text a line must have: underlined on each line that has it. */
    anchor: string;
    /** Where the value is found in the shown file, and the places kept. */
    found: { line: number; s: number; e: number }[];
    kept: { line: number; s: number; e: number }[];
  }
</script>

<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  // A file of the last capture, one 20px line per line of the file with scroll bars both ways,
  // and a search like Ctrl+F. Only the lines in view are drawn, so a large log stays quick.
  // A click or a drag on a line is handed to `onselect` as where it is on that line.
  import { search } from './pick';
  import { changedAt, type ViewFile } from './files';

  let {
    files,
    current = $bindable(),
    marks,
    editing = '',
    oncancel,
    onselect,
    onopen,
  }: {
    files: ViewFile[];
    current: string;
    marks: Marks;
    /** Set while a text is chosen for a setting: what to select. */
    editing?: string;
    oncancel?: () => void;
    onselect: (file: string, line: number, s: number, e: number) => void;
    onopen: (files: File[]) => void;
  } = $props();

  const ROW = 20;
  const HEIGHT = 440;

  let scroller = $state<HTMLDivElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let picker = $state<HTMLInputElement | null>(null);
  let top = $state(0);
  let q = $state('');
  let hit = $state(0);
  let only = $state(false);
  let over = $state(false);

  const file = $derived(files.find((f) => f.id === current) ?? files[0] ?? null);
  const lines = $derived(file?.lines ?? []);
  const hitsHere = $derived(search(lines, q));
  const at = $derived(hitsHere.length ? ((hit % hitsHere.length) + hitsHere.length) % hitsHere.length : 0);
  /** The lines shown: all of them, or those with the search. */
  const shown = $derived.by(() => {
    if (!only || !q) return lines.map((_, i) => i);
    return [...new Set(hitsHere.map((h) => h.line))];
  });
  const widest = $derived(lines.reduce((m, l) => Math.max(m, l.length), 0));
  const first = $derived(Math.max(0, Math.floor(top / ROW) - 10));
  const last = $derived(Math.min(shown.length, first + Math.ceil(HEIGHT / ROW) + 20));

  type Kind = 0 | 1 | 2 | 3 | 4 | 5 | 6;
  const CLASS = ['', 'anchor', 'hit', 'found', 'kept', 'here', 'sel'];

  /** A line cut into pieces by what is marked on it, the strongest mark winning. */
  function pieces(n: number): { t: string; c: string }[] {
    const line = lines[n] ?? '';
    if (!line) return [{ t: ' ', c: '' }];
    const mark = new Uint8Array(line.length);
    const put = (s: number, e: number, k: Kind) => {
      for (let i = Math.max(0, s); i < Math.min(line.length, e); i++) if (k > mark[i]!) mark[i] = k;
    };
    if (marks.anchor) {
      const k = line.indexOf(marks.anchor);
      if (k >= 0) put(k, k + marks.anchor.length, 1);
    }
    hitsHere.forEach((h, j) => { if (h.line === n) put(h.s, h.e, j === at ? 5 : 2); });
    for (const f of marks.found) if (f.line === n) put(f.s, f.e, 3);
    for (const f of marks.kept) if (f.line === n) put(f.s, f.e, 4);
    const sel = marks.sel;
    if (sel && file && sel.file === file.id && sel.line === n) put(sel.s, sel.e, 6);
    const out: { t: string; c: string }[] = [];
    for (let i = 0; i < line.length; ) {
      let j = i;
      while (j < line.length && mark[j] === mark[i]) j++;
      out.push({ t: line.slice(i, j), c: CLASS[mark[i]!]! });
      i = j;
    }
    return out;
  }

  /** Show line `n` of the file, in the middle of the view. */
  export function reveal(n: number, column = 0) {
    const i = shown.indexOf(n);
    if (!scroller || i < 0) return;
    scroller.scrollTop = Math.max(0, i * ROW - HEIGHT / 2 + ROW);
    const cw = charWidth();
    const x = column * cw;
    if (x < scroller.scrollLeft || x > scroller.scrollLeft + scroller.clientWidth - 80) scroller.scrollLeft = Math.max(0, x - 120);
  }

  let cw = 0;
  function charWidth(): number {
    if (cw || !scroller) return cw || 7.5;
    const probe = document.createElement('span');
    probe.className = 'probe';
    probe.textContent = 'MMMMMMMMMM';
    scroller.appendChild(probe);
    cw = probe.getBoundingClientRect().width / 10 || 7.5;
    probe.remove();
    return cw;
  }

  function step(by: number) {
    if (!hitsHere.length) return;
    hit = (at + by + hitsHere.length) % hitsHere.length;
    const h = hitsHere[hit]!;
    reveal(h.line, h.s);
  }

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      input?.focus();
      input?.select();
    }
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      step(e.shiftKey ? -1 : 1);
    } else if (e.key === 'Escape' && q) {
      e.preventDefault();
      q = '';
    }
  }

  /** Where `node`/`off` is in the text of `el`, counted in characters. */
  function offsetIn(el: Element, node: Node, off: number, length: number): number {
    if (!el.contains(node)) return el.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING ? length : 0;
    const r = document.createRange();
    r.selectNodeContents(el);
    r.setEnd(node, off);
    return r.toString().length;
  }

  /** Where the mouse is in the text, for a click that leaves no selection. */
  function caretAt(x: number, y: number): { node: Node; offset: number } | null {
    const d = document as Document & { caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null };
    if (d.caretPositionFromPoint) {
      const p = d.caretPositionFromPoint(x, y);
      return p ? { node: p.offsetNode, offset: p.offset } : null;
    }
    const r = document.caretRangeFromPoint?.(x, y);
    return r ? { node: r.startContainer, offset: r.startOffset } : null;
  }

  function onUp(e: MouseEvent) {
    if (!file) return;
    const s = window.getSelection();
    let a: { node: Node; offset: number } | null = s?.anchorNode ? { node: s.anchorNode, offset: s.anchorOffset } : null;
    let b: { node: Node; offset: number } | null = s?.focusNode ? { node: s.focusNode, offset: s.focusOffset } : a;
    if (!a || s?.isCollapsed) {
      a = caretAt(e.clientX, e.clientY) ?? a;
      b = a;
    }
    if (!a) return;
    const el = (n: Node) => (n instanceof Element ? n : n.parentElement);
    const row = el(a.node)?.closest('[data-line]');
    const text = row?.querySelector('.text');
    if (!row || !text || !scroller?.contains(row) || !text.contains(a.node)) return;
    const n = Number(row.getAttribute('data-line'));
    const len = (lines[n] ?? '').length;
    const from = offsetIn(text, a.node, a.offset, len);
    const to = b ? offsetIn(text, b.node, b.offset, len) : from;
    s?.removeAllRanges();
    onselect(file.id, n, Math.min(from, to), Math.max(from, to));
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    over = false;
    const list = [...(e.dataTransfer?.files ?? [])];
    if (list.length) onopen(list);
  }

  function counts(f: ViewFile) {
    return q && f.lines ? search(f.lines, q).length : 0;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="viewer"
  class:over
  data-tour="data.viewer"
  aria-label={t('The file')}
  onkeydown={onKey}
  ondragover={(e) => { e.preventDefault(); over = true; }}
  ondragleave={() => (over = false)}
  ondrop={onDrop}
>
  <div class="tabs" role="tablist" aria-label={t('Files of the last capture')}>
    {#each files as f (f.id)}
      <button
        type="button"
        role="tab"
        aria-selected={file?.id === f.id}
        title={f.id}
        onclick={() => { current = f.id; hit = 0; top = 0; if (scroller) scroller.scrollTop = 0; }}
      ><span class="tabname">{f.name}</span>{#if q}<span class="count" class:none={!counts(f)}>{counts(f)}</span>{/if}</button>
    {/each}
    <button type="button" class="open" onclick={() => picker?.click()}>{t('＋ Open or drop a file')}</button>
    <input bind:this={picker} type="file" multiple hidden onchange={(e) => { const l = [...(e.currentTarget.files ?? [])]; e.currentTarget.value = ''; if (l.length) onopen(l); }} />
  </div>

  <div class="find" role="search">
    <input
      bind:this={input}
      type="search"
      bind:value={q}
      oninput={() => (hit = 0)}
      onkeydown={onSearchKey}
      placeholder={t('Find in the files (Ctrl+F)')}
      aria-label={t('Find in the files')}
    />
    <span class="where" aria-live="polite">{q ? (hitsHere.length ? `${at + 1} of ${hitsHere.length}` : t('Not here')) : ''}</span>
    <button type="button" class="tiny" aria-label={t('Previous')} disabled={!hitsHere.length} onclick={() => step(-1)}>↑</button>
    <button type="button" class="tiny" aria-label={t('Next')} disabled={!hitsHere.length} onclick={() => step(1)}>↓</button>
    <label class="only"><input type="checkbox" bind:checked={only} /> {t('Only lines with it')}</label>
  </div>

  {#if editing}
    <div class="editing" role="status">
      <span>{editing}</span>
      <button type="button" onclick={() => oncancel?.()}>{t('Cancel')}</button>
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="lines" bind:this={scroller} style:height="{HEIGHT}px" onscroll={(e) => (top = e.currentTarget.scrollTop)} onmouseup={onUp}>
    {#if !file}
      <p class="note">{t('No file yet: open or drop one.')}</p>
    {:else if file.lines === null}
      <p class="note">{t('Reading {name}…', { name: file.name })}</p>
    {:else if file.problem}
      <p class="note">{file.problem}</p>
    {:else if lines.length === 0}
      <p class="note">{t('{name} is empty.', { name: file.name })}</p>
    {:else}
      <div class="inner" style:min-width="calc({widest}ch + 70px)" style:padding-top="{first * ROW}px" style:padding-bottom="{(shown.length - last) * ROW}px">
        {#each shown.slice(first, last) as n (n)}
          <div class="row" data-line={n}><span class="num" aria-hidden="true">{n + 1}</span><div class="text">{#each pieces(n) as p, i (i)}<span class={p.c}>{p.t}</span>{/each}</div></div>
        {/each}
      </div>
    {/if}
  </div>
  <p class="legend">
    {#if file?.cut}<span>{t('Only the end of this large file is here.')}</span>{/if}
    <span><span class="sel">{t('Selected')}</span></span>
    <span><span class="found">{t('Also found')}</span></span>
    <span><span class="hit">{t('Search')}</span></span>
    <span><span class="anchor">{t('Lines it looks in')}</span></span>
    {#if file && file.modified_ms}<span class="changed">{t('changed {time}', { time: changedAt(file.modified_ms) })}</span>{/if}
  </p>
</section>

<style>
  .viewer { min-width: 0; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; display: flex; flex-direction: column; }
  .viewer.over { outline: 2px dashed var(--accent); outline-offset: -4px; }
  .tabs { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 2px; padding: 8px 8px 0; background: var(--surface-2); border-bottom: 1px solid var(--border); }
  [role='tab'] {
    font-size: 13px; display: inline-flex; align-items: center; gap: 6px; padding: 5px 12px; border: 1px solid transparent;
    border-radius: 6px 6px 0 0; background: none; color: var(--quiet); margin-bottom: -1px;
    max-width: 260px; min-width: 0;
  }
  .tabname { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  [role='tab'][aria-selected='true'] { background: var(--surface); border-color: var(--border); border-bottom-color: var(--surface); color: var(--text); font-weight: 600; }
  .count { font-size: 11.5px; font-weight: 600; padding: 0 6px; border-radius: 99px; background: color-mix(in srgb, var(--dbg) 22%, transparent); }
  .count.none { background: var(--border); color: var(--quiet); }
  .open { font-size: 13px; padding: 4px 12px; border: 1px dashed var(--border-strong); background: none; margin: 0 0 6px auto; }
  .find { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-bottom: 1px solid var(--border); }
  .find input[type='search'] { flex: 1 1 200px; min-width: 0; }
  .where { min-width: 64px; font-size: 13px; }
  .tiny { width: 30px; height: 30px; padding: 0; }
  .only { display: flex; align-items: center; gap: 6px; font-size: 13px; }
  .editing { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; padding: 8px 12px; font-size: 13px; background: color-mix(in srgb, var(--warn) 16%, var(--surface)); border-bottom: 1px solid var(--border); }
  .editing button { margin-left: auto; font-size: 13px; padding: 2px 10px; }
  .lines { overflow: auto; font: 12.5px/20px var(--mono); }
  .inner { box-sizing: border-box; }
  .row { display: flex; height: 20px; }
  .num { position: sticky; left: 0; flex: none; width: 44px; padding-right: 10px; box-sizing: border-box; text-align: right; color: var(--quiet); user-select: none; background: var(--surface); }
  .text { white-space: pre; padding-right: 16px; cursor: text; user-select: text; -webkit-user-select: text; }
  .note { margin: 0; padding: 16px; color: var(--quiet); font: 13px var(--font); }
  :global(.lines .probe) { position: absolute; visibility: hidden; white-space: pre; }
  .legend { margin: 0; padding: 6px 12px; border-top: 1px solid var(--border); color: var(--quiet); font-size: 12px; display: flex; flex-wrap: wrap; gap: 4px 14px; }
  .legend .changed { margin-left: auto; }
  .legend > span > span { padding: 0 5px; border-radius: 3px; }
  .sel { background: var(--accent); color: var(--on-accent); border-radius: 2px; }
  .found { background: color-mix(in srgb, var(--warn) 26%, transparent); }
  .kept { background: color-mix(in srgb, var(--warn) 55%, transparent); box-shadow: inset 0 -2px 0 var(--warn); }
  .hit { background: color-mix(in srgb, var(--dbg) 22%, transparent); }
  .here { background: color-mix(in srgb, var(--dbg) 55%, transparent); }
  .anchor { text-decoration: underline 2px var(--accent); text-underline-offset: 3px; }
</style>
