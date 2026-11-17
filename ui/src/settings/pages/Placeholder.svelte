<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  let { title }: { title: string } = $props();
</script>

<div class="empty">
  <Chip kind="dbg" label={t('Not built yet')} />
  <p>{t('The {page} page arrives in a later phase of the plan.', { page: title })}</p>
</div>

<style>
  .empty {
    display: grid; gap: 10px; justify-items: start; padding: 20px;
    border: 1px dashed var(--border-strong); border-radius: var(--radius);
  }
</style>
