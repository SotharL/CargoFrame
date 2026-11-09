<script lang="ts">
  // The text box with formatting: it shows bold as bold, a quote as a quote, a list as a list, and
  // keeps the text as the marks the code that sends it reads (see richtext.ts). Buttons and their
  // shortcuts work on the selection; Enter goes on with a list; a paste comes in as plain text;
  // Ctrl+Z / Ctrl+Y undo and redo. A table shows as a grid: its cells are typed in on their own,
  // with buttons to add a row or a column and to take one out; the text keeps it as `| a | b |`.
  import { untrack } from 'svelte';
  import { applyMark, shortcut, type Mark } from './formatting';
  import {
    newLine,
    offsetOf,
    readText,
    select,
    selectionIn,
    showMarksAt,
    tableText,
    toHtml,
    type Table,
  } from './richtext';

  let {
    value = $bindable(''),
    marks,
    id,
    label,
    labelledby,
  }: {
    value: string;
    /** The marks on offer: the others' shortcuts are left alone. */
    marks: Mark[];
    id: string;
    label?: string;
    labelledby?: string;
  } = $props();

  let box = $state<HTMLDivElement>();
  /** The text the box shows now. */
  let shown: string | null = null;
  let composing = false;
  /** Where the selection was when the box last had it: a click on a button outside keeps it. */
  let lastSel: [number, number] | null = null;
  const here = (): [number, number] => (box ? selectionIn(box) : null) ?? lastSel ?? [value.length, value.length];

  /** The cell of a table the selection was last in, and where in it. */
  let lastCell: { cell: HTMLElement; a: number; b: number } | null = null;

  type State = { text: string; start: number; end: number };
  let undo: State[] = [];
  let redo: State[] = [];
  let lastPush = 0;
  let lastKind = '';

  function draw(text: string, sel: [number, number] | null) {
    if (!box) return;
    box.innerHTML = toHtml(text, sel);
    shown = text;
    if (sel) select(box, sel[0], sel[1]);
  }

  // the text changed from outside (a template, a variable filled in, a reset)
  $effect(() => {
    const v = value;
    if (box && v !== untrack(() => shown)) draw(v, null);
  });

  /** Keep what was there to undo to. */
  function remember(kind: string) {
    const now = Date.now();
    const sel = box ? selectionIn(box) : null;
    // typing, or deleting, in one go is one step
    const run = (kind === 'typing' || kind.startsWith('delete')) && kind === lastKind && now - lastPush <= 800;
    if (!run) {
      undo.push({ text: shown ?? value, start: sel?.[0] ?? 0, end: sel?.[1] ?? 0 });
      if (undo.length > 200) undo.shift();
    }
    redo = [];
    lastPush = now;
    lastKind = kind;
  }

  function set(text: string, start: number, end: number) {
    value = text;
    draw(text, [start, end]);
    lastSel = [start, end];
  }

  // ---- tables ----

  const cellOf = (n: EventTarget | Node | null): HTMLElement | null =>
    n instanceof Node ? ((n instanceof HTMLElement ? n : n.parentElement)?.closest<HTMLElement>('.cell') ?? null) : null;

  /** The table a grid shows now, read from its cells. */
  function tableIn(grid: HTMLElement): Table {
    const head: string[] = [];
    const rows: string[][] = [];
    for (const c of Array.from(grid.querySelectorAll<HTMLElement>('.cell'))) {
      const r = Number(c.dataset.r);
      const k = Number(c.dataset.c);
      const text = (c.textContent ?? '').replace(/\n/g, ' ');
      if (r === 0) head[k] = text;
      else (rows[r - 1] ??= [])[k] = text;
    }
    return { head, rows: rows.map((r) => head.map((_, k) => r[k] ?? '')) };
  }

  /** Put the table back in the text where the grid is; `redraw` draws the box again. */
  function putTable(grid: HTMLElement, t: Table, redraw: boolean): number {
    if (!box) return 0;
    const start = offsetOf(box, grid, 0);
    const old = grid.dataset.text ?? '';
    const text = tableText(t);
    const next = value.slice(0, start) + text + value.slice(start + old.length);
    grid.dataset.text = text;
    shown = next;
    value = next;
    if (redraw) draw(next, null);
    return start;
  }

  /** Where the selection is in a cell. */
  function inCell(cell: HTMLElement): [number, number] {
    const sel = document.getSelection();
    const len = cell.textContent?.length ?? 0;
    if (!sel || sel.rangeCount === 0 || !cell.contains(sel.anchorNode)) return [len, len];
    const r = sel.getRangeAt(0);
    const at = (n: Node, o: number) => {
      const x = document.createRange();
      x.setStart(cell, 0);
      x.setEnd(n, o);
      return x.toString().length;
    };
    return [at(r.startContainer, r.startOffset), at(r.endContainer, r.endOffset)];
  }

  function caretInCell(cell: HTMLElement, a: number, b = a) {
    cell.focus();
    const t = cell.firstChild;
    const sel = document.getSelection();
    if (!sel) return;
    const r = document.createRange();
    if (t) {
      r.setStart(t, Math.min(a, t.textContent?.length ?? 0));
      r.setEnd(t, Math.min(b, t.textContent?.length ?? 0));
    } else r.setStart(cell, 0);
    sel.removeAllRanges();
    sel.addRange(r);
    lastCell = { cell, a, b };
  }

  /** A cell's text changed: the table in the text follows. */
  function cellChanged(cell: HTMLElement) {
    const grid = cell.closest<HTMLElement>('.tw');
    if (grid) putTable(grid, tableIn(grid), false);
  }

  /** Change a cell's text around its selection. */
  function editCell(cell: HTMLElement, a: number, b: number, edit: (text: string) => { text: string; start: number; end: number }) {
    remember('cell');
    const e = edit(cell.textContent ?? '');
    cell.textContent = e.text.replace(/\n/g, ' ');
    caretInCell(cell, e.start, e.end);
    cellChanged(cell);
  }

  /** The cell a grid shows at row `r`, column `c`, after the box was drawn again. */
  function cellAt(nth: number, r: number, c: number): HTMLElement | null {
    const grid = box?.querySelectorAll<HTMLElement>('.tw')[nth];
    return grid?.querySelector<HTMLElement>(`.cell[data-r="${r}"][data-c="${c}"]`) ?? null;
  }

  /** The buttons on a grid: a row or a column more, or one less. */
  function onGridClick(e: MouseEvent) {
    const btn = (e.target as HTMLElement | null)?.closest<HTMLButtonElement>('button[data-act]');
    const grid = btn?.closest<HTMLElement>('.tw');
    if (!btn || !grid || !box) return;
    e.preventDefault();
    const nth = Array.from(box.querySelectorAll('.tw')).indexOf(grid);
    const t = tableIn(grid);
    const n = Number(btn.dataset.n ?? 0);
    let focus: [number, number] = [0, 0];
    switch (btn.dataset.act) {
      case 'addrow':
        t.rows.push(t.head.map(() => ''));
        focus = [t.rows.length, 0];
        break;
      case 'addcol':
        t.head.push(`Column ${t.head.length + 1}`);
        for (const r of t.rows) r.push('');
        focus = [0, t.head.length - 1];
        break;
      case 'row':
        t.rows.splice(n, 1);
        focus = [Math.min(n, t.rows.length), 0];
        break;
      case 'col':
        if (t.head.length < 2) return;
        t.head.splice(n, 1);
        for (const r of t.rows) r.splice(n, 1);
        focus = [0, Math.min(n, t.head.length - 1)];
        break;
      default:
        return;
    }
    remember('table');
    putTable(grid, t, true);
    const cell = cellAt(nth, focus[0], focus[1]);
    if (cell) caretInCell(cell, cell.textContent?.length ?? 0);
  }

  /** Tab goes to the next cell, Enter to the one below. */
  function moveFrom(cell: HTMLElement, dr: number, dc: number): boolean {
    const grid = cell.closest<HTMLElement>('.tw');
    if (!grid) return false;
    const cells = Array.from(grid.querySelectorAll<HTMLElement>('.cell'));
    const r = Number(cell.dataset.r);
    const c = Number(cell.dataset.c);
    const target =
      dr !== 0
        ? cells.find((x) => Number(x.dataset.r) === r + dr && Number(x.dataset.c) === c)
        : cells[cells.indexOf(cell) + dc];
    if (!target) return false;
    caretInCell(target, 0, target.textContent?.length ?? 0);
    return true;
  }

  /** A button (or its shortcut) changes the text around the selection and keeps the selection. */
  export function format(mark: Mark) {
    if (!box) return;
    if (lastCell?.cell.isConnected) {
      // in a cell: the marks of a line, a table or a code block have no place there
      if (['table', 'codeblock', 'quote', 'bullets', 'numbers'].includes(mark)) return;
      const { cell, a, b } = lastCell;
      editCell(cell, a, b, (t) => applyMark(t, a, b, mark));
      return;
    }
    const sel = here();
    remember('format');
    const edit = applyMark(value, sel[0], sel[1], mark);
    box.focus();
    set(edit.text, edit.start, edit.end);
    if (mark === 'table') {
      // straight into the new table's first cell, its name selected to type over
      const grid = Array.from(box.querySelectorAll<HTMLElement>('.tw')).find((g) => offsetOf(box!, g, 0) === edit.start);
      const cell = grid?.querySelector<HTMLElement>('.cell');
      if (cell) caretInCell(cell, 0, cell.textContent?.length ?? 0);
    }
  }

  /** Put text where the selection is (a variable, a link), in place of what is selected. */
  export function insert(text: string) {
    if (!box) return;
    if (lastCell?.cell.isConnected) {
      const { cell, a, b } = lastCell;
      const put = text.replace(/\s*\n\s*/g, ' ');
      editCell(cell, a, b, (t) => ({ text: t.slice(0, a) + put + t.slice(b), start: a + put.length, end: a + put.length }));
      return;
    }
    const [a, b] = here();
    remember('insert');
    box.focus();
    set(value.slice(0, a) + text + value.slice(b), a + text.length, a + text.length);
  }

  /** The selected text. */
  export function selected(): string {
    if (lastCell?.cell.isConnected) return (lastCell.cell.textContent ?? '').slice(lastCell.a, lastCell.b);
    const [a, b] = here();
    return value.slice(a, b);
  }

  function back(from: State[], to: State[]) {
    const s = from.pop();
    if (!s || !box) return;
    const sel = selectionIn(box);
    to.push({ text: value, start: sel?.[0] ?? 0, end: sel?.[1] ?? 0 });
    lastKind = '';
    set(s.text, s.start, s.end);
  }

  function onBeforeInput(e: InputEvent) {
    if (!box) return;
    const cell = cellOf(e.target);
    if (cell) {
      switch (e.inputType) {
        case 'insertParagraph':
        case 'insertLineBreak':
          // one line to a cell: Enter goes to the cell below
          e.preventDefault();
          moveFrom(cell, 1, 0);
          return;
        case 'insertFromPaste':
        case 'insertFromDrop':
        case 'insertReplacementText': {
          const text = e.dataTransfer?.getData('text/plain') ?? e.data;
          if (text == null) return;
          e.preventDefault();
          const [a, b] = inCell(cell);
          lastCell = { cell, a, b };
          insert(text);
          return;
        }
        case 'historyUndo':
        case 'historyRedo':
          break;
        default:
          if (e.inputType.startsWith('format')) e.preventDefault();
          else remember('typing');
          return;
      }
    }
    switch (e.inputType) {
      case 'insertParagraph':
      case 'insertLineBreak': {
        e.preventDefault();
        const [a, b] = selectionIn(box) ?? [value.length, value.length];
        remember('enter');
        const next = newLine(value, a, b);
        set(next.text, next.at, next.at);
        return;
      }
      case 'insertFromPaste':
      case 'insertFromDrop':
      case 'insertReplacementText': {
        const text = e.dataTransfer?.getData('text/plain') ?? e.data;
        if (text == null) return;
        e.preventDefault();
        insert(text.replace(/\r\n?/g, '\n'));
        return;
      }
      case 'historyUndo':
        e.preventDefault();
        back(undo, redo);
        return;
      case 'historyRedo':
        e.preventDefault();
        back(redo, undo);
        return;
      default:
        // the browser's own bold and the like would put tags in the box
        if (e.inputType.startsWith('format')) {
          e.preventDefault();
          return;
        }
        remember(e.inputType === 'insertText' ? 'typing' : e.inputType);
    }
  }

  /** The browser typed or deleted: read the text back and draw it with its marks. */
  function onInput(e?: Event) {
    if (!box || composing) return;
    const cell = cellOf(e?.target ?? document.getSelection()?.anchorNode ?? null);
    if (cell && box.contains(cell)) {
      cellChanged(cell);
      return;
    }
    const sel = selectionIn(box);
    const text = readText(box);
    value = text;
    draw(text, sel ?? [text.length, text.length]);
    lastSel = sel;
  }

  function onKey(e: KeyboardEvent) {
    const cell = cellOf(e.target);
    if (cell && e.key === 'Tab' && !e.ctrlKey && !e.altKey && moveFrom(cell, 0, e.shiftKey ? -1 : 1)) {
      e.preventDefault();
      return;
    }
    if (cell) {
      const [a, b] = inCell(cell);
      lastCell = { cell, a, b };
    }
    const mark = shortcut(e);
    if (mark) {
      if (marks.includes(mark)) {
        e.preventDefault();
        format(mark);
      } else if (['bold', 'italic', 'underline'].includes(mark)) {
        // no tags from the browser for marks not on offer
        e.preventDefault();
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && !e.altKey) {
      const k = e.key.toLowerCase();
      if (k === 'z' && !e.shiftKey) {
        e.preventDefault();
        back(undo, redo);
      } else if (k === 'y' || (k === 'z' && e.shiftKey)) {
        e.preventDefault();
        back(redo, undo);
      }
    }
  }

  function onSelection() {
    const cell = cellOf(document.activeElement);
    if (box && cell && box.contains(cell)) {
      const [a, b] = inCell(cell);
      lastCell = { cell, a, b };
      return;
    }
    if (box && document.activeElement === box) {
      lastCell = null;
      const sel = selectionIn(box);
      if (sel) lastSel = sel;
      showMarksAt(box, sel);
    }
  }

  $effect(() => {
    document.addEventListener('selectionchange', onSelection);
    return () => document.removeEventListener('selectionchange', onSelection);
  });
</script>

<div
  {id}
  bind:this={box}
  class="rich"
  contenteditable="true"
  role="textbox"
  tabindex="0"
  aria-multiline="true"
  aria-label={label}
  aria-labelledby={labelledby}
  spellcheck="true"
  onbeforeinput={onBeforeInput}
  oninput={onInput}
  onclick={onGridClick}
  onmousedown={(e) => {
    // a grid's buttons keep the focus where it is
    if ((e.target as HTMLElement | null)?.closest('button[data-act]')) e.preventDefault();
  }}
  onkeydown={onKey}
  oncompositionstart={() => (composing = true)}
  oncompositionend={(e) => {
    composing = false;
    onInput(e);
  }}
  onblur={() => box && showMarksAt(box, null)}
></div>

<style>
  .rich {
    width: 100%; height: 100%; min-height: 0; overflow: auto; outline: none;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 8px; font: inherit; color: inherit;
    white-space: pre-wrap; overflow-wrap: anywhere; cursor: text;
  }
  .rich:focus-visible { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .rich :global(.ln) { min-height: 1.4em; }
  .rich :global(.mk) { display: none; color: var(--quiet); font-weight: 400; font-style: normal; text-decoration: none; }
  .rich :global(.m.on > .mk) { display: inline; }
  .rich :global(.m.bold) { font-weight: 700; }
  .rich :global(.m.italic) { font-style: italic; }
  .rich :global(.m.underline) { text-decoration: underline; }
  .rich :global(.m.underline > .mk) { text-decoration: none; }
  .rich :global(code) {
    font-family: var(--mono); font-size: 0.92em; padding: 0 3px; border-radius: 3px;
    background: color-mix(in srgb, var(--text) 9%, transparent);
  }
  .rich :global(.lm) { color: var(--quiet); }
  /* a {{variable}}, filled in on each capture */
  .rich :global(.var) { color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, transparent); border-radius: 3px; }
  /* a quote: a bar on the left, its "> " kept in place but not drawn */
  .rich :global(.ln.quote) {
    border-left: 3px solid var(--border-strong); padding-left: 8px; color: var(--muted, inherit);
  }
  .rich :global(.ln.quote > .lm) { color: transparent; font-size: 0.5em; }
  /* a bullet: a dot drawn over its "- " */
  .rich :global(.ln.bullet) { padding-left: 6px; }
  .rich :global(.ln.bullet > .lm) { color: transparent; position: relative; }
  .rich :global(.ln.bullet > .lm::before) {
    content: '•'; position: absolute; left: 0; top: 0; color: var(--text);
  }
  .rich :global(.ln.number) { padding-left: 6px; }
  /* a code block: its lines in the code font, the ``` quiet */
  .rich :global(.ln.codeln), .rich :global(.ln.fence) {
    font-family: var(--mono); font-size: 0.92em; padding: 0 8px;
    background: color-mix(in srgb, var(--text) 7%, transparent);
  }
  .rich :global(.ln.fence) { color: var(--quiet); }
  /* a table: a grid to type in, with its buttons showing on hover */
  .rich :global(.tw) { position: relative; margin: 4px 0 18px; white-space: normal; cursor: default; }
  .rich :global(.tw table) { border-collapse: collapse; }
  .rich :global(.tw th), .rich :global(.tw td) { border: 1px solid var(--border-strong); padding: 0; vertical-align: top; }
  .rich :global(.tw thead tr:last-child th) { background: color-mix(in srgb, var(--text) 7%, transparent); font-weight: 600; }
  .rich :global(.tw .tw-top), .rich :global(.tw .tw-side) { border: none; background: none; padding: 0; }
  .rich :global(.tw .tw-top) { height: 14px; text-align: center; }
  .rich :global(.tw .cell) {
    display: block; min-width: 64px; min-height: 1.4em; padding: 3px 6px; outline: none;
    white-space: pre-wrap; cursor: text; text-align: left;
  }
  .rich :global(.tw .cell:focus) { box-shadow: inset 0 0 0 2px var(--accent); }
  .rich :global(.tw button) {
    font: inherit; font-size: 11px; line-height: 1; border: 1px solid var(--border-strong);
    background: var(--surface); color: var(--quiet); border-radius: 3px; cursor: pointer;
    padding: 1px 5px; opacity: 0; transition: opacity 0.1s;
  }
  .rich :global(.tw:hover button), .rich :global(.tw:focus-within button) { opacity: 1; }
  .rich :global(.tw button:hover) { color: var(--text); border-color: var(--accent); }
  .rich :global(.tw .tw-x:hover) { color: var(--danger, #d33); border-color: var(--danger, #d33); }
  .rich :global(.tw .tw-side) { padding-left: 3px; vertical-align: middle; }
  .rich :global(.tw .tw-add.row) { position: absolute; left: 0; bottom: -17px; }
</style>
