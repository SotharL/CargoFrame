<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  // The program on this computer: the apps it uses (and a button for what is missing), its
  // version and updates, how long its windows are kept ready, and how fast it has been.
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Section from '../../lib/components/Section.svelte';
  import Setup from './Setup.svelte';
  import UpdatesSection from './UpdatesSection.svelte';
  import Diagnostics from './Diagnostics.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import { setDebug } from '../../lib/debugView';
  import { getApi } from '../../lib/ipc';
  import type { DebugSetup, General, MemoryMode } from '../../lib/types';
  import { go } from '../route.svelte';

  let general = $state<General | null>(null);
  let failed = $state('');

  // debug mode's setup: one button makes the bug report layout and its shortcut
  let setup = $state<DebugSetup | null>(null);
  let reportTo = $state('');
  let setupProblem = $state('');
  let settingUp = $state(false);

  onMount(async () => {
    general = await (await getApi()).getGeneral().catch(() => null);
    if (general?.debug) await loadSetup();
  });

  async function loadSetup() {
    setup = await (await getApi()).debugSetupStatus().catch(() => null);
    reportTo = setup?.github[0]?.[0] ?? '';
  }

  async function setUp() {
    settingUp = true;
    setupProblem = '';
    try {
      setup = await (await getApi()).setupDebugReport(reportTo || undefined);
    } catch (e) {
      setupProblem = String(e).replace(/^Error: /, '');
    }
    settingUp = false;
  }

  const memoryHints: Record<MemoryMode, string> = {
    balanced: t('Ready for 5 minutes after it closes, then the memory is freed.'),
    low_memory: t('The memory is freed as soon as a window closes. Windows open a little slower.'),
    always_warm: t('Always ready. Opens fastest, uses the most memory.'),
  };

  /** The daily check and the memory are General settings, shown next to what they are about. */
  async function saveGeneral() {
    if (!general) return;
    try {
      await (await getApi()).setGeneral({ ...general });
      failed = '';
    } catch (e) {
      failed = String(e);
    }
  }
  function onDebug(value: boolean) {
    if (!general) return;
    general.debug = value;
    setDebug(value);
    if (value) void loadSetup();
    void saveGeneral();
  }
  function onAuto(value: boolean) {
    if (!general) return;
    general.check_updates = value;
    void saveGeneral();
  }
</script>

<div class="page">
  <Setup />
  {#if general}
    <UpdatesSection auto={general.check_updates} onauto={onAuto} />
    <Section
      title={t('Windows kept ready')}
      tour="system.memory"
      hint={t('A window kept ready opens at once, but holds memory while it waits. Speed and memory, below, shows what each one holds.')}
    >
      {#snippet aside()}
        <select bind:value={general!.memory_mode} onchange={saveGeneral} aria-label={t('Keep windows ready')}>
          <option value="balanced">{t('Balanced')}</option>
          <option value="low_memory">{t('Low memory')}</option>
          <option value="always_warm">{t('Always warm')}</option>
        </select>
      {/snippet}
      <p class="quiet small">{memoryHints[general.memory_mode]}</p>
    </Section>
  {/if}
  {#if general}
    <Section
      title={t('Debug mode')}
      tour="system.debug"
      hint={t('For reporting a problem in CargoFrame itself. Off, nothing of this is kept.')}
    >
      <Toggle label={t('Describe CargoFrame in a capture of its own windows')} checked={!!general.debug} onchange={onDebug} />
      <p class="quiet small">
        {t('A capture of a CargoFrame window then fills values such as {cargoframe.page}, {cargoframe.section}, {cargoframe.focus} and {cargoframe.guide}, and adds the layout on screen, the data rules, the settings and the end of the log to its Files blocks. The template CargoFrame bug report uses them.')}
      </p>
      {#if general.debug && setup}
        <div class="report" data-tour="system.debug.setup">
          {#if setup.layout && setup.shortcut !== null}
            <Chip kind="ok" label={t('The bug report is set up')} />
            <p class="small">
              {setup.shortcut
                ? t('In any CargoFrame window, press {keys}: the window opens with the screenshot, where it was and the files, ready to send to GitHub.', { keys: setup.shortcut })
                : t('In any CargoFrame window, choose Report a CargoFrame problem in the tray menu.')}
            </p>
            <p class="links small">
              <button type="button" class="link" onclick={() => go('layouts', 'cargoframe-report')}>{t('Open the layout')}</button>
              <button type="button" class="link" onclick={() => go('triggers')}>{t('Change the shortcut')}</button>
            </p>
          {:else if setup.github.length === 0}
            <p class="small">{t('The report is a GitHub issue: add a GitHub connection first, then set it up here.')}</p>
            <button type="button" onclick={() => go('connections')}>{t('Go to Connections')}</button>
          {:else}
            <p class="small">{t('One button makes the CargoFrame bug report layout and a shortcut that opens it.')}</p>
            <div class="row">
              {#if setup.github.length > 1}
                <select bind:value={reportTo} aria-label={t('GitHub connection the reports go to')}>
                  {#each setup.github as [id, name] (id)}<option value={id}>{name}</option>{/each}
                </select>
              {/if}
              <button type="button" class="primary" disabled={settingUp} onclick={setUp}>{t('Set up the bug report')}</button>
            </div>
          {/if}
          {#if setupProblem}<p class="problem small" role="alert">{setupProblem}</p>{/if}
        </div>
      {/if}
    </Section>
  {/if}
  {#if failed}<div role="alert"><Chip kind="err" label={t('Could not save')} /> <span class="quiet">{failed}</span></div>{/if}
  <Diagnostics />
</div>

<style>
  .page { display: grid; gap: 14px; }
  .small { margin: 0; font-size: 12.5px; }
  .report { display: grid; gap: 8px; justify-items: start; margin-top: 10px; padding: 10px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm); }
  .report .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .links { display: flex; gap: 14px; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); font: inherit; cursor: pointer; }
  .problem { color: var(--err); }
</style>
