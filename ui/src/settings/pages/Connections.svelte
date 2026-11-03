<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { createItems } from '../../lib/items.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Connection, ConnectionsView, ConnectionView, ServiceKind } from '../../lib/types';
  import ConnectionForm from '../connections/ConnectionForm.svelte';
  import SecretInput from '../connections/SecretInput.svelte';
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
    { kind: 'jira', label: 'Jira', blurb: 'Create an issue or comment on one, with the screenshot attached.' },
    { kind: 'google', label: 'Google', blurb: 'Save the screenshot and files to Drive, and optionally log a row in a sheet.' },
    { kind: 'reddit', label: 'Reddit', blurb: 'Post the screenshot to a subreddit you choose.' },
    { kind: 'claude', label: 'Claude', blurb: 'Copy the screenshot and open a chat, so you can paste it into your message.' },
    { kind: 'custom', label: 'Custom API', blurb: 'Send to any web service that has an HTTP API.' },
  ];

  function blank(kind: ServiceKind): Connection {
    switch (kind) {
      case 'jira':
        return { kind, name: 'Jira', enabled: true, site_url: 'https://', auth: 'api_token', email: '' };
      case 'google':
        return { kind, name: 'Google Drive', enabled: true, client_id: '', sheets: false, full_drive: false };
      case 'reddit':
        return { kind, name: 'Reddit', enabled: true, client_id: '', redirect_port: 53682 };
      case 'claude':
        return { kind, name: 'Claude', enabled: true, open_with: 'auto', send_automatically: true, extra_wait_seconds: 0 };
      case 'custom':
        return {
          kind, name: 'My API', enabled: true, base_url: 'https://', auth: { type: 'none' },
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
  onMount(refresh);

  const viewOf = (id: string): ConnectionView | undefined => view?.items.find((c) => c.id === id);
  const oauth = (k: ServiceKind) => k === 'google' || k === 'reddit';

  function stateChip(c: ConnectionView): { kind: 'ok' | 'warn' | 'err' | 'dbg'; label: string } {
    switch (c.state) {
      case 'connected': return { kind: 'ok', label: c.kind === 'claude' ? 'Ready' : 'Connected' };
      case 'expired': return { kind: 'err', label: 'Sign-in expired' };
      case 'disabled': return { kind: 'dbg', label: 'Turned off' };
      default: return { kind: 'warn', label: oauth(c.kind) ? 'Not signed in' : 'Needs its token' };
    }
  }

  function secretLabel(c: Connection, name: string): string {
    if (name === 'client_secret') return 'Client secret';
    if (c.kind === 'jira') return c.auth === 'pat' ? 'Personal access token' : 'API token';
    if (c.kind === 'custom') {
      return { bearer: 'Token', header: 'Key', basic: 'Password', query: 'Key', none: 'Key' }[c.auth.type];
    }
    return 'Secret';
  }
  const needsSecret = (c: Connection, name: string) =>
    !(c.kind === 'custom' && name === 'secret' && c.auth.type === 'none');

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
      if (wasNew && wantsSecret) {
        // Stay open: the token can only be stored once the connection exists.
        d.id = id;
        note = 'Saved. Now add the token below.';
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

  const hasClaude = $derived(items.items.some((i) => i.item.kind === 'claude'));
</script>

<div class="page">
  {#if view?.store_problem}
    <div class="problem" role="alert">
      <Chip kind="err" label="No password store" />
      <span>
        {view.store_problem}. Tokens and sign-ins cannot be saved until it works. On Linux, install
        and unlock GNOME Keyring or KDE Wallet.
      </span>
    </div>
  {/if}
  {#if items.failure}<Chip kind="err" label={items.failure} />{/if}
  {#each items.problems as p (p)}<Chip kind="warn" label={`Could not read a connection file: ${p}`} />{/each}
  {#each view?.problems ?? [] as p (p)}<Chip kind="warn" label={p} />{/each}

  {#if items.loaded && items.items.length === 0 && !draft}
    <div class="empty">
      <h2>No connections yet</h2>
      <p class="quiet">
        A connection tells CargoFrame where a sharing button sends your capture. Passwords and tokens
        are kept in your system's password store, never in a file.
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
          {#if v}{@const chip = stateChip(v)}<Chip kind={chip.kind} label={chip.label} />{/if}
        </div>
        {#if v?.describe.summary}<p class="quiet">{v.describe.summary}</p>{/if}
        {#if messages[id]}
          <div role="status"><Chip kind={messages[id]!.kind} label={messages[id]!.text} /></div>
        {/if}
      </div>
      <div class="actions">
        {#if v && item.enabled}
          {#if oauth(item.kind)}
            {#if signingIn === id}
              <span class="quiet">Waiting for your browser…</span>
              <button onclick={() => (void (async () => (await getApi()).cancelSignIn())())}>Cancel</button>
            {:else if v.state === 'connected'}
              <button onclick={() => act(id, (api) => api.testConnection(id))}>Test</button>
              <button onclick={() => act(id, (api) => api.disconnectConnection(id))}>Sign out</button>
            {:else}
              <button class="primary" onclick={() => signIn(id)}>{v.state === 'expired' ? 'Sign in again' : 'Sign in'}</button>
            {/if}
          {:else if item.kind !== 'claude'}
            {#if v.state === 'connected' || v.state === 'expired'}
              <button onclick={() => act(id, (api) => api.testConnection(id))}>Test</button>
            {/if}
          {/if}
        {/if}
        <button onclick={() => edit(id, item)}>Edit</button>
        {#if confirming === id}
          <button class="danger" onclick={() => remove(id)}>Delete for good</button>
          <button onclick={() => (confirming = null)}>Keep</button>
        {:else}
          <button onclick={() => (confirming = id)}>Delete</button>
        {/if}
      </div>
    </article>
  {/each}

  {#if hasClaude && !draft}
    <section class="card"><ClaudeChats /></section>
  {/if}

  {#if !draft}
    <section class="adders" aria-label="Add a connection">
      <h2>Add a connection</h2>
      <div class="kinds">
        {#each KINDS as k (k.kind)}
          <button class="kind" onclick={() => add(k.kind)}>
            <strong>{k.label}</strong>
            <span class="quiet">{k.blurb}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else}
    <form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2>{draft.id ? 'Edit connection' : `New ${KINDS.find((k) => k.kind === draft!.value.kind)?.label} connection`}</h2>
      <ConnectionForm bind:value={draft.value} />
      {#if draft.id}
        {@const v = viewOf(draft.id)}
        {#each v?.secrets ?? [] as s (s.name)}
          {#if needsSecret(draft.value, s.name)}
            <SecretInput
              label={secretLabel(draft.value, s.name)}
              hint="Kept in your system's password store. You cannot read it back from here."
              stored={s.stored}
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
        <button class="primary" type="submit" disabled={busy}>Save</button>
        <button type="button" onclick={() => (draft = null)}>{draft.id ? 'Done' : 'Cancel'}</button>
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
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  .problem {
    display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 10px 12px;
    border: 1px solid var(--err); border-radius: var(--radius);
    background: color-mix(in srgb, var(--err) 10%, transparent);
  }
</style>
