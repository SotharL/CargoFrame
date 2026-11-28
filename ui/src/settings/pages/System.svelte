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
  import Managed from '../../lib/components/Managed.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import { setDebug } from '../../lib/debugView';
  import { getApi } from '../../lib/ipc';
  import type { DebugSetup, General, MemoryMode, PcCheck, Policy } from '../../lib/types';
  import { go } from '../route.svelte';

  let general = $state<General | null>(null);
  /** What the organisation decided (its policy.json); null where each person chooses. */
  let policy = $state<Policy | null>(null);
  let failed = $state('');

  // debug mode's setup: one button makes the bug report layout and its shortcut
  let setup = $state<DebugSetup | null>(null);
  let reportTo = $state('');
  let setupProblem = $state('');
  let settingUp = $state(false);

  onMount(async () => {
    const api = await getApi();
    general = await api.getGeneral().catch(() => null);
    policy = (await api.appInfo().catch(() => null))?.policy ?? null;
    if (general?.debug) await loadSetup();
  });

  async function loadSetup() {
    setup = await (await getApi()).debugSetupStatus().catch(() => null);
    reportTo = setup?.github[0]?.[0] ?? '';
  }

  // *Check this PC*: the real tests run on this computer, the report kept for the bug report
  let pc = $state<PcCheck | null>(null);
  let checking = $state(false);
  let copied = $state(false);
  let pcProblem = $state('');

  async function checkPc() {
    checking = true;
    copied = false;
    try {
      pc = await (await getApi()).checkThisPc();
      pcProblem = '';
    } catch (e) {
      pcProblem = String(e).replace(/^Error: /, '');
    }
    checking = false;
  }

  async function copyReport() {
    if (!pc) return;
    await navigator.clipboard?.writeText(pc.report).catch(() => {});
    copied = true;
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
    <UpdatesSection auto={general.check_updates} onauto={onAuto} locked={policy?.check_updates != null} />
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
      <Toggle label={t('Describe CargoFrame in a capture of its own windows')} checked={!!general.debug} onchange={onDebug} disabled={policy?.debug_mode === false} />
      {#if policy?.debug_mode === false}<Managed />{/if}
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
      {#if general.debug}
        <div class="report pc" data-tour="system.debug.check">
          <p class="title"><b>{t('Check this PC')}</b></p>
          <p class="small">
            {t('Check this PC tries what only this computer can show: a screenshot of every screen, a 3-second recording, the apps CargoFrame uses, starting when you sign in, the credential store and the shortcuts. The report goes with the next bug report.')}
          </p>
          <button type="button" disabled={checking} onclick={checkPc}>
            {checking ? t('Checking… (about 10 seconds)') : pc ? t('Check this PC again') : t('Check this PC')}
          </button>
          {#if pcProblem}<p class="problem small" role="alert">{pcProblem}</p>{/if}
          {#if pc}
            <ul class="steps" aria-label={t('What was checked')}>
              {#each pc.steps as s (s.id)}
                <li class={s.outcome}>
                  <span class="mark" aria-label={s.outcome === 'ok' ? t('Works') : s.outcome === 'warn' ? t('Works, with something to know') : t('Does not work')}>{s.outcome === 'ok' ? '✓' : s.outcome === 'warn' ? '!' : '✗'}</span>
                  <span class="what"><b>{s.name}</b> <span class="quiet">{s.detail}</span></span>
                  <span class="quiet ms">{s.ms} ms</span>
                </li>
              {/each}
            </ul>
            <p class="links small">
              <button type="button" class="link" onclick={copyReport}>{copied ? t('Copied') : t('Copy the report')}</button>
              {#if pc.saved}<span class="quiet" title={pc.saved}>{t('Kept for the next bug report')}</span>{/if}
            </p>
          {/if}
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
  .report .title { margin: 0; }
  .report .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .links { display: flex; gap: 14px; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); font: inherit; cursor: pointer; }
  .problem { color: var(--err); }
  .steps { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; width: 100%; font-size: 12.5px; }
  .steps li { display: grid; grid-template-columns: 18px minmax(0, 1fr) auto; gap: 8px; align-items: baseline; }
  .steps .what { min-width: 0; overflow-wrap: anywhere; }
  .steps .mark { font-weight: 700; text-align: center; }
  .steps .ok .mark { color: var(--ok); }
  .steps .warn .mark { color: var(--warn); }
  .steps .fail .mark { color: var(--err); }
  .steps .ms { font-variant-numeric: tabular-nums; }
</style>
