<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Connection, ConnectionsView, ConnectionView, ServiceKind } from '../../lib/types';
  import ConnectionForm from '../connections/ConnectionForm.svelte';
  import SecretInput from '../connections/SecretInput.svelte';
  import type { HookStore } from '../connections/DiscordForm.svelte';
  import { getLinkFor } from '../../lib/connectorHelp';
  import ClaudeChats from '../connections/ClaudeChats.svelte';

  const items = createItems<Connection>('connection');
  let view = $state<ConnectionsView | null>(null);
  let draft = $state<{ id: string | null; value: Connection } | null>(null);
  let formError = $state('');
  let note = $state('');
  let busy = $state(false);
  let confirming = $state<string | null>(null);
  let signingIn = $state<string | null>(null);
  let messages = $state<Record<string, { kind: 'ok' | 'err'; text: string }>>({});

  const KINDS: { kind: ServiceKind; label: string; blurb: string }[] = [
    { kind: 'jira', label: t('Jira'), blurb: t('Create an issue or comment on one, with the screenshot attached.') },
    { kind: 'github', label: 'GitHub', blurb: t('Open an issue in a repository, or comment on one, with the screenshot shown in it.') },
    { kind: 'google', label: t('Google'), blurb: t('Save the screenshot and files to Drive, and optionally log a row in a sheet.') },
    { kind: 'reddit', label: t('Reddit'), blurb: t('Post the screenshot to a subreddit you choose.') },
    { kind: 'claude', label: t('Claude'), blurb: t('Open a chat and paste the screenshot, the ticked files and your message into it.') },
    { kind: 'slack', label: t('Slack'), blurb: t('Post a message with the screenshot and files into a channel or thread.') },
    { kind: 'discord', label: t('Discord'), blurb: t('Post a message with the screenshot and files into a channel, thread or forum.') },
    { kind: 'teams', label: t('Teams'), blurb: t('Post a message with the screenshot into a chat, channel or channel thread, as you.') },
    { kind: 'x', label: 'X', blurb: t('Post the screenshot with a short text to your X account.') },
    { kind: 'bluesky', label: t('Bluesky'), blurb: t('Post the screenshot with a short text to your Bluesky account. Needs only an app password.') },
    { kind: 'instagram', label: t('Instagram'), blurb: t('Post the screenshot with a caption to your Instagram professional account.') },
    { kind: 'custom', label: t('Custom API'), blurb: t('Send to any web service that has an HTTP API.') },
  ];
  // a company build can leave services out: only the ones this copy has are offered
  let built = $state<ServiceKind[] | null>(null);
  const offered = $derived(built ? KINDS.filter((k) => built!.includes(k.kind)) : KINDS);

  function blank(kind: ServiceKind): Connection {
    switch (kind) {
      case 'jira':
        return { kind, name: 'Jira', enabled: true, site_url: 'https://', auth: 'api_token', email: '' };
      case 'google':
        return { kind, name: t('Google Drive'), enabled: true, client_id: '', sheets: false, full_drive: false };
      case 'reddit':
        return { kind, name: 'Reddit', enabled: true, client_id: '', redirect_port: 53682 };
      case 'claude':
        return { kind, name: 'Claude', enabled: true, open_with: 'auto', send_automatically: true, extra_wait_seconds: 0 };
      case 'slack':
        return { kind, name: 'Slack', enabled: true };
      case 'discord':
        return { kind, name: 'Discord', enabled: true, mode: 'webhook', shown_as: '', hooks: [] };
      case 'teams':
        return { kind, name: 'Teams', enabled: true, client_id: '', tenant: 'organizations' };
      case 'x':
        return { kind, name: 'X', enabled: true, client_id: '', redirect_port: 53683 };
      case 'bluesky':
        return { kind, name: 'Bluesky', enabled: true, handle: '', service: 'https://bsky.social' };
      case 'github':
        return { kind, name: 'GitHub', enabled: true, repository: '', api_url: 'https://api.github.com', files: 'branch', branch: 'cargoframe-attachments' };
      case 'instagram':
        return { kind, name: 'Instagram', enabled: true, upload_url: 'https://', public_url: 'https://', host_header: '' };
      case 'custom':
        return {
          kind, name: t('My API'), enabled: true, base_url: 'https://', auth: { type: 'none' },
          send: { method: 'POST', path: '', headers: [], body: { type: 'json', template: '{\n  "text": "{{prompt}}"\n}' } },
          lists: [], test_path: '', result_url_path: '',
        };
    }
  }

  async function refresh() {
    await items.reload();
    try {
      view = await (await getApi()).listConnections();
    } catch (e) {
      formError = String(e);
    }
  }
  onMount(async () => {
    built = (await (await getApi()).appInfo().catch(() => null))?.connectors ?? null;
    await refresh();
  });

  const viewOf = (id: string): ConnectionView | undefined => view?.items.find((c) => c.id === id);
  const oauth = (k: ServiceKind) => k === 'google' || k === 'reddit' || k === 'teams' || k === 'x';

  function stateChip(c: ConnectionView, saved: Connection): { kind: 'ok' | 'warn' | 'err' | 'dbg'; label: string } {
    switch (c.state) {
      case 'connected': return { kind: 'ok', label: c.kind === 'claude' ? t('Ready') : t('Connected') };
      case 'expired': return { kind: 'err', label: t('Sign-in expired') };
      case 'disabled': return { kind: 'dbg', label: t('Turned off') };
      case 'not_in_build': return { kind: 'dbg', label: t('Not in this copy of CargoFrame') };
      default: return { kind: 'warn', label: oauth(c.kind) ? t('Not signed in') : saved.kind === 'discord' && saved.mode === 'webhook' ? t('Needs a webhook link') : t('Needs its token') };
    }
  }

  function secretLabel(c: Connection, name: string): string {
    if (name === 'client_secret') return t('Client secret');
    if (c.kind === 'jira') return c.auth === 'pat' ? t('Personal access token') : t('API token');
    if (c.kind === 'slack') return t('Bot token (starts with xoxb-)');
    if (c.kind === 'discord') return t('Bot token');
    if (c.kind === 'bluesky') return t('App password');
    if (c.kind === 'github') return t('Personal access token');
    if (c.kind === 'instagram') return name === 'host_key' ? t('Key of the picture server') : t('Access token');
    if (c.kind === 'custom') {
      return { bearer: t('Token'), header: t('Key'), basic: t('Password'), query: t('Key'), none: t('Key') }[c.auth.type];
    }
    return t('Secret');
  }
  const needsSecret = (c: Connection, name: string) => {
    if (c.kind === 'custom' && name === 'secret' && c.auth.type === 'none') return false;
    // Discord: the bot token only for a bot; each webhook link has its own box in the channel list
    if (c.kind === 'discord') return c.mode === 'bot' && name === 'token';
    // the picture server's key only when it says which header carries one
    if (c.kind === 'instagram' && name === 'host_key') return c.host_header.trim() !== '';
    return true;
  };

  async function act(id: string, f: (api: Awaited<ReturnType<typeof getApi>>) => Promise<string | void>) {
    delete messages[id];
    try {
      const text = await f(await getApi());
      if (text) messages[id] = { kind: 'ok', text };
    } catch (e) {
      messages[id] = { kind: 'err', text: String(e) };
    }
    await refresh();
  }

  async function signIn(id: string) {
    signingIn = id;
    await act(id, (api) => api.connectConnection(id));
    signingIn = null;
  }

  function add(kind: ServiceKind) {
    formError = '';
    note = '';
    draft = { id: null, value: blank(kind) };
  }
  function edit(id: string, c: Connection) {
    formError = '';
    note = '';
    draft = { id, value: structuredClone($state.snapshot(c)) as Connection };
  }

  async function save() {
    const d = draft;
    if (!d) return;
    busy = true;
    formError = '';
    note = '';
    try {
      const wasNew = d.id === null;
      const id = await items.save(d.id, d.value);
      await refresh();
      const v = viewOf(id);
      const wantsSecret = v?.secrets.some((s) => !s.stored && needsSecret(d.value, s.name));
      const wantsLinks = d.value.kind === 'discord' && d.value.mode === 'webhook';
      if (wasNew && (wantsSecret || wantsLinks)) {
        // Stay open: the token or link can only be stored once the connection exists.
        d.id = id;
        note = wantsLinks ? t('Saved. Now add a channel and paste its webhook link.') : t('Saved. Now add the token below.');
      } else {
        draft = null;
      }
    } catch (e) {
      formError = String(e);
    }
    busy = false;
  }

  async function remove(id: string) {
    await act(id, async (api) => {
      await api.deleteItem('connection', id);
    });
    confirming = null;
  }

  async function toggle(id: string, c: Connection, enabled: boolean) {
    try {
      await items.save(id, { ...c, enabled });
      await refresh();
    } catch (e) {
      formError = String(e);
    }
  }

  /** Where a Discord webhook link is kept, once the connection exists (and not before). */
  function hookStore(): HookStore | null {
    const d = draft;
    if (!d?.id) return null;
    const id = d.id;
    const set = async (slot: number, link: string) => {
      await (await getApi()).setConnectionSecret(id, `hook${slot}`, link);
      note = '';
      await refresh();
    };
    return {
      stored: (slot) => viewOf(id)?.secrets.some((s) => s.name === `hook${slot}` && s.stored) ?? false,
      save: set,
      clear: (slot) => set(slot, ''),
    };
  }

  const hasClaude = $derived(items.items.some((i) => i.item.kind === 'claude'));
