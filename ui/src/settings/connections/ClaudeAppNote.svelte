<script lang="ts">
  // A Claude connection that opens the desktop app needs it installed. On Windows this says so and
  // offers to install it (the same button as Setup, through winget); the download page is always
  // one click away.
  import { onDestroy, onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { CLAUDE_APP_DOWNLOAD } from '../../lib/connectorHelp';
  import { getApi } from '../../lib/ipc';
  import { setupState } from '../setupState.svelte';
  import type { SetupReport } from '../../lib/types';

  let { pollMs = 2500 }: { pollMs?: number } = $props();

  let report = $state<SetupReport | null>(null);
  let error = $state('');
  let timer: ReturnType<typeof setInterval> | undefined;

  const app = $derived(report?.items.find((i) => i.id === 'claude_app'));
  const running = $derived(report?.install.type === 'running');

  function take(r: SetupReport) {
    report = r;
    setupState.missing = r.missing;
    if (r.install.type === 'running' && !timer) timer = setInterval(() => void refresh(), pollMs);
    if (r.install.type !== 'running' && timer) {
      clearInterval(timer);
      timer = undefined;
    }
  }
  async function refresh() {
    try {
      take(await (await getApi()).getSetup());
      error = '';
    } catch (e) {
      error = String(e);
    }
  }
  async function install() {
    error = '';
    try {
      take(await (await getApi()).installMissing());
    } catch (e) {
      error = String(e);
    }
  }
  async function download() {
    try {
      await (await getApi()).openLink(CLAUDE_APP_DOWNLOAD);
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => void refresh());
  onDestroy(() => timer && clearInterval(timer));
</script>

{#if report?.os === 'windows'}
  <div class="note">
    {#if app?.status === 'ok'}
      <Chip kind="ok" label="Claude desktop app found" />
    {:else if running}
      <Chip kind="warn" label="Installing…" />
    {:else}
      <span class="quiet">The chat opens in the Claude desktop app{app ? ', which is not installed yet' : ''}.</span>
      {#if app?.status === 'missing' && report.install.type === 'run'}
        <button type="button" class="primary" onclick={install}>Install it</button>
      {/if}
      <button type="button" onclick={download}>Get it from claude.ai <span aria-hidden="true">↗</span></button>
    {/if}
    {#if error}<span class="error" role="alert">{error}</span>{/if}
  </div>
{/if}

<style>
  .note { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .error { color: var(--err); font-size: 12.5px; }
</style>
