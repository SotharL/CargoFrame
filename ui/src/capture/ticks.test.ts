import { describe, expect, it } from 'vitest';
import { demoSession } from './demo';
import { demoCapabilities, demoReach } from '../lib/demoReach';
import { boxesFor, buttonLabel, buttonsOf, initialTicks, isTicked, MAX_BUTTONS, setTick } from './ticks';
import type { Block, ServiceKind, SessionView } from '../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

/** The demo capture with a second button, to Reddit, next to the Jira one. */
async function twoButtons(second: ServiceKind | null = 'reddit'): Promise<{ view: SessionView; jira: Button; other: Button }> {
  const view = await demoSession();
  const jira = view.layout!.blocks.find((b): b is Button => b.type === 'sharing_button')!;
  const other: Button = { ...jira, id: 'other', label: 'Post', connection_id: second, prompt_block_id: 'prompt', mappings: [] };
  view.layout!.blocks.push(other);
  view.sharing.other = {
    enabled: true, reasons: [], mode: second ? 'send' : 'save_locally', note: null, kind: second,
    reach: demoReach(view.layout!, other, demoCapabilities(second ?? 'jira'), second),
  };
  return { view, jira, other };
}

describe('the boxes under a block', () => {
  it('start ticked for every block the layout says the button sends', async () => {
    const { view } = await twoButtons();
    const ticks = initialTicks(view);
    // Jira: picture, files, prompt and the two mapped fields; Reddit: picture and prompt only (no files)
    expect(ticks.send!.sort()).toEqual(['logs', 'note', 'proj', 'prompt', 'shot']);
    expect(ticks.other!.sort()).toEqual(['prompt', 'shot']);
  });

  it('are one per button, in the order of the buttons, in the colour of the destination', async () => {
    const { view } = await twoButtons();
    const boxes = boxesFor(view, 'shot', initialTicks(view));
    expect(boxes.map((b) => [b.buttonId, b.kind, b.checked, b.disabled])).toEqual([
      ['send', 'jira', true, false],
      ['other', 'reddit', true, false],
    ]);
  });

  it('are empty and cannot be ticked for what the destination cannot take, and say why', async () => {
    const { view } = await twoButtons();
    const [jira, reddit] = boxesFor(view, 'logs', initialTicks(view));
    expect(jira).toMatchObject({ checked: true, disabled: false });
    expect(reddit).toMatchObject({ checked: false, disabled: true });
    expect(reddit!.title).toBe('Post cannot send this: Reddit cannot take files on a post');
  });

  it('are empty and cannot be ticked for a block the button has no field for, and say what to do', async () => {
    const { view } = await twoButtons();
    const [, reddit] = boxesFor(view, 'note', initialTicks(view));
    expect(reddit).toMatchObject({ checked: false, disabled: true });
    expect(reddit!.title).toBe('Post has no field for this. Set it up in the layout');
  });

  it('can switch on what the button had switched off, because the destination takes it', async () => {
    const { view, jira } = await twoButtons();
    jira.send = { screenshot: false, files: 'none', inline_limit_kb: 64 };
    view.sharing.send!.reach = demoReach(view.layout!, jira, demoCapabilities('jira'), 'jira');
    let ticks = initialTicks(view);
    const [box] = boxesFor(view, 'shot', ticks);
    expect(box).toMatchObject({ checked: false, disabled: false });
    expect(box!.title).toBe('Share leaves this out. Tick to send it this time');
    ticks = setTick(ticks, 'send', 'shot', true);
    expect(isTicked(ticks, 'send', 'shot')).toBe(true);
    expect(boxesFor(view, 'shot', ticks)[0]!.checked).toBe(true);
  });

  it('keep what the destination needs ticked and say so', async () => {
    const { view } = await twoButtons('claude');
    // Claude cannot work without the prompt
    const prompt = boxesFor(view, 'prompt', initialTicks(view));
    expect(prompt.find((b) => b.buttonId === 'other')).toMatchObject({ checked: true, locked: true });
    expect(prompt.find((b) => b.buttonId === 'other')!.title).toContain('required');
    // Jira needs a summary, the first one does not need the prompt
    expect(prompt.find((b) => b.buttonId === 'send')!.locked).toBe(false);
  });

  it('tell two buttons to the same kind of service apart with a number', async () => {
    const { view } = await twoButtons('jira');
    const boxes = boxesFor(view, 'shot', initialTicks(view));
    expect(boxes.map((b) => b.badge)).toEqual(['1', '2']);
    // different services need no number
    const { view: mixed } = await twoButtons('reddit');
    expect(boxesFor(mixed, 'shot', initialTicks(mixed)).map((b) => b.badge)).toEqual([null, null]);
  });

  it('are changed one at a time, for one button, without touching the rest', async () => {
    const { view } = await twoButtons();
    const ticks = initialTicks(view);
    const next = setTick(ticks, 'other', 'shot', false);
    expect(isTicked(next, 'other', 'shot')).toBe(false);
    expect(isTicked(next, 'send', 'shot')).toBe(true);
    expect(isTicked(ticks, 'other', 'shot')).toBe(true); // the old state is left as it was
    expect(isTicked(setTick(next, 'other', 'shot', true), 'other', 'shot')).toBe(true);
  });

  it('exist only for the first five buttons', async () => {
    const view = await demoSession();
    const first = view.layout!.blocks.find((b): b is Button => b.type === 'sharing_button')!;
    for (let i = 0; i < 6; i++) view.layout!.blocks.push({ ...first, id: `extra${i}` });
    expect(MAX_BUTTONS).toBe(5);
    expect(buttonsOf(view)).toHaveLength(5);
    expect(boxesFor(view, 'shot', {})).toHaveLength(5);
  });

  it('call a button that only saves a zip "Save locally"', async () => {
    const { view, other } = await twoButtons(null);
    expect(buttonLabel(view, other)).toBe('Save locally');
    expect(boxesFor(view, 'shot', initialTicks(view))[1]).toMatchObject({ label: 'Save locally', kind: null });
  });
});

describe('boxes of a log viewer', () => {
  it('has none: nothing of it is sent', async () => {
    const { demoSession } = await import('./demo');
    const { initialTicks, boxesFor: boxes } = await import('./ticks');
    const view = await demoSession();
    view.layout!.blocks.push({ id: 'log', type: 'log_view', rect: { x: 0, y: 20, w: 6, h: 3 }, title: 'Log' });
    expect(boxes(view, 'log', initialTicks(view))).toEqual([]);
    expect(boxes(view, 'note', initialTicks(view)).length).toBeGreaterThan(0);
  });
});
