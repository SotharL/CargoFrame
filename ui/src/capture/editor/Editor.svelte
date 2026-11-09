<script lang="ts">
  import { onMount, tick } from 'svelte';
  import {
    COLORS, THICKNESS, add, clearAll, clampPoint, emptyState, isEdited, rectOf, redo, resetCrop,
    setCrop, strokeWidth, takeOut, textAt as textIndexAt, textSize, toPicturePoint, undo, visibleRect,
    type Annotation, type EditorState, type Pt, type PxRect, type TextStyle, type Thickness,
  } from './model';
  import { exportPng, fontOf, measureLine, paint } from './render';
  import StylePicker from './StylePicker.svelte';
  import { READ_TRIES, placed, regionPng, toCopy } from './ocr';
  import type { TextReading } from '../../lib/types';

  let {
    image,
    original,
    size,
    edited = $bindable(false),
    reader,
  }: {
    image: CanvasImageSource;
    /** The untouched PNG; returned as it is when nothing was changed. */
    original: Blob;
    size: { width: number; height: number };
    edited?: boolean;
    /** Reads the text in a part of the picture; without it there is no "Read text" tool. */
    reader?: (png: Blob, attempt: number) => Promise<TextReading>;
  } = $props();

  type Tool = 'crop' | 'pen' | 'rect' | 'ellipse' | 'arrow' | 'text' | 'read' | null;
  let tool = $state<Tool>(null);
  let color = $state(COLORS[0]!);
  let thickness = $state<Thickness>('medium');
  let doc = $state<EditorState>(emptyState());
  let live = $state<Annotation | null>(null);
  let cropPreview = $state<PxRect | null>(null);

  let box = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let cssSize = $state({ w: 100, h: 100 });
  let dpr = $state(1);

  // text tool: where the box is, what is typed, and how it looks
  let textAt = $state<{ pt: Pt; css: { x: number; y: number } } | null>(null);
  let textValue = $state('');
  let textInput = $state<HTMLTextAreaElement>();
  let textStyle = $state<TextStyle>({ bold: true, italic: false, box: false });
  /** The size of the text being edited, kept when an existing text is changed. */
  let textPx = $state<number | null>(null);

  // read text: the part being chosen, then what was read in it
  let readPreview = $state<PxRect | null>(null);
  type Reading = {
    rect: PxRect;
    attempt: number;
    status: 'reading' | 'done' | 'failed';
    result: TextReading | null;
    error: string;
    /** The lines picked to copy (none: all of them). */
    picked: number[];
    copied: boolean;
  };
  let reading = $state<Reading | null>(null);
  /** Counts the reads, so a late answer to an earlier one is dropped. */
  let readRun = 0;

  const visible = $derived(visibleRect(doc, size));

  $effect(() => {
    edited = isEdited(doc);
  });

  function fit() {
    if (!box) return;
    const aw = Math.max(80, box.clientWidth);
    const ah = Math.max(80, box.clientHeight);
    const s = Math.min(aw / visible.w, ah / visible.h, 1);
    cssSize = { w: Math.max(1, Math.floor(visible.w * s)), h: Math.max(1, Math.floor(visible.h * s)) };
    dpr = window.devicePixelRatio || 1;
  }

  $effect(() => {
    // Refit when the visible area changes (crop, reset).
    visible;
    fit();
  });

  onMount(() => {
    fit();
    const ro = new ResizeObserver(fit);
    if (box) ro.observe(box);
    return () => ro.disconnect();
  });

  // Repaint on any change. The canvas holds device pixels; CSS scales it to `cssSize`.
  $effect(() => {
    if (!canvas) return;
    const w = Math.max(1, Math.round(cssSize.w * dpr));
    const h = Math.max(1, Math.round(cssSize.h * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    paint(ctx, image, size, doc, canvas, { live, cropPreview });
  });

  function pointFrom(e: PointerEvent): Pt {
    const r = canvas!.getBoundingClientRect();
    return clampPoint(
      toPicturePoint({ x: e.clientX, y: e.clientY }, r, visible),
      size,
    );
  }

  function square(a: Pt, b: Pt): Pt {
    const d = Math.max(Math.abs(b.x - a.x), Math.abs(b.y - a.y));
    return { x: a.x + Math.sign(b.x - a.x || 1) * d, y: a.y + Math.sign(b.y - a.y || 1) * d };
  }

  let start: Pt | null = null;

  function down(e: PointerEvent) {
    if (!tool || e.button !== 0) return;
    const p = pointFrom(e);
    if (tool === 'text') {
      // without this the click takes the focus back from the box just opened, which closes it
      e.preventDefault();
      commitText();
      openText(p);
      return;
    }
    canvas!.setPointerCapture(e.pointerId);
    start = p;
    const w = strokeWidth(size, thickness);
    if (tool === 'pen') live = { kind: 'pen', color, width: w, points: [p] };
    else if (tool === 'rect') live = { kind: 'rect', color, width: w, a: p, b: p };
    else if (tool === 'ellipse') live = { kind: 'ellipse', color, width: w, a: p, b: p };
    else if (tool === 'arrow') live = { kind: 'arrow', color, width: w, a: p, b: p };
    else if (tool === 'crop') cropPreview = rectOf(p, p);
    else if (tool === 'read') readPreview = rectOf(p, p);
  }

  function move(e: PointerEvent) {
    if (!start) return;
    const p = pointFrom(e);
    if (live?.kind === 'pen') {
      const last = live.points[live.points.length - 1]!;
      if (Math.hypot(p.x - last.x, p.y - last.y) >= 1) live = { ...live, points: [...live.points, p] };
    } else if (live && (live.kind === 'rect' || live.kind === 'ellipse')) {
      live = { ...live, b: e.shiftKey ? square(start, p) : p };
    } else if (live?.kind === 'arrow') {
      live = { ...live, b: p };
    } else if (tool === 'crop') {
      cropPreview = rectOf(start, p);
    } else if (tool === 'read') {
      readPreview = rectOf(start, p);
    }
  }

  function up(e: PointerEvent) {
    if (!start) return;
    canvas?.releasePointerCapture(e.pointerId);
    if (live) doc = add(doc, live);
    else if (tool === 'crop' && cropPreview) doc = setCrop(doc, cropPreview, size);
    else if (tool === 'read' && readPreview && readPreview.w >= 4 && readPreview.h >= 4) void readIn(readPreview, 0);
    live = null;
    cropPreview = null;
    readPreview = null;
    start = null;
  }

  /** Read the text in the part `rect` of the picture, the `attempt`-th way. */
  async function readIn(rect: PxRect, attempt: number) {
    if (!reader) return;
    const run = ++readRun;
    reading = { rect, attempt, status: 'reading', result: null, error: '', picked: [], copied: false };
    try {
      const png = await regionPng(image, rect);
      const result = await reader(png, attempt);
      if (run !== readRun) return;
      reading = { rect, attempt, status: 'done', result, error: '', picked: [], copied: false };
    } catch (e) {
      if (run !== readRun) return;
      reading = { rect, attempt, status: 'failed', result: null, error: e instanceof Error ? e.message : String(e), picked: [], copied: false };
    }
  }

  const tryAgain = () => reading && void readIn(reading.rect, (reading.attempt + 1) % READ_TRIES);

  function closeReading() {
    readRun++;
    reading = null;
  }

  function pickLine(i: number) {
    if (!reading) return;
    const picked = reading.picked.includes(i) ? reading.picked.filter((n) => n !== i) : [...reading.picked, i];
    reading = { ...reading, picked, copied: false };
  }

  async function copyReading() {
    if (!reading?.result) return;
    try {
      await navigator.clipboard.writeText(toCopy(reading.result, reading.picked));
      reading = { ...reading, copied: true };
    } catch {
      reading = { ...reading, error: 'The text could not be copied: select it below and press Ctrl+C' };
    }
  }

  /** Where on the frame a part of the picture is shown. */
  function cssRect(r: PxRect) {
    const a = cssOf({ x: r.x, y: r.y });
    const b = cssOf({ x: r.x + r.w, y: r.y + r.h });
    return { x: a.x, y: a.y, w: b.x - a.x, h: b.y - a.y };
  }
  const readBox = $derived(readPreview ? cssRect(readPreview) : reading ? cssRect(reading.rect) : null);
  const readLines = $derived(
    reading?.result ? placed(reading.result, reading.rect).map((l, i) => ({ ...cssRect(l), text: l.text, i })) : [],
  );

  /** Where on the frame a point of the picture is shown. */
  function cssOf(p: Pt) {
    return { x: ((p.x - visible.x) / visible.w) * cssSize.w, y: ((p.y - visible.y) / visible.h) * cssSize.h };
  }

  /** Open the text box at `p`: on a text already there, that text comes back to be changed. */
  function openText(p: Pt) {
    const i = textIndexAt(doc, p, measureLine);
    if (i >= 0) {
      const out = takeOut(doc, i);
      const m = out.mark;
      if (m?.kind === 'text') {
        doc = out.state;
        color = m.color;
        textStyle = { bold: m.bold !== false, italic: !!m.italic, box: !!m.box };
        textPx = m.size;
        textAt = { pt: m.at, css: cssOf(m.at) };
        textValue = m.text;
        tick().then(() => textInput?.focus());
        return;
      }
    }
    textPx = null;
    textAt = { pt: p, css: cssOf(p) };
    textValue = '';
    tick().then(() => textInput?.focus());
  }

  function commitText() {
    if (!textAt) return;
    const text = textValue.replace(/\s+$/, '');
    const at = textAt.pt;
    textAt = null;
    textValue = '';
    if (text.trim()) {
      doc = add(doc, { kind: 'text', color, size: textPx ?? textSize(size, thickness), at, text, ...textStyle });
    }
    textPx = null;
  }

  function onTextKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      commitText();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      textAt = null;
      textValue = '';
    } else if ((e.ctrlKey || e.metaKey) && (e.key === 'b' || e.key === 'i')) {
      e.preventDefault();
      if (e.key === 'b') textStyle.bold = !textStyle.bold;
      else textStyle.italic = !textStyle.italic;
    }
  }

  /** Leaving the box puts the text on the picture, unless the focus went to its own buttons. */
  function onTextBlur(e: FocusEvent) {
    const to = e.relatedTarget as HTMLElement | null;
    if (to?.closest('[data-text-tools]') || to?.closest('.style')) return;
    commitText();
  }

  /** How big the text box is drawn on screen, so what is typed looks like the result. */
  const textCss = $derived(textPx ?? textSize(size, thickness));
  const textFont = $derived(fontOf({ kind: 'text', color, size: (textCss * cssSize.w) / visible.w, at: { x: 0, y: 0 }, text: '', ...textStyle }));

  function pick(t: Exclude<Tool, null>) {
    commitText();
    if (t !== 'read') closeReading();
    tool = tool === t ? null : t;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && reading) {
      closeReading();
      return;
    }
    if (!(e.ctrlKey || e.metaKey)) return;
    const k = e.key.toLowerCase();
    if (k === 'z' && !e.shiftKey) {
      e.preventDefault();
      doc = undo(doc);
    } else if (k === 'y' || (k === 'z' && e.shiftKey)) {
      e.preventDefault();
      doc = redo(doc);
    }
  }

  /** The picture to send: the original file untouched, or the edited one. */
  export async function exportImage(): Promise<Blob> {
    commitText();
    if (!isEdited(doc)) return original;
    return exportPng(image, size, doc);
  }

  const cursor = $derived(tool === 'text' ? 'text' : tool ? 'crosshair' : 'default');
  const readingText = $derived(reading?.result ? toCopy(reading.result, reading.picked) : '');
  const textRows = $derived(Math.max(1, textValue.split('\n').length));
