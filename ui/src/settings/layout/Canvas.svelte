<script lang="ts">
  import { roleTag } from '../../lib/roles';
  import { buttonKind, kindStyle } from '../../lib/connector';
  import type { Block, ConnectionView, IssueView, Reach, Rect } from '../../lib/types';
  import { lineFor, type Line } from './reach';
  import {
    BLOCK_TYPES, COLUMNS, canPlace, makeBlock, moveTo, removeBlock, resizeTo, rowsNeeded, typeLabel,
    type BlockType,
  } from './model';

  let {
    blocks = $bindable(),
    selected = $bindable(null),
    issues = [],
    reach = {},
    connections = [],
  }: {
    blocks: Block[];
    selected: string | null;
    issues?: IssueView[];
    /** What each sharing button sends, by button id. */
    reach?: Record<string, Reach>;
    connections?: ConnectionView[];
  } = $props();

  const ROW = 40;
  const GAP = 8;

  let grid = $state<HTMLDivElement>();
  const rows = $derived(rowsNeeded(blocks));

  // What is being dragged right now, if anything.
  type Drag =
    | { kind: 'move' | 'resize'; id: string; startX: number; startY: number; origin: Rect }
    | { kind: 'new'; type: BlockType; moved: boolean };
  let drag = $state<Drag | null>(null);
  let preview = $state<{ rect: Rect; valid: boolean } | null>(null);
  let pointer = $state({ x: 0, y: 0 });

  function pitch() {
    const box = grid!.getBoundingClientRect();
    const cellW = (box.width - GAP * (COLUMNS - 1)) / COLUMNS;
    return { box, x: cellW + GAP, y: ROW + GAP, cellW };
  }

  function clampRect(r: Rect): Rect {
    const w = Math.min(Math.max(1, r.w), COLUMNS);
    return { x: Math.min(Math.max(0, r.x), COLUMNS - w), y: Math.max(0, r.y), w, h: Math.max(1, r.h) };
  }

  function begin(e: PointerEvent, id: string, kind: 'move' | 'resize') {
    if (e.button !== 0) return;
    const b = blocks.find((k) => k.id === id);
    if (!b) return;
    e.preventDefault();
    e.stopPropagation();
    selected = id;
    drag = { kind, id, startX: e.clientX, startY: e.clientY, origin: { ...b.rect } };
    preview = { rect: { ...b.rect }, valid: true };
    pointer = { x: e.clientX, y: e.clientY };
    listen();
  }

  function beginNew(e: PointerEvent, type: BlockType) {
    if (e.button !== 0) return;
    drag = { kind: 'new', type, moved: false };
    pointer = { x: e.clientX, y: e.clientY };
    preview = null;
    listen();
  }

  function listen() {
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp, { once: true });
    window.addEventListener('pointercancel', cancel, { once: true });
  }
  function unlisten() {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    window.removeEventListener('pointercancel', cancel);
  }

  function onMove(e: PointerEvent) {
    if (!drag || !grid) return;
    pointer = { x: e.clientX, y: e.clientY };
    const p = pitch();
    if (drag.kind === 'new') {
      drag.moved = true;
      const wanted = drag.type;
      const def = BLOCK_TYPES.find((b) => b.type === wanted)!;
      const inside =
        e.clientX >= p.box.left && e.clientX <= p.box.right && e.clientY >= p.box.top && e.clientY <= p.box.bottom;
      if (!inside) {
        preview = null;
        return;
      }
      const [w, h] = def.size;
      const x = Math.floor((e.clientX - p.box.left) / p.x);
      const y = Math.floor((e.clientY - p.box.top) / p.y);
      const rect = clampRect({ x, y, w, h });
      preview = { rect, valid: canPlace(blocks, rect) };
      return;
    }
    const dx = Math.round((e.clientX - drag.startX) / p.x);
    const dy = Math.round((e.clientY - drag.startY) / p.y);
    const o = drag.origin;
    const rect = drag.kind === 'move'
      ? clampRect({ ...o, x: o.x + dx, y: o.y + dy })
      : clampRect({ ...o, w: Math.min(o.w + dx, COLUMNS - o.x), h: o.h + dy });
    preview = { rect, valid: canPlace(blocks, rect, drag.id) };
  }

  function onUp() {
    const d = drag;
    const pv = preview;
    finish();
    if (!d) return;
    if (d.kind === 'new') {
      if (!d.moved) {
        // A click: put it in the first free spot.
        addAt(d.type);
      } else if (pv?.valid) {
        const b = makeBlock(blocks, d.type, { x: pv.rect.x, y: pv.rect.y });
        if (b) {
          blocks = [...blocks, b];
          selected = b.id;
        }
      }
      return;
    }
    if (pv?.valid) {
      const next = d.kind === 'move' ? moveTo(blocks, d.id, pv.rect.x, pv.rect.y) : resizeTo(blocks, d.id, pv.rect.w, pv.rect.h);
      if (next) blocks = next;
    }
  }

  function cancel() {
    finish();
  }
  function finish() {
    unlisten();
    drag = null;
    preview = null;
  }

  export function addAt(type: BlockType) {
    const b = makeBlock(blocks, type);
    if (b) {
      blocks = [...blocks, b];
      selected = b.id;
    }
  }

  function onKey(e: KeyboardEvent, b: Block) {
    const arrows: Record<string, [number, number]> = {
      ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1],
    };
    const step = arrows[e.key];
    if (step) {
      e.preventDefault();
      const next = e.shiftKey
        ? resizeTo(blocks, b.id, b.rect.w + step[0], b.rect.h + step[1])
        : moveTo(blocks, b.id, b.rect.x + step[0], b.rect.y + step[1]);
      if (next) blocks = next;
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      blocks = removeBlock(blocks, b.id);
      selected = null;
    } else if (e.key === 'Escape') {
      selected = null;
    }
  }

  function label(b: Block): string {
    return b.type === 'sharing_button' ? b.label || 'Share' : b.type === 'screenshot' ? 'Screenshot' : b.title || typeLabel(b.type);
  }

  function worst(id: string): 'error' | 'warning' | 'debug' | null {
    const mine = issues.filter((i) => i.block_id === id);
    if (mine.some((i) => i.severity === 'error')) return 'error';
    if (mine.some((i) => i.severity === 'warning')) return 'warning';
    return mine.length ? 'debug' : null;
  }
  const count = (id: string) => issues.filter((i) => i.block_id === id).length;

  // The button whose reach the canvas shows: the selected one, or the only one there is. What it
  // cannot send is greyed out and says why, so the limit is seen where the block is.
  type Button = Extract<Block, { type: 'sharing_button' }>;
  const buttons = $derived(blocks.filter((b): b is Button => b.type === 'sharing_button'));
  const shown = $derived.by((): Button | null => {
    const pick = buttons.find((b) => b.id === selected);
    if (pick) return pick;
    return buttons.length === 1 ? buttons[0]! : null;
  });
  const kindOf = (b: Button) => buttonKind(b.connection_id, connections);
  const lineOf = (b: Block): Line | null => {
    if (!shown || b.type === 'sharing_button') return null;
    const r = reach[shown.id]?.blocks.find((x) => x.block_id === b.id);
    return r ? lineFor(r, shown, kindOf(shown)) : null;
  };
  /** The short tag on the block; the whole sentence is in its tooltip and in the side panel. */
  const tag = (l: Line) => (l.state === 'cannot_send' ? '✕ Cannot send' : l.state === 'sent' ? '✓ Sent' : l.state === 'switched_off' ? '– Left out' : '– Not used');

  const gridStyle = $derived(
    `grid-template-columns:repeat(${COLUMNS},minmax(0,1fr));grid-template-rows:repeat(${rows},${ROW}px);gap:${GAP}px;`,
  );
  const cell = (r: Rect) => `grid-column:${r.x + 1} / span ${r.w};grid-row:${r.y + 1} / span ${r.h};`;
  const ghostType = $derived.by(() => {
    const d = drag;
    return d?.kind === 'new' ? BLOCK_TYPES.find((b) => b.type === d.type) : undefined;
  });
