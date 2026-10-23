import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Connections from './Connections.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';

let api: Api;

beforeEach(() => {
  api = memoryApi();
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

async function addJira() {
  render(Connections);
  await fireEvent.click(await screen.findByRole('button', { name: /Jira/ }));
  await fireEvent.input(screen.getByLabelText('Jira address'), { target: { value: 'https://acme.atlassian.net' } });
  await fireEvent.input(screen.getByLabelText('Account e-mail'), { target: { value: 'me@example.com' } });
  await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
}

describe('Connections page', () => {
  it('saves a connection (a live form value, not a plain object) and asks for its token', async () => {
    await addJira();
    expect(await screen.findByText('Saved. Now add the token below.')).toBeTruthy();
    expect(screen.getByText('Needs its token')).toBeTruthy();
    const saved = await api.listItems('connection');
    expect(saved.items).toHaveLength(1);
  });

  it('stores the token and never shows it again; the connection becomes ready', async () => {
    await addJira();
    const box = await screen.findByPlaceholderText('Paste it here');
    await fireEvent.input(box, { target: { value: 'tok-secret' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save API token' }));
    expect(await screen.findByText("Stored in your system's password store")).toBeTruthy();
    expect(screen.queryByDisplayValue('tok-secret')).toBeNull();
    expect(document.body.textContent).not.toContain('tok-secret');
    const view = await api.listConnections();
    expect(view.items[0]!.secrets.every((s) => s.stored)).toBe(true);
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByText('Connected')).toBeTruthy();
  });

  it('pressing Enter in the token box stores the token instead of saving the connection form', async () => {
    await addJira();
    const box = await screen.findByPlaceholderText('Paste it here');
    await fireEvent.input(box, { target: { value: 'tok-2' } });
    await fireEvent.keyDown(box, { key: 'Enter' });
    expect(await screen.findByText("Stored in your system's password store")).toBeTruthy();
  });

  it('keeps Claude chats on the page once a Claude connection exists', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Claude/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByText('Claude chats')).toBeTruthy());
  });
});