</script>

<svelte:window onkeydown={onKey} />

<div class="editor">
  <div class="toolbar" role="toolbar" aria-label="Screenshot tools">
    <div class="group">
      <button class="tool" class:on={tool === 'crop'} aria-pressed={tool === 'crop'} title="Crop" aria-label="Crop" onclick={() => pick('crop')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 2v16h16M2 6h16v16" /></svg>
      </button>
      <button class="tool" class:on={tool === 'pen'} aria-pressed={tool === 'pen'} title="Draw" aria-label="Draw" onclick={() => pick('pen')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20l1-4L17 4l3 3L8 19z" /></svg>
      </button>
      <button class="tool" class:on={tool === 'rect'} aria-pressed={tool === 'rect'} title="Square (hold Shift for equal sides)" aria-label="Square" onclick={() => pick('rect')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="14" /></svg>
      </button>
      <button class="tool" class:on={tool === 'ellipse'} aria-pressed={tool === 'ellipse'} title="Circle (hold Shift for a true circle)" aria-label="Circle" onclick={() => pick('ellipse')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><ellipse cx="12" cy="12" rx="8" ry="7" /></svg>
      </button>
      <button class="tool" class:on={tool === 'arrow'} aria-pressed={tool === 'arrow'} title="Arrow" aria-label="Arrow" onclick={() => pick('arrow')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 19L19 5M10 5h9v9" /></svg>
      </button>
      <button class="tool" class:on={tool === 'text'} aria-pressed={tool === 'text'} title="Text: click to write, click a text to change it" aria-label="Text" onclick={() => pick('text')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6V4h14v2M12 4v16M9 20h6" /></svg>
      </button>
      {#if reader}
        <button class="tool" class:on={tool === 'read'} aria-pressed={tool === 'read'} title="Read text: drag over the text to copy it" aria-label="Read text" onclick={() => pick('read')}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4M8 9h8M12 9v7" /></svg>
        </button>
      {/if}
    </div>
    {#if tool === 'text'}
      <div class="group" data-text-tools role="group" aria-label="Text style">
        <button class="tool letter" class:on={textStyle.bold} aria-pressed={!!textStyle.bold} title="Bold (Ctrl+B)" aria-label="Bold" onmousedown={(e) => e.preventDefault()} onclick={() => (textStyle.bold = !textStyle.bold)}><b>B</b></button>
        <button class="tool letter" class:on={textStyle.italic} aria-pressed={!!textStyle.italic} title="Italic (Ctrl+I)" aria-label="Italic" onmousedown={(e) => e.preventDefault()} onclick={() => (textStyle.italic = !textStyle.italic)}><i>I</i></button>
        <button class="tool" class:on={textStyle.box} aria-pressed={!!textStyle.box} title="On a box: readable on any picture" aria-label="On a box" onmousedown={(e) => e.preventDefault()} onclick={() => (textStyle.box = !textStyle.box)}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="6" width="18" height="12" rx="2" /><path d="M8 10h8M12 10v5" /></svg>
        </button>
      </div>
    {/if}

    <span class="sep" aria-hidden="true"></span>

    <StylePicker colors={COLORS} thicknesses={THICKNESS} bind:color bind:thickness />

    <span class="sep" aria-hidden="true"></span>

    <div class="group">
      <button class="tool" title="Undo (Ctrl+Z)" aria-label="Undo" disabled={doc.annotations.length === 0} onclick={() => (doc = undo(doc))}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 010 11H11" /></svg>
      </button>
      <button class="tool" title="Redo (Ctrl+Y)" aria-label="Redo" disabled={doc.redo.length === 0} onclick={() => (doc = redo(doc))}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 000 11H13" /></svg>
      </button>
      <button class="tool" title="Reset crop: show the whole picture again" aria-label="Reset crop" disabled={!doc.crop} onclick={() => (doc = resetCrop(doc))}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg>
      </button>
      <button class="tool" title="Clear all: remove every mark and the crop" aria-label="Clear all" disabled={!edited} onclick={() => (doc = clearAll())}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M10 7V4h4v3M6 7l1 13h10l1-13M10 11v6M14 11v6" /></svg>
      </button>
    </div>
  </div>

  <div class="stage" bind:this={box}>
    <div class="frame" style:width="{cssSize.w}px" style:height="{cssSize.h}px">
      <canvas
        bind:this={canvas}
        style:width="{cssSize.w}px"
        style:height="{cssSize.h}px"
        style:cursor={cursor}
        aria-label="Screenshot, {visible.w} by {visible.h} pixels"
        onpointerdown={down}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={up}
      ></canvas>
      {#if readBox}
        <div class="read-area" class:busy={reading?.status === 'reading'} style:left="{readBox.x}px" style:top="{readBox.y}px" style:width="{readBox.w}px" style:height="{readBox.h}px" aria-hidden="true"></div>
      {/if}
      {#each readLines as l (l.i)}
        <button
          type="button"
          class="read-line"
          class:picked={reading?.picked.includes(l.i)}
          style:left="{l.x}px"
          style:top="{l.y}px"
          style:width="{l.w}px"
          style:height="{l.h}px"
          title={l.text}
          aria-label="Line {l.i + 1}: {l.text}"
          aria-pressed={reading?.picked.includes(l.i) ?? false}
          onclick={() => pickLine(l.i)}
        ></button>
      {/each}
      {#if textAt}
        <textarea
          bind:this={textInput}
          class="text-input"
          class:box={textStyle.box}
          rows={textRows}
          style:left="{textAt.css.x}px"
          style:top="{textAt.css.y}px"
          style:color={color}
          style:font={textFont}
          bind:value={textValue}
          onkeydown={onTextKey}
          onblur={onTextBlur}
          aria-label="Text to add"
          placeholder="Type, Enter to place"
        ></textarea>
      {/if}
    </div>
  </div>
  {#if reading}
    <div class="reading" role="region" aria-label="Text read">
      {#if reading.status === 'reading'}
        <p class="quiet" role="status">Reading the text…</p>
      {:else if reading.status === 'failed'}
        <p class="warn" role="alert">{reading.error}</p>
      {:else if reading.result && reading.result.lines.length === 0}
        <p class="quiet" role="status">No text found here. Try again, or choose a slightly larger part.</p>
      {:else if reading.result}
        <textarea class="found selectable" readonly rows={Math.min(5, Math.max(1, readingText.split('\n').length))} aria-label="The text found" value={readingText}></textarea>
        {#if reading.error}<p class="warn">{reading.error}</p>{/if}
      {/if}
      <div class="read-buttons">
        {#if reading.status === 'done' && reading.result?.lines.length}
          <button type="button" class="primary" onclick={copyReading}>
            {reading.copied ? 'Copied' : reading.picked.length ? `Copy ${reading.picked.length} line${reading.picked.length === 1 ? '' : 's'}` : 'Copy'}
          </button>
        {/if}
        <button type="button" onclick={tryAgain} disabled={reading.status === 'reading'} title="Read it another way">Try again</button>
        <button type="button" onclick={closeReading}>Close</button>
        {#if reading.result}
          <span class="quiet small">Read on this computer ({reading.result.reader}), way {reading.attempt + 1} of {READ_TRIES}{reading.result.lines.length > 1 ? ' · click lines to copy only those' : ''}</span>
        {/if}
      </div>
    </div>
  {/if}
  <div class="status quiet">
    {visible.w} × {visible.h} px{doc.crop ? ' (cropped)' : ''}
    {#if doc.annotations.length}· {doc.annotations.length} mark{doc.annotations.length === 1 ? '' : 's'}{/if}
    {#if tool === 'crop'}· drag to choose the part to keep{/if}
    {#if tool === 'read'}· drag over the text to read it{/if}
    {#if tool === 'text'}· click to write (Shift+Enter for a new line), click a text to change it{/if}
  </div>
</div>

<style>
  .editor { display: grid; grid-template-rows: auto 1fr auto; gap: 8px; height: 100%; min-height: 0; }
  .editor:has(.reading) { grid-template-rows: auto 1fr auto auto; }
  /* read text: the part read, its lines (click to pick), and what was found */
  .read-area { position: absolute; border: 2px dashed var(--accent); border-radius: 2px; pointer-events: none; z-index: 1; }
  .read-area.busy { animation: reading 1s ease-in-out infinite alternate; }
  @keyframes reading { from { border-color: var(--accent); } to { border-color: transparent; } }
  @media (prefers-reduced-motion: reduce) { .read-area.busy { animation: none; } }
  .read-line {
    position: absolute; z-index: 1; padding: 0; border: none; border-radius: 2px; cursor: pointer;
    background: color-mix(in srgb, #ffd400 32%, transparent); box-shadow: 0 0 0 1px color-mix(in srgb, #ffd400 70%, transparent);
  }
  .read-line:hover { background: color-mix(in srgb, #ffd400 45%, transparent); }
  .read-line.picked { background: color-mix(in srgb, var(--accent) 40%, transparent); box-shadow: 0 0 0 2px var(--accent); }
  .reading { display: grid; gap: 6px; padding: 8px; border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); }
  .found { width: 100%; resize: vertical; font: 13px var(--mono); background: var(--surface-2); border: 1px solid var(--border); border-radius: 4px; padding: 4px 6px; }
  .read-buttons { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .small { font-size: 12px; }
  .warn { color: var(--warn); font-size: 13px; }
  /* One line: icon buttons only. A block too narrow for that wraps rather than cuts anything off. */
  .toolbar { display: flex; flex-wrap: wrap; gap: 4px 4px; align-items: center; }
  .group { display: inline-flex; gap: 2px; align-items: center; }
  .sep { width: 1px; height: 18px; background: var(--border-strong); margin: 0 4px; }
  .tool { width: 28px; height: 28px; padding: 0; display: inline-grid; place-items: center; }
  .tool svg { width: 17px; height: 17px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .tool.on { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .stage {
    min-height: 0; display: grid; place-items: center;
    background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--radius-sm);
    overflow: hidden; padding: 8px;
  }
  .frame { position: relative; line-height: 0; box-shadow: 0 1px 4px rgb(0 0 0 / 0.25); }
  canvas { display: block; touch-action: none; }
  .tool.letter { font-size: 14px; font-family: Georgia, serif; }
  /* the box is drawn where the text will be, in its size, so what is typed is what is placed */
  .text-input {
    position: absolute; min-width: 140px; max-width: 90%; resize: none; overflow: hidden;
    background: rgb(0 0 0 / 0.35); border: 1px dashed var(--accent); border-radius: 3px; padding: 0;
    line-height: 1.25; z-index: 2; white-space: pre; field-sizing: content;
  }
  .text-input.box { background: rgb(255 255 255 / 0.85); }
  .status { font-size: 12px; }
</style>