</script>

<div class="canvas">
  <div class="palette" role="toolbar" aria-label="Add a block">
    <span class="quiet">Add:</span>
    {#each BLOCK_TYPES as t (t.type)}
      <button
        type="button"
        class="chip"
        title="{t.hint}. Click to add, or drag onto the grid."
        onpointerdown={(e) => beginNew(e, t.type)}
        onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); addAt(t.type); } }}
      >＋ {t.label}</button>
    {/each}
  </div>

  <div class="scroll">
    <div class="grid" bind:this={grid} style={gridStyle} class:dragging={drag !== null}>
      {#each blocks as b (b.id)}
        {@const w = worst(b.id)}
        {@const l = lineOf(b)}
        <div
          class="block {b.type}"
          class:sharing={b.type === 'sharing_button'}
          class:greyed={l?.state === 'cannot_send' || l?.state === 'switched_off'}
          class:unused={l?.state === 'not_used'}
          class:selected={selected === b.id}
          class:lifted={drag && drag.kind !== 'new' && drag.id === b.id}
          class:err={w === 'error'}
          class:warn={w === 'warning'}
          class:dbg={w === 'debug'}
          style={cell(b.rect) + (b.type === 'sharing_button' ? kindStyle(kindOf(b)) : '')}
          role="button"
          tabindex="0"
          aria-label="{typeLabel(b.type)} block {label(b)}, column {b.rect.x + 1}, row {b.rect.y + 1}, {b.rect.w} by {b.rect.h}. Arrow keys move it, Shift with arrows resizes it, Delete removes it.{l ? ` ${l.text}.` : ''}"
          aria-pressed={selected === b.id}
          onpointerdown={(e) => begin(e, b.id, 'move')}
          onfocus={() => (selected = b.id)}
          onkeydown={(e) => onKey(e, b)}
        >
          <!-- One line, so a block a single row high still shows its title: the title shrinks first,
               the small (type) after it and the badges on the right stay visible. -->
          <div class="line">
            <span class="name" title={label(b)}>{label(b)}</span>
            <span class="kind">({typeLabel(b.type).toLowerCase()})</span>
            {#if b.type === 'text' && roleTag(b.role)}<span class="tag">{roleTag(b.role)}</span>{/if}
            {#if l}<span class="reach {l.state}" title={l.text}>{tag(l)}</span>{/if}
            {#if count(b.id)}<span class="count {w}" title="{count(b.id)} to look at">{w === 'error' ? '✕' : w === 'warning' ? '!' : '?'} {count(b.id)}</span>{/if}
          </div>
          <button
            type="button"
            class="handle"
            tabindex="-1"
            aria-label="Resize {label(b)}"
            onpointerdown={(e) => begin(e, b.id, 'resize')}
          ></button>
        </div>
      {/each}
      {#if preview}
        <div class="ghost" class:bad={!preview.valid} style={cell(preview.rect)} aria-hidden="true"></div>
      {/if}
    </div>
  </div>

  {#if drag?.kind === 'new' && drag.moved && ghostType}
    <div class="follow" style="left:{pointer.x + 12}px;top:{pointer.y + 12}px" aria-hidden="true">{ghostType.label}</div>
  {/if}
</div>

<style>
  .canvas { display: grid; gap: 8px; min-width: 0; }
  .palette { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .chip { padding: 3px 10px; border-radius: 999px; font-size: 13px; touch-action: none; }
  .scroll { overflow: auto; border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface-2); padding: 10px; }
  .grid {
    display: grid; position: relative; min-width: 560px;
    /* Faint cell guides, so it is clear where blocks can go. */
    background-image:
      linear-gradient(to right, color-mix(in srgb, var(--text) 6%, transparent) 1px, transparent 1px),
      linear-gradient(to bottom, color-mix(in srgb, var(--text) 6%, transparent) 1px, transparent 1px);
    background-size: calc((100% + 8px) / 12) 48px;
    user-select: none;
  }
  .grid.dragging { cursor: grabbing; }
  .block {
    position: relative; display: flex; align-items: flex-start; gap: 2px;
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--radius-sm);
    padding: 6px 8px; overflow: hidden; cursor: grab; touch-action: none; min-width: 0;
  }
  .block.selected { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent); }
  .block.lifted { opacity: 0.45; }
  /* A sharing button wears its destination's colour and outline; the label says where. */
  .block.sharing { background: var(--kind); color: var(--on-kind); border: 2px solid var(--kind-line); }
  .block.sharing .kind { color: inherit; opacity: 0.85; }
  .block.greyed { opacity: 0.55; border-style: dashed; background: var(--surface-2); }
  .block.unused { border-style: dashed; }
  .reach { flex: none; margin-left: auto; font-size: 10.5px; font-weight: 600; padding: 0 6px; border-radius: 999px; border: 1px solid var(--border-strong); white-space: nowrap; }
  .reach.sent { border-color: var(--ok); }
  .reach.cannot_send { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  .reach.switched_off, .reach.not_used { color: var(--quiet); }
  .reach + .count { margin-left: 0; }
  .block.err { border-color: var(--err); }
  .block.warn { border-color: var(--warn); }
  .block.dbg { border-color: var(--dbg); }
  .block.selected.err, .block.selected.warn, .block.selected.dbg { box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent); }
  .line { display: flex; gap: 6px; align-items: baseline; min-width: 0; width: 100%; padding-right: 10px; }
  .kind { font-size: 10px; color: var(--quiet); flex: none; white-space: nowrap; }
  .tag {
    flex: none; font-size: 11px; font-weight: 600; padding: 0 6px; border-radius: 999px; border: 1px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .count { flex: none; margin-left: auto; font-size: 11px; font-weight: 700; padding: 0 6px; border-radius: 999px; border: 1px solid var(--quiet); }
  .count.error { border-color: var(--err); background: color-mix(in srgb, var(--err) 16%, transparent); }
  .count.warning { border-color: var(--warn); background: color-mix(in srgb, var(--warn) 16%, transparent); }
  .count.debug { border-color: var(--dbg); background: color-mix(in srgb, var(--dbg) 16%, transparent); }
  .name { font-weight: 600; flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .screenshot { background: repeating-linear-gradient(45deg, var(--surface), var(--surface) 10px, var(--surface-2) 10px, var(--surface-2) 20px); }
  .handle {
    position: absolute; right: 0; bottom: 0; width: 16px; height: 16px; padding: 0; border: 0;
    border-radius: 0; background: transparent; cursor: nwse-resize; touch-action: none;
  }
  .handle::after {
    content: ''; position: absolute; right: 3px; bottom: 3px; width: 8px; height: 8px;
    border-right: 2px solid var(--quiet); border-bottom: 2px solid var(--quiet);
  }
  .ghost {
    pointer-events: none; border: 2px dashed var(--accent); border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .ghost.bad { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  .follow {
    position: fixed; pointer-events: none; z-index: 10; padding: 2px 10px; border-radius: 999px;
    background: var(--accent); color: var(--on-accent); font-weight: 600; font-size: 12px;
  }
</style>
