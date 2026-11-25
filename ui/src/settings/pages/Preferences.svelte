<script lang="ts">
  import { LANGUAGES, resolve, t } from '../../lib/i18n/index.svelte';
  // How CargoFrame is to you: its language, its look, what it tells you, and whether it starts
  // with the computer. What it keeps and saves is on Files & backup; the program on this computer
  // (memory, updates) on System.
  import { onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Segmented from '../../lib/components/Segmented.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import AccentControl from './AccentControl.svelte';
  import { getApi } from '../../lib/ipc';
  import { go } from '../route.svelte';
  import { applyAccent, applyTheme } from '../../lib/theme';
  import type { General, LanguageInfo, ThemeChoice } from '../../lib/types';
  import { defaultGeneral } from '../../lib/defaults';

  let loaded = $state(false);
  let general = $state<General>(defaultGeneral());
  let autostart = $state(false);
  // an older copy Windows starts at sign-in instead of this one (a build run as administrator)
  let others = $state<string[]>([]);
  let stopping = $state(false);
  let language = $state<LanguageInfo | null>(null);
  let status = $state<'' | 'saved' | 'error'>('');
  let errorText = $state('');

  const themes: { value: ThemeChoice; label: string }[] = [
    { value: 'system', label: t('System') },
    { value: 'light', label: t('Light') },
    { value: 'dark', label: t('Dark') },
  ];

  onMount(async () => {
    const api = await getApi();
    [general, autostart, language] = await Promise.all([
      api.getGeneral(),
      api.getAutostart(),
      api.getLanguage().catch(() => null),
    ]);
    others = await api.autostartOthers().catch(() => []);
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

  async function stopOthers() {
    stopping = true;
    await save(async () => {
      others = await (await getApi()).removeAutostartOthers();
      autostart = await (await getApi()).getAutostart();
    });
    stopping = false;
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
      if (import.meta.env.MODE !== 'test') location.replace(`${location.pathname}?page=preferences`);
    });
  };
  const onAutostart = (v: boolean) => save(async () => (await getApi()).setAutostart(v));
</script>

{#if loaded}
  <div class="page">
    <Section
      title={t('Language')}
      tour="prefs.language"
      hint={t('Settings, the CargoFrame windows, the tray menu and the notifications. Settings opens again in the new language; a CargoFrame window already open keeps its own until it closes.')}
    >
      {#snippet aside()}
        <select value={general.language ?? 'system'} onchange={onLanguage} aria-label={t('Language')}>
          <option value="system">{t('Same as this computer ({name})', { name: systemName })}</option>
          {#each LANGUAGES as l (l.code)}
            <option value={l.code} lang={l.code}>{l.name}</option>
          {/each}
        </select>
      {/snippet}
    </Section>

    <Section title={t('Appearance')} tour="prefs.appearance">
      <Segmented label={t('Theme')} options={themes} bind:value={general.theme} onchange={onTheme} />
      <AccentControl value={general.accent} onpreview={applyAccent} onsave={onAccentSave} />
    </Section>

    <Section
      title={t('Notifications')}
      tour="prefs.notifications"
      hint={t('A notification can show in a recording of the screen: switch off the ones you do not want there.')}
    >
      <Toggle label={t('When something happens')} bind:checked={general.notifications.routine} onchange={onNotices} />
      <p class="quiet hint">{t('CargoFrame started, a recording started, something was shared or saved.')}</p>
      <Toggle label={t('When something goes wrong')} bind:checked={general.notifications.problems} onchange={onNotices} />
      <p class="quiet hint">{t('A capture or a recording failed, or a share did not go through.')}</p>
      <p class="quiet hint">
        {t('What shows on screen while recording (the red outline, the dot) is set on Recording.')}
        <button type="button" class="link" onclick={() => go('video')}>{t('Open Recording')}</button>
      </p>
    </Section>

    <Section title={t('Startup')} tour="prefs.startup" hint={t('CargoFrame waits in the tray for your shortcuts.')}>
      <Toggle label={t('Start CargoFrame when I sign in')} bind:checked={autostart} onchange={onAutostart} />
      {#if others.length}
        <div class="other" role="alert">
          <Chip kind="warn" label={t('Another copy of CargoFrame starts when you sign in')} />
          <p class="quiet hint">
            {t('Windows starts this older copy for every account, so it opens instead of this version:')}
          </p>
          {#each others as path (path)}<code class="path" title={path}>{path}</code>{/each}
          <p class="quiet hint">{t('Windows asks for an administrator to stop it. This version keeps starting when you sign in.')}</p>
          <button type="button" disabled={stopping} onclick={stopOthers}>
            {stopping ? t('Waiting for Windows…') : t('Stop the older copy')}
          </button>
        </div>
      {/if}
    </Section>

    <div class="status" role="status" aria-live="polite">
      {#if status === 'saved'}<Chip kind="ok" label={t('Saved')} />{/if}
      {#if status === 'error'}<Chip kind="err" label={t('Could not save')} /> <span class="quiet">{errorText}</span>{/if}
    </div>
  </div>
{/if}

<style>
  .page { display: grid; gap: 14px; }
  .hint { margin: 0; font-size: 12.5px; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); font: inherit; cursor: pointer; }
  .other { display: grid; gap: 6px; justify-items: start; padding: 10px 12px; border-radius: 8px;
    background: var(--surface-2, transparent); border: 1px solid var(--border); }
  .path { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .status { min-height: 26px; display: flex; align-items: center; gap: 8px; }
</style>
