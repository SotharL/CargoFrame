<script lang="ts">
  import { LANGUAGES, resolve, t } from '../../lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Segmented from '../../lib/components/Segmented.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import SafetySection from './SafetySection.svelte';
  import AccentControl from './AccentControl.svelte';
  import { getApi } from '../../lib/ipc';
  import { applyAccent, applyTheme } from '../../lib/theme';
  import type { General, LanguageInfo, MemoryMode, ThemeChoice } from '../../lib/types';

  let loaded = $state(false);
  let general = $state<General>({
    theme: 'system', memory_mode: 'balanced', check_updates: false, accent: '#4f8ee0',
    notifications: { routine: true, problems: true },
  });
  let autostart = $state(false);
  let language = $state<LanguageInfo | null>(null);
  let status = $state<'' | 'saved' | 'error'>('');
  let errorText = $state('');

  const themes: { value: ThemeChoice; label: string }[] = [
    { value: 'system', label: t('System') },
    { value: 'light', label: t('Light') },
    { value: 'dark', label: t('Dark') },
  ];

  const memoryHints: Record<MemoryMode, string> = {
    balanced: t('Keeps the window ready for 5 minutes after you close it, then frees the memory.'),
    low_memory: t('Frees the memory as soon as a window closes. Windows open a little slower.'),
    always_warm: t('Keeps the window ready at all times. Opens fastest, uses the most memory.'),
  };

  onMount(async () => {
    const api = await getApi();
    [general, autostart, language] = await Promise.all([
      api.getGeneral(),
      api.getAutostart(),
      api.getLanguage().catch(() => null),
    ]);
    loaded = true;
  });

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
  const onNotices = () => save(async () => (await getApi()).setGeneral({ ...general }));
  /** The language the computer is set to, as CargoFrame would speak it (English when it has no pack). */
  const systemName = $derived(
    LANGUAGES.find((l) => l.code === resolve('system', language?.system ?? undefined))?.name ?? 'English',
  );
  /** Save the language, then draw Settings again in it (each window reads its texts when it opens). */
  const onLanguage = (e: Event) => {
    const chosen = (e.currentTarget as HTMLSelectElement).value;
    return save(async () => {
      await (await getApi()).setGeneral({ ...general, language: chosen });
      if (import.meta.env.MODE !== 'test') location.replace(`${location.pathname}?page=general`);
    });
  };
  const onAutostart = (v: boolean) => save(async () => (await getApi()).setAutostart(v));
</script>

{#if loaded}
  <div class="page">
    <Section title={t('Appearance')}>
      <Segmented
        label={t('Theme')}
        options={themes}
        bind:value={general.theme}
        onchange={onTheme}
      />
      <AccentControl value={general.accent} onpreview={applyAccent} onsave={onAccentSave} />
    </Section>

    <Section
      title={t('Language')}
      hint={t('Settings, the CargoFrame windows, the tray menu and the notifications. Changing it opens Settings again in the new language; a CargoFrame window already open keeps its language until it closes.')}
    >
      <label class="row">
        <span>{t('Language')}</span>
        <select value={general.language ?? 'system'} onchange={onLanguage}>
          <option value="system">{t('Same as this computer ({name})', { name: systemName })}</option>
          {#each LANGUAGES as l (l.code)}
            <option value={l.code} lang={l.code}>{l.name}</option>
          {/each}
        </select>
      </label>
    </Section>

    <Section title={t('Startup')} hint={t('CargoFrame stays in the tray and waits for your shortcut.')}>
      <Toggle label={t('Run CargoFrame when I sign in')} bind:checked={autostart} onchange={onAutostart} />
    </Section>

    <Section
      title={t('Notifications')}
      hint={t('A notice can show up inside a recording of the screen. Switch off the ones you do not want there.')}
    >
      <Toggle label={t('Say when something happens')} bind:checked={general.notifications.routine} onchange={onNotices} />
      <p class="quiet hint">{t('CargoFrame started, a recording started, something was shared or saved.')}</p>
      <Toggle label={t('Say when something goes wrong')} bind:checked={general.notifications.problems} onchange={onNotices} />
      <p class="quiet hint">{t('A capture or a recording failed, or a share did not go through.')}</p>
    </Section>

    <SafetySection onstatus={(kind, text) => { status = kind === 'ok' ? 'saved' : 'error'; errorText = kind === 'ok' ? '' : text; }} />

    <Section
      title={t('Windows kept ready')}
      hint={t('A CargoFrame window that is kept ready opens at once, but holds memory while it waits. The About page shows what each window holds now.')}
    >
      <label class="row">
        <span>{t('Keep windows ready')}</span>
        <select bind:value={general.memory_mode} onchange={onMemory}>
          <option value="balanced">{t('Balanced')}</option>
          <option value="low_memory">{t('Low memory')}</option>
          <option value="always_warm">{t('Always warm')}</option>
        </select>
      </label>
      <p class="quiet">{memoryHints[general.memory_mode]}</p>
    </Section>

    <div class="status" role="status" aria-live="polite">
      {#if status === 'saved'}<Chip kind="ok" label={t('Saved')} />{/if}
      {#if status === 'error'}<Chip kind="err" label={t('Could not save')} /> <span class="quiet">{errorText}</span>{/if}
    </div>
  </div>
{/if}

<style>
  .page { display: grid; gap: 14px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .status { min-height: 26px; display: flex; align-items: center; gap: 8px; }
</style>
