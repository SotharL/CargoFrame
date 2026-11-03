<script lang="ts">
  import { onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Segmented from '../../lib/components/Segmented.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import SafetySection from './SafetySection.svelte';
  import AccentControl from './AccentControl.svelte';
  import TransferSection from './TransferSection.svelte';
  import UpdatesSection from './UpdatesSection.svelte';
  import { getApi } from '../../lib/ipc';
  import { applyAccent, applyTheme } from '../../lib/theme';
  import type { AppInfo, General, MemoryMode, MemoryStatus, ThemeChoice, TimingView } from '../../lib/types';

  let loaded = $state(false);
  let general = $state<General>({ theme: 'system', memory_mode: 'balanced', check_updates: false, accent: '#4f8ee0' });
  let autostart = $state(false);
  let info = $state<AppInfo | null>(null);
  let timings = $state<TimingView[]>([]);
  let memory = $state<MemoryStatus | null>(null);
  let refreshing = $state(false);
  let refreshedAt = $state('');
  let status = $state<'' | 'saved' | 'error'>('');
  let errorText = $state('');

  const themes: { value: ThemeChoice; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];

  const memoryHints: Record<MemoryMode, string> = {
    balanced: 'Keeps the window ready for 5 minutes after you close it, then frees the memory.',
    low_memory: 'Frees the memory as soon as a window closes. Windows open a little slower.',
    always_warm: 'Keeps the window ready at all times. Opens fastest, uses the most memory.',
  };

  onMount(async () => {
    const api = await getApi();
    [general, autostart, info] = await Promise.all([
      api.getGeneral(),
      api.getAutostart(),
      api.appInfo(),
    ]);
    await readPerformance();
    loaded = true;
  });

  async function readPerformance() {
    const api = await getApi();
    [timings, memory] = await Promise.all([
      api.getTimings().catch(() => []),
      api.getMemoryStatus().catch(() => null),
    ]);
  }

  /** The Refresh button. It stays on "Refreshing" for a moment even when the answer is instant, so
   * the click is visibly answered, then says when the numbers were read. */
  async function refreshPerformance() {
    if (refreshing) return;
    refreshing = true;
    try {
      await Promise.all([readPerformance(), new Promise((done) => setTimeout(done, 450))]);
      refreshedAt = new Date().toLocaleTimeString();
    } finally {
      refreshing = false;
    }
  }

  const windowText = {
    open: 'Open now',
    kept: 'Hidden, kept ready (holds memory)',
    closed: 'Closed (no memory held)',
  } as const;

  async function save(action: () => Promise<void>) {
    try {
      await action();
      status = 'saved';
    } catch (e) {
      status = 'error';
      errorText = String(e);
    }
  }

  const onTheme = (t: ThemeChoice) => {
    applyTheme(t);
    return save(async () => (await getApi()).setGeneral({ ...general, theme: t }));
  };
  const onAccentSave = (hex: string) => {
    general.accent = hex;
    applyAccent(hex);
    return save(async () => (await getApi()).setGeneral({ ...general }));
  };
  const onMemory = () => save(async () => (await getApi()).setGeneral({ ...general }));
  const onAutoCheck = () => save(async () => (await getApi()).setGeneral({ ...general }));
  const onAutostart = (v: boolean) => save(async () => (await getApi()).setAutostart(v));
</script>

{#if loaded}
  <div class="page">
    <Section title="Appearance">
      <Segmented
        label="Theme"
        options={themes}
        bind:value={general.theme}
        onchange={onTheme}
      />
      <AccentControl value={general.accent} onpreview={applyAccent} onsave={onAccentSave} />
    </Section>

    <Section title="Startup" hint="CargoFrame stays in the tray and waits for your shortcut.">
      <Toggle label="Run CargoFrame when I sign in" bind:checked={autostart} onchange={onAutostart} />
    </Section>

    <SafetySection onstatus={(kind, text) => { status = kind === 'ok' ? 'saved' : 'error'; errorText = kind === 'ok' ? '' : text; }} />

    <TransferSection />

    <Section title="Files">
      <p class="quiet">Settings: <span class="mono selectable">{info?.config_dir}</span></p>
      <p class="quiet">Logs and kept zips: <span class="mono selectable">{info?.data_dir}</span></p>
      <div>
        <button type="button" onclick={async () => (await getApi()).openConfigFolder()}>
          Open settings folder
        </button>
      </div>
    </Section>

    <Section
      title="Performance"
      hint="How much memory CargoFrame keeps ready, what that is doing right now, and how long the last runs took against the targets it aims for."
    >
      <div class="sub">
        <h3>Memory</h3>
        <label class="row">
          <span>Keep windows ready</span>
          <select bind:value={general.memory_mode} onchange={async () => { await onMemory(); await refreshPerformance(); }}>
            <option value="balanced">Balanced</option>
            <option value="low_memory">Low memory</option>
            <option value="always_warm">Always warm</option>
          </select>
        </label>
        <p class="quiet">{memoryHints[general.memory_mode]}</p>
      </div>

      <div class="sub" aria-busy={refreshing}>
        <h3>Right now</h3>
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
          <button type="button" onclick={refreshPerformance} disabled={refreshing}>
            {refreshing ? 'Refreshing…' : 'Refresh'}
          </button>
          {#if refreshing}<span class="spinner" aria-hidden="true"></span>{/if}
          <span class="quiet" role="status" aria-live="polite">
            {#if refreshing}Reading the current state…{:else if refreshedAt}Refreshed at {refreshedAt}{/if}
          </span>
        </div>
      </div>
    </Section>

    <UpdatesSection bind:auto={general.check_updates} onauto={onAutoCheck} />

    <div class="status" role="status" aria-live="polite">
      {#if status === 'saved'}<Chip kind="ok" label="Saved" />{/if}
      {#if status === 'error'}<Chip kind="err" label="Could not save" /> <span class="quiet">{errorText}</span>{/if}
    </div>
  </div>
{/if}

<style>
  .page { display: grid; gap: 14px; }
  .sub { display: grid; gap: 8px; }
  .sub h3 { font-size: 13.5px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .status { min-height: 26px; display: flex; align-items: center; gap: 8px; }
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
