<script lang="ts">
  import { onMount, tick } from 'svelte';
  import {
    COLORS, THICKNESS, add, clearAll, clampPoint, emptyState, isEdited, rectOf, redo, resetCrop,
    setCrop, strokeWidth, textSize, toPicturePoint, undo, visibleRect,
    type Annotation, type EditorState, type Pt, type PxRect, type Thickness,
  } from './model';
  import { exportPng, paint } from './render';

  let {
    image,
    original,
    size,
    edited = $bindable(false),
  }: {
    image: CanvasImageSource;
    /** The untouched PNG; returned as it is when nothing was changed. */
    original: Blob;
    size: { width: number; height: number };
    edited?: boolean;
  } = $props();

  type Tool = 'crop' | 'pen' | 'rect' | 'ellipse' | 'text' | null;
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

  // text tool
  let textAt = $state<{ pt: Pt; css: { x: number; y: number } } | null>(null);
  let textValue = $state('');
  let textInput = $state<HTMLInputElement>();

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
      commitText();
      const r = canvas!.getBoundingClientRect();
      textAt = { pt: p, css: { x: e.clientX - r.left, y: e.clientY - r.top } };
      textValue = '';
      tick().then(() => textInput?.focus());
      return;
    }
    canvas!.setPointerCapture(e.pointerId);
    start = p;
    const w = strokeWidth(size, thickness);
    if (tool === 'pen') live = { kind: 'pen', color, width: w, points: [p] };
    else if (tool === 'rect') live = { kind: 'rect', color, width: w, a: p, b: p };
    else if (tool === 'ellipse') live = { kind: 'ellipse', color, width: w, a: p, b: p };
    else if (tool === 'crop') cropPreview = rectOf(p, p);
  }

  function move(e: PointerEvent) {
    if (!start) return;
    const p = pointFrom(e);
    if (live?.kind === 'pen') {
      const last = live.points[live.points.length - 1]!;
      if (Math.hypot(p.x - last.x, p.y - last.y) >= 1) live = { ...live, points: [...live.points, p] };
    } else if (live && (live.kind === 'rect' || live.kind === 'ellipse')) {
      live = { ...live, b: e.shiftKey ? square(start, p) : p };
    } else if (tool === 'crop') {
      cropPreview = rectOf(start, p);
    }
  }

  function up(e: PointerEvent) {
    if (!start) return;
    canvas?.releasePointerCapture(e.pointerId);
    if (live) doc = add(doc, live);
    else if (tool === 'crop' && cropPreview) doc = setCrop(doc, cropPreview, size);
    live = null;
    cropPreview = null;
    start = null;
  }

  function commitText() {
    if (!textAt) return;
    const text = textValue;
    const at = textAt.pt;
    textAt = null;
    textValue = '';
    if (text.trim()) {
      doc = add(doc, { kind: 'text', color, size: textSize(size, thickness), at, text });
    }
  }

  function onTextKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      commitText();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      textAt = null;
      textValue = '';
    }
  }

  function pick(t: Exclude<Tool, null>) {
    commitText();
    tool = tool === t ? null : t;
  }

  function onKey(e: KeyboardEvent) {
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
      <button class="tool" class:on={tool === 'text'} aria-pressed={tool === 'text'} title="Text" aria-label="Text" onclick={() => pick('text')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6V4h14v2M12 4v16M9 20h6" /></svg>
      </button>
    </div>

    <div class="group" role="group" aria-label="Colour">
      {#each COLORS as c (c)}
        <button
          class="swatch"
          class:on={color === c}
          style:--sw={c}
          aria-label="Colour {c}"
          aria-pressed={color === c}
          onclick={() => (color = c)}
        ></button>
      {/each}
    </div>

    <div class="group" role="group" aria-label="Thickness">
      {#each THICKNESS as t (t)}
        <button class="thick" class:on={thickness === t} aria-pressed={thickness === t} title={t} onclick={() => (thickness = t)}>
          <span class="bar {t}"></span>
          <span class="sr-only">{t}</span>
        </button>
      {/each}
    </div>

    <div class="group">
      <button title="Undo (Ctrl+Z)" aria-label="Undo" disabled={doc.annotations.length === 0} onclick={() => (doc = undo(doc))}>Undo</button>
      <button title="Redo (Ctrl+Y)" aria-label="Redo" disabled={doc.redo.length === 0} onclick={() => (doc = redo(doc))}>Redo</button>
      <button disabled={!doc.crop} onclick={() => (doc = resetCrop(doc))}>Reset crop</button>
      <button disabled={!edited} onclick={() => (doc = clearAll())}>Clear all</button>
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
      {#if textAt}
        <input
          bind:this={textInput}
          class="text-input"
          type="text"
          style:left="{textAt.css.x}px"
          style:top="{textAt.css.y}px"
          style:color={color}
          bind:value={textValue}
          onkeydown={onTextKey}
          onblur={commitText}
          aria-label="Text to add"
          placeholder="Type, then Enter"
        />
      {/if}
    </div>
  </div>
  <div class="status quiet">
    {visible.w} × {visible.h} px{doc.crop ? ' (cropped)' : ''}
    {#if doc.annotations.length}· {doc.annotations.length} mark{doc.annotations.length === 1 ? '' : 's'}{/if}
    {#if tool === 'crop'}· drag to choose the part to keep{/if}
  </div>
</div>

<style>
  .editor { display: grid; grid-template-rows: auto 1fr auto; gap: 8px; height: 100%; min-height: 0; }
  .toolbar { display: flex; flex-wrap: wrap; gap: 6px 14px; align-items: center; }
  .group { display: inline-flex; gap: 4px; align-items: center; }
  .tool { width: 32px; height: 30px; padding: 0; display: inline-grid; place-items: center; }
  .tool svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .tool.on { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .swatch { width: 20px; height: 20px; padding: 0; border-radius: 50%; background: var(--sw); border: 2px solid var(--border-strong); }
  .swatch.on { outline: 2px solid var(--accent); outline-offset: 2px; }
  .thick { width: 30px; height: 28px; padding: 0; display: inline-grid; place-items: center; }
  .thick.on { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .bar { display: block; width: 16px; background: currentColor; border-radius: 2px; }
  .bar.thin { height: 2px; }
  .bar.medium { height: 4px; }
  .bar.thick { height: 7px; }
  .stage {
    min-height: 0; display: grid; place-items: center;
    background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--radius-sm);
    overflow: hidden; padding: 8px;
  }
  .frame { position: relative; line-height: 0; box-shadow: 0 1px 4px rgb(0 0 0 / 0.25); }
  canvas { display: block; touch-action: none; }
  .text-input {
    position: absolute; transform: translate(0, -50%); min-width: 160px;
    background: rgb(0 0 0 / 0.55); border: 1px solid var(--accent); border-radius: 4px; padding: 2px 6px;
    font-weight: 600; z-index: 2;
  }
  .status { font-size: 12px; }
</style>
