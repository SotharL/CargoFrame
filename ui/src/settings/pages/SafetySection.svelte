<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import { defaultSafe } from '../../lib/defaults';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Safe } from '../../lib/types';

  /** Reports the outcome of a save so the page can show one status line. */
  let { onstatus }: { onstatus: (kind: 'ok' | 'err', text: string) => void } = $props();

  let loaded = $state(false);
  let safe = $state<Safe>(defaultSafe());
  let folderText = $state('');
  let folderCheck = $state<{ ok: boolean; text: string } | null>(null);

  onMount(async () => {
    safe = await (await getApi()).getSafe();
    folderText = safe.folder ?? '';
    loaded = true;
  });

  async function save() {
    try {
      await (await getApi()).setSafe({ ...safe, folder: folderText.trim() || null });
      onstatus('ok', t('Saved'));
    } catch (e) {
      onstatus('err', String(e));
    }
  }

  async function choose() {
    const p = await (await getApi()).pickFolder(t('Choose the folder for saved zips'));
    if (p) {
      folderText = p;
      folderCheck = null;
      await save();
    }
  }

  async function check() {
    folderCheck = null;
    try {
      await (await getApi()).testSafeFolder(folderText);
      folderCheck = { ok: true, text: t('CargoFrame can save here') };
    } catch (e) {
      folderCheck = { ok: false, text: String(e) };
    }
  }
</script>

{#if loaded}
  <Section
    title={t('When a share fails')}
    tour="files.safe"
    hint={t('Nothing you captured is lost: CargoFrame tries again, then keeps a zip in a folder you choose.')}
  >
    <details class="steps">
      <summary>{t('What happens, step by step')}</summary>
      <ol>
        <li>{t('CargoFrame tries again a few times, waiting a little longer each time.')}</li>
        <li>{t('If it still fails, it saves everything as a zip in the folder below.')}</li>
        <li>{t('If that folder is not set or cannot be written, it asks you where to save.')}</li>
        <li>{t('If you cancel, the zip is kept in CargoFrame\'s own folder and you are told where.')}</li>
      </ol>
    </details>

    <div class="sub">
      <h3>{t('Folder for saved zips')}</h3>
      <Field
        label={t('Folder')}
        hint={t('Also where “Save locally” puts its zip when a button has no active connection. Leave it empty to be asked after a failed share; “Save locally” needs it set.')}
      >
        <div class="row">
          <input type="text" bind:value={folderText} onchange={save} placeholder={t('Ask me each time')} />
          <button type="button" onclick={choose}>{t('Choose…')}</button>
          <button type="button" onclick={check} disabled={!folderText.trim()}>{t('Test')}</button>
        </div>
      </Field>
      {#if folderCheck}<Chip kind={folderCheck.ok ? 'ok' : 'err'} label={folderCheck.text} />{/if}
    </div>

    <div class="sub">
      <h3>{t('Retries')}</h3>
      <div class="two">
        <Field label={t('Retries')} hint={t('Extra attempts after the first one (0 to 10).')}>
          <input type="number" min="0" max="10" bind:value={safe.max_retries} onchange={save} />
        </Field>
        <Field label={t('Give up after (seconds)')} hint={t('How long one attempt may take.')}>
          <input type="number" min="5" max="300" bind:value={safe.timeout_secs} onchange={save} />
        </Field>
      </div>
    </div>
  </Section>
{/if}

<style>
  .steps { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px; }
  .steps summary { cursor: pointer; font-weight: 600; font-size: 13px; }
  .steps ol { margin: 8px 0 4px; padding-left: 20px; display: grid; gap: 4px; }
  .sub { display: grid; gap: 8px; }
  .sub h3 { font-size: 13.5px; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; min-width: 0; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  input[type='number'] {
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; width: 100%;
  }
</style>
