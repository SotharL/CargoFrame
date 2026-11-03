import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/svelte';
import Canvas from './Canvas.svelte';
import type { Block, ConnectionView, Reach } from '../../lib/types';
import { demoCapabilities } from '../../lib/demoReach';

const blocks: Block[] = [
  { id: 'desc', type: 'text', rect: { x: 0, y: 0, w: 3, h: 1 }, title: 'Description of the problem seen', placement: 'above', role: 'prompt', default_text: '' },
  { id: 'send', type: 'sharing_button', rect: { x: 3, y: 0, w: 3, h: 1 }, label: 'Send', connection_id: null, prompt_block_id: null, mappings: [], on_success: 'close_window' },
];

describe('Layout canvas', () => {
  afterEach(cleanup);

  it('a block one row high keeps its title and its small (type) on one line', () => {
    render(Canvas, { blocks, selected: null });
    const lines = [...document.querySelectorAll('.block .line')];
    expect(lines).toHaveLength(2);
    expect(lines[0]!.querySelector('.name')?.textContent).toBe('Description of the problem seen');
    expect(lines[0]!.querySelector('.kind')?.textContent).toBe('(text)');
    expect(lines[0]!.querySelector('.tag')?.textContent).toBe('Prompt');
    expect(lines[1]!.querySelector('.name')?.textContent).toBe('Send');
    expect(lines[1]!.querySelector('.kind')?.textContent).toBe('(sharing button)');
  });
});

describe('Layout canvas: what the button sends', () => {
  afterEach(cleanup);

  const layout: Block[] = [
    { id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 4, h: 2 } },
    { id: 'logs', type: 'files', rect: { x: 4, y: 0, w: 4, h: 2 }, title: 'Logs', include: [], preselect: [], look_in: { folders: ['project'], subfolder: '' } },
    { id: 'note', type: 'text', rect: { x: 0, y: 2, w: 4, h: 1 }, title: 'Note', placement: 'above', role: 'plain', default_text: '' },
    { id: 'send', type: 'sharing_button', rect: { x: 4, y: 2, w: 4, h: 1 }, label: 'Post', connection_id: 'r', prompt_block_id: null, mappings: [], on_success: 'keep_open' },
  ];
  const reach = {
    send: {
      capabilities: demoCapabilities('reddit'),
      blocks: [
        { block_id: 'shot', state: 'sent', role: 'screenshot' },
        { block_id: 'logs', state: 'cannot_send', why: 'Reddit cannot take files on a post' },
        { block_id: 'note', state: 'not_used' },
      ],
    },
  } satisfies Record<string, Reach>;
  const connections: ConnectionView[] = [
    { id: 'r', name: 'Reddit', kind: 'reddit', kind_label: 'Reddit', enabled: true, state: 'connected', secrets: [], describe: { summary: '', fields: [], lists: [] }, capabilities: demoCapabilities('reddit') },
  ];

  it('greys out a block the destination cannot take and says why', () => {
    render(Canvas, { blocks: layout, selected: null, reach, connections });
    const logs = [...document.querySelectorAll('.block')].find((b) => b.textContent!.includes('Logs'))!;
    expect(logs.classList.contains('greyed')).toBe(true);
    expect(logs.querySelector('.reach')?.textContent).toBe('✕ Cannot send');
    expect(logs.querySelector('.reach')?.getAttribute('title')).toBe('Cannot be sent: Reddit cannot take files on a post');
    expect(logs.getAttribute('aria-label')).toContain('Cannot be sent: Reddit cannot take files on a post');
  });

  it('tags what is sent and what is not used, without greying the first', () => {
    render(Canvas, { blocks: layout, selected: null, reach, connections });
    const shot = [...document.querySelectorAll('.block')].find((b) => b.textContent!.includes('Screenshot'))!;
    expect(shot.classList.contains('greyed')).toBe(false);
    expect(shot.querySelector('.reach')?.textContent).toBe('✓ Sent');
    const note = [...document.querySelectorAll('.block')].find((b) => b.textContent!.includes('Note'))!;
    expect(note.classList.contains('unused')).toBe(true);
    expect(note.querySelector('.reach')?.textContent).toBe('– Not used');
  });

  it('gives the sharing button its destination colour', () => {
    render(Canvas, { blocks: layout, selected: null, reach, connections });
    const button = document.querySelector('.block.sharing')!;
    expect(button.getAttribute('style')).toContain('var(--kind-reddit)');
    expect(button.querySelector('.reach')).toBeNull();
  });

  it('shows nothing about reach when two buttons are in play and none is selected', () => {
    const two: Block[] = [...layout, { ...layout[3]!, id: 'send2', rect: { x: 0, y: 3, w: 4, h: 1 } } as Block];
    render(Canvas, { blocks: two, selected: null, reach, connections });
    expect(document.querySelectorAll('.reach')).toHaveLength(0);
  });

  it('follows the selected button when there are two', () => {
    const two: Block[] = [...layout, { ...layout[3]!, id: 'send2', rect: { x: 0, y: 3, w: 4, h: 1 } } as Block];
    render(Canvas, { blocks: two, selected: 'send', reach, connections });
    expect(document.querySelectorAll('.block.greyed')).toHaveLength(1);
  });

  it('shows a button with no active connection in the colour of this computer', () => {
    const off = connections.map((c) => ({ ...c, state: 'disconnected' as const }));
    render(Canvas, { blocks: layout, selected: null, reach, connections: off });
    expect(document.querySelector('.block.sharing')!.getAttribute('style')).toContain('var(--kind-local)');
  });
});
