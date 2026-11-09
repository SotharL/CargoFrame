<script lang="ts">
  // Marks drawn over the recording: lines, squares, circles, arrows and text, in any colour and
  // thickness, as on a screenshot. They are in the recording's own pixels, on a see-through
  // canvas laid over the picture; `exportMarks` draws them on a see-through picture the size of
  // the recording, which ffmpeg lays over the kept part, so what is seen is what is sent.
  import { onMount, tick } from 'svelte';
  import {
    add, clampPoint, strokeWidth, takeOut, textAt as textIndexAt, textSize,
    type Annotation, type EditorState, type Pt, type TextStyle, type Thickness,
  } from '../editor/model';
  import { drawAnnotations, fontOf, measureLine } from '../editor/render';
  import { containRect } from './contain';

  export type MarkTool = 'pen' | 'rect' | 'ellipse' | 'arrow' | 'text';

  let {
    width,
    height,
    doc = $bindable(),
    tool,
    color = $bindable(),
    thickness,
    textStyle = $bindable(),
    active = true,
  }: {
    width: number;
    height: number;
    doc: EditorState;
    tool: MarkTool;
    color: string;
    thickness: Thickness;
    textStyle: TextStyle;
    /** False while another tool has the mouse: the marks are shown but not drawn on. */
    active?: boolean;
  } = $props();

  const size = $derived({ width, height });
  let layer = $state<HTMLElement | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let space = $state({ w: 0, h: 0 });
  let live = $state<Annotation | null>(null);
  let start: Pt | null = null;

  let textBox = $state<{ pt: Pt } | null>(null);
  let textValue = $state('');
  let textPx = $state<number | null>(null);
  let input = $state<HTMLTextAreaElement | null>(null);

  onMount(() => {
    if (!layer || typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => measure());
    watch.observe(layer);
    measure();
    return () => watch.disconnect();
  });
  function measure() {
    if (layer) space = { w: layer.clientWidth, h: layer.clientHeight };
  }
  const shown = $derived(containRect(space, width, height));
  const k = $derived(shown.w > 0 ? shown.w / width : 1);

  // Draw the marks, scaled to the canvas, whenever they change.
  $effect(() => {
    if (!canvas) return;
    const dpr = (typeof window !== 'undefined' && window.devicePixelRatio) || 1;
    const w = Math.max(1, Math.round(shown.w * dpr));
    const h = Math.max(1, Math.round(shown.h * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, w, h);
    ctx.save();
    ctx.scale(w / width, h / height);
    drawAnnotations(ctx, doc.annotations);
    if (live) drawAnnotations(ctx, [live]);
    ctx.restore();
  });

  function pointOf(e: PointerEvent): Pt {
    const r = canvas!.getBoundingClientRect();
    const x = r.width > 0 ? ((e.clientX - r.left) / r.width) * width : 0;
    const y = r.height > 0 ? ((e.clientY - r.top) / r.height) * height : 0;
    return clampPoint({ x, y }, size);
  }

  function down(e: PointerEvent) {
    if (!active || e.button !== 0) return;
    const p = pointOf(e);
    if (tool === 'text') {
      // keeps the focus in the text box that opens
      e.preventDefault();
      commitText();
      openText(p);
      return;
    }
    canvas!.setPointerCapture?.(e.pointerId);
    start = p;
    const w = strokeWidth(size, thickness);
    live =
      tool === 'pen'
        ? { kind: 'pen', color, width: w, points: [p] }
        : { kind: tool, color, width: w, a: p, b: p };
  }
  function move(e: PointerEvent) {
    if (!start || !live) return;
    const p = pointOf(e);
    if (live.kind === 'pen') {
      const last = live.points[live.points.length - 1]!;
      if (Math.hypot(p.x - last.x, p.y - last.y) >= 1) live = { ...live, points: [...live.points, p] };
    } else if (live.kind === 'rect' || live.kind === 'ellipse' || live.kind === 'arrow') {
      live = { ...live, b: p };
    }
  }
  function up(e: PointerEvent) {
    if (!start) return;
    canvas?.releasePointerCapture?.(e.pointerId);
    if (live) doc = add(doc, live);
    live = null;
    start = null;
  }

  function openText(p: Pt) {
    const i = textIndexAt(doc, p, measureLine);
    if (i >= 0) {
      const out = takeOut(doc, i);
      if (out.mark?.kind === 'text') {
        doc = out.state;
        color = out.mark.color;
        textStyle = { bold: out.mark.bold !== false, italic: !!out.mark.italic, box: !!out.mark.box };
        textPx = out.mark.size;
        textBox = { pt: out.mark.at };
        textValue = out.mark.text;
        void tick().then(() => input?.focus());
        return;
      }
    }
    textPx = null;
    textBox = { pt: p };
    textValue = '';
    void tick().then(() => input?.focus());
  }
  /** Put the text being written on the recording. */
  export function commitText() {
    if (!textBox) return;
    const text = textValue.replace(/\s+$/, '');
    const at = textBox.pt;
    textBox = null;
    textValue = '';
    if (text.trim()) doc = add(doc, { kind: 'text', color, size: textPx ?? textSize(size, thickness), at, text, ...textStyle });
    textPx = null;
  }
  function onTextKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      commitText();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      textBox = null;
      textValue = '';
    }
  }
  function onTextBlur(e: FocusEvent) {
    const to = e.relatedTarget as HTMLElement | null;
    if (to?.closest('[data-mark-tools]')) return;
    commitText();
  }
  const textFont = $derived(
    fontOf({ kind: 'text', color, size: (textPx ?? textSize(size, thickness)) * k, at: { x: 0, y: 0 }, text: '', ...textStyle }),
  );
</script>

<div class="layer" class:idle={!active} bind:this={layer}>
  <div class="frame" style:left="{shown.left}px" style:top="{shown.top}px" style:width="{shown.w}px" style:height="{shown.h}px">
    <canvas
      bind:this={canvas}
      class:text={tool === 'text'}
      aria-label="Picture to mark, {doc.annotations.length} {doc.annotations.length === 1 ? 'mark' : 'marks'}"
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
    ></canvas>
    {#if textBox}
      <textarea
        bind:this={input}
        class="text-input"
        class:box={textStyle.box}
        rows={Math.max(1, textValue.split('\n').length)}
        style:left="{textBox.pt.x * k}px"
        style:top="{textBox.pt.y * k}px"
        style:color={color}
        style:font={textFont}
        bind:value={textValue}
        onkeydown={onTextKey}
        onblur={onTextBlur}
        aria-label="Text to add on the recording"
        placeholder="Type, Enter to place"
      ></textarea>
    {/if}
  </div>
</div>

<style>
  .layer { position: absolute; inset: 0; }
  .layer.idle { pointer-events: none; }
  .frame { position: absolute; overflow: hidden; }
  canvas { position: absolute; inset: 0; width: 100%; height: 100%; touch-action: none; cursor: crosshair; }
  canvas.text { cursor: text; }
  .text-input {
    position: absolute; min-width: 120px; max-width: 90%; resize: none; overflow: hidden; padding: 0;
    background: rgb(0 0 0 / 0.35); border: 1px dashed var(--accent); border-radius: 3px;
    line-height: 1.25; white-space: pre; z-index: 2; field-sizing: content;
  }
  .text-input.box { background: rgb(255 255 255 / 0.85); }
</style>
