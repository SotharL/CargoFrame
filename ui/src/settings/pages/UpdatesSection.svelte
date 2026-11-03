<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import SecretInput from '../connections/SecretInput.svelte';
  import { formatSize } from '../../capture/values';
  import { getApi } from '../../lib/ipc';
  import type { UpdateView } from '../../lib/types';

  let {
    auto = $bindable(false),
    onauto,
  }: {
    /** The "check once a day" setting; saved by the page that owns General. */
    auto?: boolean;
    onauto?: (value: boolean) => void;
  } = $props();

  let view = $state<UpdateView | null>(null);
  let error = $state('');
  let stop: (() => void) | undefined;
  let gone = false;

  async function refresh() {
    try {
      view = await (await getApi()).getUpdateView();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    await refresh();
    const unlisten = await (await getApi()).onUpdateChanged(() => void refresh());
    if (gone) unlisten();
    else stop = unlisten;
  });
  onDestroy(() => {
    gone = true;
    stop?.();
  });

  const working = $derived(
    view !== null &&
      ['checking', 'downloading', 'verifying', 'installing'].includes(view.phase.state),
  );

  async function check() {
    error = '';
    try {
      view = await (await getApi()).checkUpdate();
    } catch (e) {
      error = String(e);
    }
  }

  async function install() {
    error = '';
    try {
      await (await getApi()).installUpdate();
    } catch (e) {
      error = String(e);
    }
    await refresh();
  }

  async function saveToken(value: string) {
    await (await getApi()).setUpdateToken(value);
    await refresh();
  }

  async function open(url: string) {
    try {
      await (await getApi()).openLink(url);
    } catch (e) {
      error = String(e);
    }
  }

  /** A date for the release line, or nothing when GitHub did not say. */
  function day(iso: string | null): string {
    if (!iso) return '';
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? '' : d.toLocaleDateString();
  }

  const percent = (done: number, total: number | null) =>
    total && total > 0 ? Math.min(100, Math.round((done / total) * 100)) : null;

  // Where the token matters: the last check or the install failed for want of one.
  const wantsToken = $derived(
    view !== null &&
      ((view.last?.state === 'failed' && view.last.needs_token) ||
        (view.phase.state === 'failed' && view.phase.needs_token)),
  );
</script>

<Section
  title="Updates"
  hint="CargoFrame looks for a newer release on GitHub, checks the download against its published checksum, and installs it over this one. Your settings stay."
>
  {#if view}
    <p>
      Version <span class="mono selectable">{view.current}</span>
    </p>

    {#if !view.supported}
      <p class="quiet">
        There is no installer for this kind of computer yet, so CargoFrame cannot update itself here.
      </p>
    {/if}

    <div class="actions">
      <button type="button" class="primary" onclick={check} disabled={working || !view.supported}>
        {view.phase.state === 'checking' ? 'Checking…' : 'Check for updates'}
      </button>
      {#if view.phase.state === 'checking'}<span class="spinner" aria-hidden="true"></span>{/if}
    </div>

    <div class="result" role="status" aria-live="polite">
      {#if view.phase.state === 'downloading'}
        {@const p = percent(view.phase.done, view.phase.total)}
        <p>
          Downloading
          {#if view.phase.total}
            {formatSize(view.phase.done)} of {formatSize(view.phase.total)}
          {:else}
            {formatSize(view.phase.done)}
          {/if}
        </p>
        <progress
          aria-label="Download progress"
          max={view.phase.total ?? undefined}
          value={p === null ? undefined : view.phase.done}
        ></progress>
      {:else if view.phase.state === 'verifying'}
        <p>Checking the download against its checksum…</p>
      {:else if view.phase.state === 'installing'}
        <p>
          Installing. CargoFrame will close and open again by itself; this window closes with it.
        </p>
      {:else if view.phase.state === 'failed'}
        <p><Chip kind="err" label="The update did not finish" /></p>
        <p class="error" role="alert">{view.phase.message}</p>
        {#if view.phase.installer}
          <p class="quiet">
            The downloaded installer was checked and kept here, so you can run it yourself:
            <span class="mono selectable">{view.phase.installer}</span>
          </p>
        {/if}
      {:else if view.last?.state === 'up_to_date'}
        <p><Chip kind="ok" label="CargoFrame is up to date" /></p>
        <p class="quiet">The newest release is {view.last.latest}.</p>
      {:else if view.last?.state === 'failed'}
        <p><Chip kind="err" label="Could not check" /></p>
        <p class="error" role="alert">{view.last.message}</p>
      {/if}

      {#if view.last?.state === 'available' && !working && view.phase.state !== 'failed'}
        <p>
          <Chip kind="warn" label={`Version ${view.last.version} is available`} />
        </p>
        <p class="quiet">
          {[view.last.installer, formatSize(view.last.size), day(view.last.published) && `released ${day(view.last.published)}`].filter(Boolean).join(' · ')}
        </p>
        {#if view.last.notes.trim()}
          <pre class="notes selectable" aria-label="Release notes">{view.last.notes.trim()}</pre>
        {/if}
        <div class="actions">
          <button type="button" class="primary" onclick={install}>Download and install</button>
          <button type="button" onclick={() => open(view!.last?.state === 'available' ? view!.last.page : '')}>
            Release page
          </button>
        </div>
      {/if}
    </div>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <Toggle bind:checked={auto} label="Look for a newer version once a day" onchange={onauto} />
    <p class="quiet">
      You get a notification when one is found. Nothing is downloaded or installed until you press
      the button above.
    </p>

    <div class="token" class:ask={wantsToken}>
      <SecretInput
        label="GitHub access token"
        stored={view.token_set}
        onsave={saveToken}
        onclear={() => saveToken('')}
        hint={`While the ${view.repository} repository is private, GitHub only answers with a token. Create a fine-grained token that can only read this repository's contents. It is kept in your system's password store and sent to api.github.com only. Leave it empty once the repository is public.`}
      />
      {#if view.token_problem}
        <p class="error" role="alert">
          The password store could not be read, so a token cannot be used: {view.token_problem}
        </p>
      {/if}
    </div>
  {:else if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</Section>

<style>
  .actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .result { display: grid; gap: 6px; }
  .result p { margin: 0; }
  .error { color: var(--err); font-size: 13px; margin: 0; }
  .notes {
    margin: 0; padding: 8px 10px; max-height: 160px; overflow: auto; white-space: pre-wrap;
    background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-sm);
    font: inherit; font-size: 13px;
  }
  progress { width: 100%; max-width: 420px; accent-color: var(--accent); }
  .token { display: grid; gap: 6px; padding-top: 6px; border-top: 1px solid var(--border); }
  .token.ask { border: 1px solid var(--warn); border-radius: var(--radius-sm); padding: 10px; }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%;
    border: 2px solid var(--border-strong); border-top-color: var(--accent);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation-duration: 2.4s; } }
</style>
