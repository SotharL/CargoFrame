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

  it('adds a Teams connection: app registration steps, the app id, then a browser sign-in', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Teams/ }));
    // The steps for registering the app are on the form, with what to give it.
    const steps = screen.getByText('How to register the app').closest('details')!;
    for (const word of ['http://localhost', 'ChatMessage.Send', 'ChannelMessage.Send', 'Chat.ReadBasic', 'public client flows']) {
      expect(steps.textContent).toContain(word);
    }
    // There is no token to paste: the person signs in with the browser.
    expect(screen.queryByText('Needs its token')).toBeNull();
    const id = screen.getByLabelText('Application (client) ID');
    await fireEvent.input(id, { target: { value: '3f2a1b4c-5d6e-4f70-8a9b-0c1d2e3f4a5b' } });
    expect((screen.getByLabelText('Directory') as HTMLInputElement).value).toBe('organizations');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Not signed in')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Connected')).toBeTruthy();
    const saved = await api.listItems('connection');
    expect(saved.items[0]!.item).toMatchObject({
      kind: 'teams',
      name: 'Teams',
      client_id: '3f2a1b4c-5d6e-4f70-8a9b-0c1d2e3f4a5b',
      tenant: 'organizations',
    });
  });

  it('adds a Discord connection with a webhook: no bot, a link per channel, then it is ready', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Discord/ }));
    // The easy way is the one shown first, with the steps to make a webhook.
    const steps = screen.getByText('How to make a webhook').closest('details')!;
    for (const word of ['Copy Webhook URL', 'Manage Webhooks', 'Forums work too']) {
      expect(steps.textContent).toContain(word);
    }
    expect(screen.queryByLabelText('Bot token')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Add a channel' }));
    await fireEvent.input(screen.getByPlaceholderText('bugs'), { target: { value: 'bugs' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved. Now add a channel and paste its webhook link.')).toBeTruthy();
    expect(screen.getByText('Needs a webhook link')).toBeTruthy();

    const box = await screen.findByLabelText('Webhook link');
    await fireEvent.input(box, { target: { value: 'https://discord.com/api/webhooks/1/secret-part' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save Webhook link' }));
    expect(await screen.findByText("Stored in your system's password store")).toBeTruthy();
    expect(document.body.textContent).not.toContain('secret-part');
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByText('Connected')).toBeTruthy();
    const saved = await api.listItems('connection');
    expect(saved.items[0]!.item).toMatchObject({
      kind: 'discord',
      name: 'Discord',
      enabled: true,
      mode: 'webhook',
      hooks: [{ slot: 0, name: 'bugs' }],
    });
  });

  it('adds a Discord bot: the steps, a bot token, then it is ready', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Discord/ }));
    await fireEvent.click(screen.getByRole('radio', { name: 'Bot (advanced)' }));
    const steps = screen.getByText('How to get the token').closest('details')!;
    for (const word of ['Reset Token', 'Attach Files', 'Create Public', 'URL Generator']) {
      expect(steps.textContent).toContain(word);
    }
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved. Now add the token below.')).toBeTruthy();
    const box = await screen.findByLabelText('Bot token');
    await fireEvent.input(box, { target: { value: 'discord-secret' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save Bot token' }));
    expect(await screen.findByText("Stored in your system's password store")).toBeTruthy();
    expect(document.body.textContent).not.toContain('discord-secret');
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByText('Connected')).toBeTruthy();
    const saved = await api.listItems('connection');
    expect(saved.items[0]!.item).toMatchObject({ kind: 'discord', name: 'Discord', enabled: true, mode: 'bot' });
  });

  it('adds a Slack connection: setup steps, then a bot token, then it is ready', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Slack/ }));
    // The steps for getting a token are on the form, with the scopes to give the app.
    const steps = screen.getByText('How to get the token').closest('details')!;
    for (const scope of ['chat:write', 'files:write', 'channels:read', 'groups:read']) {
      expect(steps.textContent).toContain(scope);
    }
    expect(steps.textContent).toContain('/invite @');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved. Now add the token below.')).toBeTruthy();
    expect(screen.getByText('Needs its token')).toBeTruthy();
    const box = await screen.findByLabelText('Bot token (starts with xoxb-)');
    await fireEvent.input(box, { target: { value: 'xoxb-secret' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save Bot token (starts with xoxb-)' }));
    expect(await screen.findByText("Stored in your system's password store")).toBeTruthy();
    expect(document.body.textContent).not.toContain('xoxb-secret');

    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByText('Connected')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Test' })).toBeTruthy();
    const saved = await api.listItems('connection');
    expect(saved.items[0]!.item).toMatchObject({ kind: 'slack', name: 'Slack', enabled: true });
  });
  it('puts a button to the page where each secret is obtained beside its box', async () => {
    const opened: string[] = [];
    api = memoryApi();
    api.openLink = async (url) => void opened.push(url);
    setApiForTests(api);
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Slack/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await screen.findByLabelText('Bot token (starts with xoxb-)');
    await fireEvent.click(screen.getByRole('button', { name: /Open Slack apps/ }));
    expect(opened).toEqual(['https://api.slack.com/apps']);
  });

  it('puts the same button beside an app id', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Teams/ }));
    expect(screen.getByRole('button', { name: /Open app registrations/ })).toBeTruthy();
  });

  it('adds an X connection: the app steps, the Client ID, then a browser sign-in', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^X\b/ }));
    const steps = screen.getByText('How to make the app').closest('details')!;
    for (const word of ['Read and write', 'Native App', 'http://127.0.0.1:53683/callback']) expect(steps.textContent).toContain(word);
    await fireEvent.input(screen.getByLabelText('Client ID'), { target: { value: 'abc123' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Not signed in')).toBeTruthy();
    const saved = await api.listItems('connection');
    expect(saved.items[0]!.item).toMatchObject({ kind: 'x', client_id: 'abc123', redirect_port: 53683 });
  });

  it('adds a Bluesky connection: the handle, then an app password', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Bluesky/ }));
    await fireEvent.input(screen.getByLabelText('Handle'), { target: { value: 'jane.bsky.social' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved. Now add the token below.')).toBeTruthy();
    expect(screen.getByText('App password')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Make an app password/ })).toBeTruthy();
  });

  it('adds an Instagram connection: the two addresses, and the key box only when a header is named', async () => {
    render(Connections);
    await fireEvent.click(await screen.findByRole('button', { name: /^Instagram/ }));
    await fireEvent.input(screen.getByLabelText('Upload address'), { target: { value: 'https://up.example.com/{name}' } });
    await fireEvent.input(screen.getByLabelText('Public address'), { target: { value: 'https://cdn.example.com/{name}' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Access token')).toBeTruthy();
    expect(screen.queryByText('Key of the picture server')).toBeNull();
  });

  it('offers only the services this copy of CargoFrame was built with', async () => {
    setApiForTests(memoryApi({ connectors: ['jira', 'teams'] }));
    render(Connections);
    await screen.findByRole('button', { name: /^Jira/ });
    await waitFor(() => expect(screen.queryByRole('button', { name: /^Bluesky/ })).toBeNull());
    const offered = [...screen.getByRole('region', { name: 'Add a connection' }).querySelectorAll('button strong')].map((b) => b.textContent);
    expect(offered).toEqual(['Jira', 'Teams']);
  });
});

