<script lang="ts">
  // How fast CargoFrame has been on this computer, and which windows hold memory right now.
  // Only shown, never changed here: the setting that decides it is in General.
  import { onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { getApi } from '../../lib/ipc';
  import type { MemoryStatus, TimingView } from '../../lib/types';

  let timings = $state<TimingView[]>([]);
  let memory = $state<MemoryStatus | null>(null);
  let refreshing = $state(false);
  let refreshedAt = $state('');

  const windowText = {
    open: 'Open now',
    kept: 'Hidden, kept ready (holds memory)',
    closed: 'Closed (no memory held)',
  } as const;

  async function read() {
    const api = await getApi();
    [timings, memory] = await Promise.all([
      api.getTimings().catch(() => []),
      api.getMemoryStatus().catch(() => null),
    ]);
  }

  onMount(read);

  /** The Refresh button stays on "Refreshing" for a moment even when the answer is instant, so the
   * click is visibly answered, then says when the numbers were read. */
  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      await Promise.all([read(), new Promise((done) => setTimeout(done, 450))]);
      refreshedAt = new Date().toLocaleTimeString();
    } finally {
      refreshing = false;
    }
  }
</script>

<Section
  title="Speed and memory"
  hint="How long the last captures and shares took against the targets CargoFrame aims for, and which windows hold memory now. Useful when something feels slow."
>
  <div class="sub" aria-busy={refreshing}>
    <h3>Windows right now</h3>
    {#if memory}
      <ul class="timings">
        {#each memory.windows as w (w.label)}
          <li>
            <span>{w.name}</span>
            <Chip kind={w.state === 'kept' ? 'warn' : w.state === 'open' ? 'ok' : 'dbg'} label={windowText[w.state]} />
          </li>
        {/each}
      </ul>
    {:else}
      <p class="quiet">Could not read the window state.</p>
    {/if}

    <h3>Last measured</h3>
    {#if timings.length === 0}
      <p class="quiet">Nothing measured yet. Use a trigger or share something, then press Refresh.</p>
    {:else}
      <ul class="timings">
        {#each timings as t (t.id)}
          <li>
            <span>{t.label}{t.note ? ` (${t.note})` : ''}</span>
            <Chip
              kind={t.budget_ms !== null && t.ms > t.budget_ms ? 'warn' : 'dbg'}
              label={t.budget_ms !== null ? `${t.ms} ms of ${t.budget_ms}` : `${t.ms} ms`}
            />
          </li>
        {/each}
      </ul>
    {/if}
    <div class="refresh">
      <button type="button" onclick={refresh} disabled={refreshing}>
        {refreshing ? 'Refreshing…' : 'Refresh'}
      </button>
      {#if refreshing}<span class="spinner" aria-hidden="true"></span>{/if}
      <span class="quiet" role="status" aria-live="polite">
        {#if refreshing}Reading the current state…{:else if refreshedAt}Refreshed at {refreshedAt}{/if}
      </span>
    </div>
  </div>
</Section>

<style>
  .sub { display: grid; gap: 8px; }
  .sub h3 { font-size: 13.5px; }
  .timings { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .timings li { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .refresh { display: flex; align-items: center; gap: 10px; }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%;
    border: 2px solid var(--border-strong); border-top-color: var(--accent);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation-duration: 2.4s; } }
</style>
