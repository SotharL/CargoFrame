<script lang="ts">
  // Everything CargoFrame saves or keeps: where a share that failed is saved, a backup of layouts
  // and values for another computer, then what is on this computer, a way to clear it, and reset
  // (last: export comes first).
  import { t } from '../../lib/i18n/index.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import SafetySection from './SafetySection.svelte';
  import YourData from './YourData.svelte';
  import TransferSection from './TransferSection.svelte';

  let status = $state<{ ok: boolean; text: string } | null>(null);
</script>

<div class="page">
  <SafetySection onstatus={(kind, text) => (status = { ok: kind === 'ok', text })} />
  {#if status}
    <div role={status.ok ? 'status' : 'alert'}>
      {#if status.ok}<Chip kind="ok" label={t('Saved')} />{:else}<Chip kind="err" label={t('Could not save')} /> <span class="quiet">{status.text}</span>{/if}
    </div>
  {/if}
  <TransferSection />
  <YourData />
</div>

<style>
  .page { display: grid; gap: 14px; }
</style>
