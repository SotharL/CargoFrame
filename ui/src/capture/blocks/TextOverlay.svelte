<script lang="ts">
  // A text laid over the recording (the keys pressed, the time): black letters with a white
  // outline, as the sent clip has them. Dragged (or moved with the arrow keys) where it goes,
  // and taken off with its × (or Delete). Where it is, as parts of the picture, is bound.
  import { KEY_SIZE } from './keyLayer';

  let {
    text,
    placeholder,
    name,
    at = $bindable(),
    shown,
    hot = false,
    onremove,
  }: {
    /** What shows now; `null` between two lines. */
    text: string | null;
    /** What marks the place while nothing shows (seen only with the mouse over the picture). */
    placeholder: string;
    /** What it is, for the screen reader and the × ("the keys", "the time"). */
    name: string;
    at: { fx: number; fy: number };
    /** The picture's size on screen. */
    shown: { w: number; h: number };
    hot?: boolean;
    onremove: () => void;
  } = $props();

  let box = $state<HTMLElement | null>(null);
  let dragFrom: { x: number; y: number; fx: number; fy: number } | null = null;

  function down(e: PointerEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    dragFrom = { x: e.clientX, y: e.clientY, ...at };
  }
  function move(e: PointerEvent) {
    if (!dragFrom || shown.w <= 0) return;
    const r = box?.getBoundingClientRect();
    const maxX = Math.max(0, 1 - (r?.width ?? 0) / shown.w);
    const maxY = Math.max(0, 1 - (r?.height ?? 0) / shown.h);
    at = {
      fx: Math.min(maxX, Math.max(0, dragFrom.fx + (e.clientX - dragFrom.x) / shown.w)),
      fy: Math.min(maxY, Math.max(0, dragFrom.fy + (e.clientY - dragFrom.y) / shown.h)),
    };
  }
  const up = () => (dragFrom = null);
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.05 : 0.01;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const m = moves[e.key];
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      onremove();
    } else if (m) {
      e.preventDefault();
      e.stopPropagation();
      at = { fx: Math.min(0.97, Math.max(0, at.fx + m[0])), fy: Math.min(0.95, Math.max(0, at.fy + m[1])) };
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="over"
  class:empty={!text}
  class:hot
  bind:this={box}
  style:left="{at.fx * shown.w}px"
  style:top="{at.fy * shown.h}px"
  style:font-size="{Math.max(9, shown.h * KEY_SIZE)}px"
  role="group"
  tabindex="0"
  aria-label={`${name[0]?.toUpperCase()}${name.slice(1)} over the recording: drag, or use the arrow keys, to move it; Delete takes it off`}
  title="Drag to move"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
>
  <span class="line">{text ?? placeholder}</span>
  <button type="button" class="off" aria-label={`Take ${name} off the recording`} title="Take it off" onclick={onremove}>×</button>
</div>

<style>
  /* black letters with a white outline, as the sent clip has them */
  .over {
    position: absolute; display: inline-flex; align-items: center; gap: 0.35em; padding: 0.1em 0.25em; line-height: 1.4;
    font-weight: 700; font-family: system-ui, 'Segoe UI', 'DejaVu Sans', sans-serif; color: #000; white-space: pre;
    cursor: grab; touch-action: none; user-select: none; border-radius: 4px; border: 1px dashed transparent;
    text-shadow: -0.08em -0.08em 0 #fff, 0.08em -0.08em 0 #fff, -0.08em 0.08em 0 #fff, 0.08em 0.08em 0 #fff,
      0 -0.11em 0 #fff, 0 0.11em 0 #fff, -0.11em 0 0 #fff, 0.11em 0 0 #fff;
  }
  .over:active { cursor: grabbing; }
  .over.hot, .over:focus-visible { border-color: rgb(255 255 255 / 0.7); }
  /* where it goes, while nothing shows: only when the mouse is over the picture */
  .over.empty .line { opacity: 0; }
  .over.empty.hot .line, .over.empty:focus-visible .line { opacity: 0.45; }
  .off {
    font: 600 12px/1 system-ui, sans-serif; padding: 1px 4px; border-radius: 999px; text-shadow: none; opacity: 0;
    background: rgb(0 0 0 / 0.6); color: #fff; border: 0; cursor: pointer;
  }
  .over.hot .off, .over:focus-within .off { opacity: 1; }
</style>