</script>

<div class="page">
  {#if view?.store_problem}
    <div class="problem" role="alert">
      <Chip kind="err" label={t('No password store')} />
      <span>
        {t('{store_problem}. Tokens and sign-ins cannot be saved until it works. On Linux, install and unlock GNOME Keyring or KDE Wallet.', { store_problem: view.store_problem })}
      </span>
    </div>
  {/if}
  {#if items.failure}<Chip kind="err" label={items.failure} />{/if}
  {#each items.problems as p (p)}<Chip kind="warn" label={t('Could not read a connection file: {problem}', { problem: p })} />{/each}
  {#each view?.problems ?? [] as p (p)}<Chip kind="warn" label={p} />{/each}

  {#if items.loaded && items.items.length === 0 && !draft}
    <div class="empty">
      <h2>{t('No connections yet')}</h2>
      <p class="quiet">
        {t('A connection tells CargoFrame where a sharing button sends your capture. Passwords and tokens are kept in your system\'s password store, never in a file.')}
      </p>
    </div>
  {/if}

  {#each items.items as { id, item } (id)}
    {@const v = viewOf(id)}
    <article class="card" class:off={!item.enabled}>
      <div class="main">
        <div class="head">
          <h2>{item.name}</h2>
          <span class="quiet">{v?.kind_label ?? item.kind}</span>
          {#if v}{@const chip = stateChip(v, item)}<Chip kind={chip.kind} label={chip.label} />{/if}
        </div>
        {#if v?.describe.summary}<p class="quiet">{v.describe.summary}</p>{/if}
        {#if messages[id]}
          <div role="status"><Chip kind={messages[id]!.kind} label={messages[id]!.text} /></div>
        {/if}
      </div>
      <div class="actions">
        {#if v && item.enabled && v.state !== 'not_in_build'}
          {#if oauth(item.kind)}
            {#if signingIn === id}
              <span class="quiet">{t('Waiting for your browser…')}</span>
              <button onclick={() => (void (async () => (await getApi()).cancelSignIn())())}>{t('Cancel')}</button>
            {:else if v.state === 'connected'}
              <button data-tour="conn.test" onclick={() => act(id, (api) => api.testConnection(id))}>{t('Test')}</button>
              <button onclick={() => act(id, (api) => api.disconnectConnection(id))}>{t('Sign out')}</button>
            {:else}
              <button class="primary" data-tour="conn.signin" onclick={() => signIn(id)}>{v.state === 'expired' ? t('Sign in again') : t('Sign in')}</button>
            {/if}
          {:else if item.kind !== 'claude'}
            {#if v.state === 'connected' || v.state === 'expired'}
              <button data-tour="conn.test" onclick={() => act(id, (api) => api.testConnection(id))}>{t('Test')}</button>
            {/if}
          {/if}
        {/if}
        <button onclick={() => edit(id, item)}>{t('Edit')}</button>
        {#if confirming === id}
          <button class="danger" onclick={() => remove(id)}>{t('Delete for good')}</button>
          <button onclick={() => (confirming = null)}>{t('Keep')}</button>
        {:else}
          <button onclick={() => (confirming = id)}>{t('Delete')}</button>
        {/if}
      </div>
    </article>
  {/each}

  {#if hasClaude && !draft}
    <section class="card"><ClaudeChats /></section>
  {/if}

  {#if !draft}
    <section class="adders" data-tour="conn.add" aria-label={t('Add a connection')}>
      <h2>{t('Add a connection')}</h2>
      <div class="kinds">
        {#each offered as k (k.kind)}
          <button class="kind" data-tour={`add-${k.kind}`} onclick={() => add(k.kind)}>
            <strong>{k.label}</strong>
            <span class="quiet">{k.blurb}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? t('Edit connection') : t('New {service} connection', { service: KINDS.find((k) => k.kind === draft!.value.kind)?.label ?? '' })}</h2>
      <ConnectionForm bind:value={draft.value} hooks={hookStore()} />
      {#if draft.id}
        {@const v = viewOf(draft.id)}
        {#each v?.secrets ?? [] as s (s.name)}
          {#if needsSecret(draft.value, s.name)}
            <SecretInput
              label={secretLabel(draft.value, s.name)}
              hint={t('Kept in your system\'s password store. You cannot read it back from here.')}
              stored={s.stored}
              get={getLinkFor(draft.value, s.name === 'client_secret' ? 'client_secret' : s.name === 'app_password' ? 'app_password' : 'token')}
              onsave={async (value) => {
                await (await getApi()).setConnectionSecret(draft!.id!, s.name, value);
                note = '';
                await refresh();
              }}
              onclear={async () => {
                await (await getApi()).setConnectionSecret(draft!.id!, s.name, '');
                note = '';
                await refresh();
              }}
            />
          {/if}
        {/each}
      {/if}
      {#if note}<div role="status"><Chip kind="ok" label={note} /></div>{/if}
      {#if formError}<p class="error" role="alert">{formError}</p>{/if}
      <div class="buttons">
        <button class="primary" type="submit" data-tour="conn.save" disabled={busy}>{t('Save')}</button>
        <button type="button" data-tour="conn.done" onclick={() => (draft = null)}>{draft.id ? t('Done') : t('Cancel')}</button>
      </div>
    </form>
  {/if}
</div>

<style>
  .page { display: grid; gap: 12px; }
  .empty, .card, .editor, .adders {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px;
  }
  .empty, .adders { display: grid; gap: 8px; }
  .card { display: flex; gap: 16px; justify-content: space-between; flex-wrap: wrap; }
  .card.off .main { opacity: 0.6; }
  .main { display: grid; gap: 6px; min-width: 0; }
  .head { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; align-self: start; }
  .kinds { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 8px; }
  .kind { display: grid; gap: 4px; text-align: left; padding: 10px 12px; }
  .kind .quiet { font-size: 12.5px; }
  .editor { display: grid; gap: 14px; }
  .editor h2 { margin: 0; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  .problem {
    display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 10px 12px;
    border: 1px solid var(--err); border-radius: var(--radius);
    background: color-mix(in srgb, var(--err) 10%, transparent);
  }
</style>
