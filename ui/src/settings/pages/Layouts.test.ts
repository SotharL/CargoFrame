import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Layouts from './Layouts.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { Layout } from '../../lib/types';

let api: Api;

beforeEach(() => {
  api = memoryApi();
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

const names = async () => (await api.listItems<Layout>('layout')).items.map((e) => e.item.name).sort();

async function signedInJira() {
  await api.saveItem('connection', 'work', { name: 'Work Jira', kind: 'jira', enabled: true, site_url: 'https://a.atlassian.net', auth: 'api_token', email: 'me@a.com' });
  await api.setConnectionSecret('work', 'token', 'x');
}

describe('Layouts: templates', () => {
  it('offers the local template when there is no connection, and writes nothing until saved', async () => {
    render(Layouts);
    await fireEvent.click(await screen.findByRole('button', { name: 'Start from a template' }));
    expect(await screen.findByText('Save a capture locally')).toBeTruthy();
    expect(screen.queryByText('Jira bug report')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Use Save a capture locally' }));
    expect(await screen.findByRole('button', { name: 'Save as my layout' })).toBeTruthy();
    expect(screen.getByText(/saving makes your own copy/)).toBeTruthy();
    expect(await names()).toEqual([]);
  });

  it('saving a template makes a copy of it, and using it again makes another', async () => {
    render(Layouts);
    for (let round = 0; round < 2; round++) {
      // The panel closes once a template is chosen, so each round opens it again.
      await fireEvent.click((await screen.findAllByRole('button', { name: 'Start from a template' }))[0]!);
      await fireEvent.click(await screen.findByRole('button', { name: 'Use Save a capture locally' }));
      await fireEvent.click(await screen.findByRole('button', { name: 'Save as my layout' }));
      await waitFor(async () => expect((await names()).length).toBe(round + 1));
    }
    expect(await names()).toEqual(['Save a capture locally', 'Save a capture locally']);
    const ids = (await api.listItems('layout')).items.map((e) => e.id).sort();
    expect(ids).toEqual(['save-a-capture-locally', 'save-a-capture-locally-2']);
  });

  it('builds a template for the signed-in connection and offers it by that connection\'s name', async () => {
    await signedInJira();
    render(Layouts);
    await fireEvent.click(await screen.findByRole('button', { name: 'Start from a template' }));
    expect(await screen.findByText('Work Jira')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Use Jira bug report for Work Jira' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Save as my layout' }));
    await waitFor(async () => expect((await names()).length).toBe(1));
    const saved = (await api.listItems<Layout>('layout')).items[0]!.item;
    const button = saved.blocks.find((b) => b.type === 'sharing_button');
    expect(button && 'connection_id' in button ? button.connection_id : null).toBe('work');
  });

  it('discarding a template leaves nothing behind', async () => {
    render(Layouts);
    await fireEvent.click(await screen.findByRole('button', { name: 'Start from a template' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Use Save a capture locally' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Discard' }));
    expect(screen.queryByRole('button', { name: 'Save as my layout' })).toBeNull();
    expect(await names()).toEqual([]);
  });
});

describe('Layouts: save as copy', () => {
  it('writes a second layout and leaves the first as it was saved', async () => {
    await api.saveItem('layout', 'bug', { id: 'bug', name: 'Bug report', blocks: [] });
    render(Layouts);
    const nameBox = (await screen.findByLabelText('Layout name')) as HTMLInputElement;
    await fireEvent.input(nameBox, { target: { value: 'Bug report v2' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save as copy' }));
    await waitFor(async () => expect((await names()).length).toBe(2));
    expect(await names()).toEqual(['Bug report', 'Bug report v2 (copy)']);
  });
});

describe('Layouts: what a button sends', () => {
  it('shows what the chosen destination can take, what each block does, and greys out what cannot be sent', async () => {
    await api.saveItem('connection', 'rd', { name: 'My Reddit', kind: 'reddit', enabled: true, client_id: 'x', redirect_port: 8080 });
    await api.connectConnection('rd');
    await api.saveItem('layout', 'post', {
      id: 'post', name: 'Post',
      blocks: [
        { id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 6, h: 4 } },
        { id: 'logs', type: 'files', rect: { x: 6, y: 0, w: 6, h: 4 }, title: 'Logs', include: ['*.log'], preselect: [], look_in: { folders: ['project'], subfolder: '' } },
        { id: 'send', type: 'sharing_button', rect: { x: 0, y: 4, w: 6, h: 1 }, label: 'Post it', connection_id: 'rd', prompt_block_id: null, mappings: [], on_success: 'keep_open' },
      ],
    });
    render(Layouts);
    // The layout opens from the picker; the shared canvas then tags what the button cannot send.
    const logs = await screen.findByRole('button', { name: /Files block Logs/ });
    await waitFor(() => expect(logs.classList.contains('greyed')).toBe(true));

    // Focusing a block selects it, the way the keyboard does.
    (await screen.findByRole('button', { name: /Sharing button block Post it/ })).focus();
    expect(await screen.findByText('Reddit can take')).toBeTruthy();
    expect(screen.getAllByText(/Reddit cannot take files on a post/).length).toBeGreaterThan(0);
    expect(screen.getByLabelText('Ticked files')).toHaveProperty('disabled', true);
  });
});
