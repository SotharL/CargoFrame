<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Field from '../../lib/components/Field.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Safe } from '../../lib/types';

  let loaded = $state(false);
  let safe = $state<Safe>({ folder: null, timeout_secs: 30, max_retries: 3 });
  let folderText = $state('');
  let folderCheck = $state<{ ok: boolean; text: string } | null>(null);
  let status = $state<{ kind: 'ok' | 'err'; text: string } | null>(null);

  onMount(async () => {
    safe = await (await getApi()).getSafe();
    folderText = safe.folder ?? '';
    loaded = true;
  });

  async function save() {
    status = null;
    try {
      await (await getApi()).setSafe({ ...safe, folder: folderText.trim() || null });
      status = { kind: 'ok', text: 'Saved' };
    } catch (e) {
      status = { kind: 'err', text: String(e) };
    }
  }

  async function choose() {
    const p = await (await getApi()).pickFolder('Choose the folder for saved zips');
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
      folderCheck = { ok: true, text: 'CargoFrame can save here' };
    } catch (e) {
      folderCheck = { ok: false, text: String(e) };
    }
  }
</script>

{#if loaded}
  <div class="page">
    <Section title="When sharing fails">
      <ol class="steps">
        <li>CargoFrame tries again a few times, waiting a little longer each time.</li>
        <li>If it still fails, it saves everything as a zip in the folder below.</li>
        <li>If that folder is not set or cannot be written, it asks you where to save.</li>
        <li>If you cancel, the zip is kept in CargoFrame's own folder and you are told where.</li>
      </ol>
      <p class="quiet">Nothing you captured is thrown away because a service was down.</p>
    </Section>

    <Section title="Folder for saved zips">
      <Field label="Folder" hint="Leave it empty to be asked each time.">
        <div class="row">
          <input type="text" bind:value={folderText} onchange={save} placeholder="Ask me each time" />
          <button type="button" onclick={choose}>Choose…</button>
          <button type="button" onclick={check} disabled={!folderText.trim()}>Test</button>
        </div>
      </Field>
      {#if folderCheck}<Chip kind={folderCheck.ok ? 'ok' : 'err'} label={folderCheck.text} />{/if}
    </Section>

    <Section title="Retries">
      <div class="two">
        <Field label="Give up after (seconds)" hint="How long one attempt may take.">
          <input type="number" min="5" max="300" bind:value={safe.timeout_secs} onchange={save} />
        </Field>
        <Field label="Retries" hint="Extra attempts after the first one (0 to 10).">
          <input type="number" min="0" max="10" bind:value={safe.max_retries} onchange={save} />
        </Field>
      </div>
    </Section>

    {#if status}<Chip kind={status.kind} label={status.text} />{/if}
  </div>
{/if}

<style>
  .page { display: grid; gap: 16px; }
  .steps { margin: 0; padding-left: 20px; display: grid; gap: 4px; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; min-width: 0; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  input[type='number'] {
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; width: 100%;
  }
</style>
