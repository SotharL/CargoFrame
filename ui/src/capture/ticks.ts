// The boxes under the blocks: one per sharing button, ticked when that button sends the block.
// The starting state comes from what the layout says each button sends (the "reach", worked out by
// the same Rust code that does the sharing); the person can clear a box, or tick one the button
// has switched off, for this capture only. A box that cannot be ticked says why.
import type { Block, BlockReach, ServiceKind, SessionView } from '../lib/types';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** A layout shows at most this many sharing buttons, and so at most this many boxes per block. */
export const MAX_BUTTONS = 5;

/** The blocks each sharing button sends, by button id. */
export type Ticks = Record<string, string[]>;

/** How one box looks and behaves. */
export interface TickBox {
  buttonId: string;
  label: string;
  kind: ServiceKind | null;
  checked: boolean;
  /** Needed by the button: it cannot be cleared. */
  locked: boolean;
  /** `cannot` and `unused` boxes are shown empty and cannot be ticked. */
  disabled: boolean;
  /** What a hover or a screen reader says. */
  title: string;
  /** "1", "2"… when several buttons go to the same kind of service, so their boxes are told apart. */
  badge: string | null;
}

export function buttonsOf(view: SessionView): SharingBlock[] {
  return (view.layout?.blocks ?? []).filter((b): b is SharingBlock => b.type === 'sharing_button').slice(0, MAX_BUTTONS);
}

function reachOf(view: SessionView, buttonId: string, blockId: string): BlockReach | undefined {
  return view.sharing[buttonId]?.reach?.blocks.find((r) => r.block_id === blockId);
}

/** Every block the layout says each button sends, as the capture starts. */
export function initialTicks(view: SessionView): Ticks {
  const out: Ticks = {};
  for (const button of buttonsOf(view)) {
    out[button.id] = (view.sharing[button.id]?.reach?.blocks ?? []).filter((r) => r.state === 'sent').map((r) => r.block_id);
  }
  return out;
}

export function isTicked(ticks: Ticks, buttonId: string, blockId: string): boolean {
  return ticks[buttonId]?.includes(blockId) ?? false;
}

/** The boxes after one was ticked or cleared. */
export function setTick(ticks: Ticks, buttonId: string, blockId: string, on: boolean): Ticks {
  const rest = (ticks[buttonId] ?? []).filter((id) => id !== blockId);
  return { ...ticks, [buttonId]: on ? [...rest, blockId] : rest };
}

/** What a button says it is called: a button that only saves a zip reads "Save locally". */
export function buttonLabel(view: SessionView, button: SharingBlock): string {
  return view.sharing[button.id]?.mode === 'save_locally' ? 'Save locally' : button.label || 'Share';
}

function describe(r: BlockReach | undefined, label: string, on: boolean): { title: string; disabled: boolean } {
  if (!r) return { title: `${label} does not use this block`, disabled: true };
  switch (r.state) {
    case 'sent':
      if (r.role === 'field') return { title: `${label} ${on ? 'sends this' : 'leaves this out'} (the “${r.field}” field)`, disabled: false };
      if (r.role === 'prompt') return { title: `${label} ${on ? 'sends this as the prompt' : 'leaves out the prompt'}`, disabled: false };
      return { title: `${label} ${on ? 'sends this' : 'leaves this out'}`, disabled: false };
    case 'switched_off':
      return { title: `${label} leaves this out. Tick to send it this time`, disabled: false };
    case 'cannot_send':
      return { title: `${label} cannot send this: ${r.why}`, disabled: true };
    default:
      return { title: `${label} has no field for this. Set it up in the layout`, disabled: true };
  }
}

/** The room the dots of a block take at the end of its first row, in px (TickBoxes draws 18 px each). */
export const cornerWidth = (boxes: readonly TickBox[]) => (boxes.length ? boxes.length * 18 + 2 : 0);

/** The boxes of one block, in the order of the buttons. Empty for a block no button could ever use. */
export function boxesFor(view: SessionView, blockId: string, ticks: Ticks): TickBox[] {
  // a log viewer only shows, and tabs only hold other blocks: nothing of them is ever sent
  const type = view.layout?.blocks.find((b) => b.id === blockId)?.type;
  if (type === 'log_view' || type === 'tabs') return [];
  const buttons = buttonsOf(view);
  const kinds = buttons.map((b) => view.sharing[b.id]?.kind ?? null);
  return buttons.map((button, i) => {
    const r = reachOf(view, button.id, blockId);
    const label = buttonLabel(view, button);
    const kind = kinds[i]!;
    const on = isTicked(ticks, button.id, blockId);
    const { title, disabled } = describe(r, label, on);
    const same = kinds.filter((k) => k === kind).length > 1;
    const locked = r?.state === 'sent' && r.required === true && on;
    return {
      buttonId: button.id,
      label,
      kind,
      checked: on,
      locked,
      disabled,
      title: locked ? `${title}. It is required, so it stays` : title,
      badge: same ? String(kinds.slice(0, i + 1).filter((k) => k === kind).length) : null,
    };
  });
}
