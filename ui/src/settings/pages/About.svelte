<script lang="ts">
  // The program itself: its version and updates, then how it has been doing on this computer.
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import UpdatesSection from './UpdatesSection.svelte';
  import Diagnostics from './Diagnostics.svelte';
  import { getApi } from '../../lib/ipc';
  import type { General } from '../../lib/types';

  let general = $state<General | null>(null);
  let failed = $state('');

  onMount(async () => {
    general = await (await getApi()).getGeneral().catch(() => null);
  });

  /** The daily check is a General setting; it is shown next to the button it is about. */
  async function onAuto(value: boolean) {
    if (!general) return;
    general.check_updates = value;
    try {
      await (await getApi()).setGeneral({ ...general });
      failed = '';
    } catch (e) {
      failed = String(e);
    }
  }
</script>

<div class="page">
  {#if general}
    <UpdatesSection auto={general.check_updates} onauto={onAuto} />
  {/if}
  {#if failed}<div role="alert"><Chip kind="err" label="Could not save" /> <span class="quiet">{failed}</span></div>{/if}
  <Diagnostics />
</div>

<style>
  .page { display: grid; gap: 14px; }
</style>
