<script lang="ts">
  // The text box with formatting: it shows bold as bold, a quote as a quote, a list as a list, and
  // keeps the text as the marks the code that sends it reads (see richtext.ts). Buttons and their
  // shortcuts work on the selection; Enter goes on with a list; a paste comes in as plain text;
  // Ctrl+Z / Ctrl+Y undo and redo.
  import { untrack } from 'svelte';
  import { applyMark, shortcut, type Mark } from './formatting';
  import { newLine, readText, select, selectionIn, showMarksAt, toHtml } from './richtext';

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
  }

  /** A button (or its shortcut) changes the text around the selection and keeps the selection. */
  export function format(mark: Mark) {
    if (!box) return;
    const sel = selectionIn(box) ?? [value.length, value.length];
    remember('format');
    const edit = applyMark(value, sel[0], sel[1], mark);
    box.focus();
    set(edit.text, edit.start, edit.end);
  }

  function insert(text: string) {
    if (!box) return;
    const [a, b] = selectionIn(box) ?? [value.length, value.length];
    remember('insert');
    set(value.slice(0, a) + text + value.slice(b), a + text.length, a + text.length);
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
  function onInput() {
    if (!box || composing) return;
    const sel = selectionIn(box);
    const text = readText(box);
    value = text;
    draw(text, sel ?? [text.length, text.length]);
  }

  function onKey(e: KeyboardEvent) {
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
    if (box && document.activeElement === box) showMarksAt(box, selectionIn(box));
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
  onkeydown={onKey}
  oncompositionstart={() => (composing = true)}
  oncompositionend={() => {
    composing = false;
    onInput();
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
</style>
