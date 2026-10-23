<script lang="ts">
  import { onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Segmented from '../../lib/components/Segmented.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import SafetySection from './SafetySection.svelte';
  import TransferSection from './TransferSection.svelte';
  import { getApi } from '../../lib/ipc';
  import { applyTheme } from '../../lib/theme';
  import type { AppInfo, General, MemoryMode, ThemeChoice, TimingView } from '../../lib/types';

  let loaded = $state(false);
  let general = $state<General>({ theme: 'system', memory_mode: 'balanced', check_updates: false });
  let autostart = $state(false);
  let info = $state<AppInfo | null>(null);
  let timings = $state<TimingView[]>([]);
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
    timings = await api.getTimings().catch(() => []);
    loaded = true;
  });

  async function refreshTimings() {
    timings = await (await getApi()).getTimings().catch(() => []);
  }

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
  const onMemory = () => save(async () => (await getApi()).setGeneral({ ...general }));
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
    </Section>

    <Section title="Startup" hint="CargoFrame stays in the tray and waits for your shortcut.">
      <Toggle label="Run CargoFrame when I sign in" bind:checked={autostart} onchange={onAutostart} />
    </Section>

    <SafetySection onstatus={(kind, text) => { status = kind === 'ok' ? 'saved' : 'error'; errorText = kind === 'ok' ? '' : text; }} />

    <Section
      title="Performance"
      hint="How much memory CargoFrame keeps ready, and how long the last runs took against the targets it aims for."
    >
      <div class="sub">
        <h3>Memory</h3>
        <label class="row">
          <span>Keep windows ready</span>
          <select bind:value={general.memory_mode} onchange={onMemory}>
            <option value="balanced">Balanced</option>
            <option value="low_memory">Low memory</option>
            <option value="always_warm">Always warm</option>
          </select>
        </label>
        <p class="quiet">{memoryHints[general.memory_mode]}</p>
      </div>

      <div class="sub">
        <h3>Last measured</h3>
        {#if timings.length === 0}
          <p class="quiet">Nothing measured yet. Use a trigger or share something, then look again.</p>
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
        <div><button type="button" onclick={refreshTimings}>Refresh</button></div>
      </div>
    </Section>

    <Section title="Updates">
      <p>
        Version <span class="mono selectable">{info?.version}</span>
      </p>
      <p class="quiet">
        Automatic checks are off while this project is private. To update, run the newer
        installer; it upgrades CargoFrame in place and keeps your settings.
      </p>
    </Section>

    <Section title="Files">
      <p class="quiet">Settings: <span class="mono selectable">{info?.config_dir}</span></p>
      <p class="quiet">Logs and kept zips: <span class="mono selectable">{info?.data_dir}</span></p>
      <div>
        <button type="button" onclick={async () => (await getApi()).openConfigFolder()}>
          Open settings folder
        </button>
      </div>
    </Section>

    <TransferSection />

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
</style>
