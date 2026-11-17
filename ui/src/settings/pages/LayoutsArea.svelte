<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  // The Layouts page: the builder, and Fill fields (what fills a field of a service on each share
  // from the values of Data & fields). Fill fields was a tab of Data & fields before 0.5.0.
  import { untrack } from 'svelte';
  import PageTabs from '../../lib/components/PageTabs.svelte';
  import { route } from '../route.svelte';
  import Fields from './Fields.svelte';
  import Layouts from './Layouts.svelte';

  type Tab = 'layouts' | 'fields';
  let tab = $state<Tab>(route.arg === 'fields' ? 'fields' : 'layouts');
  $effect(() => {
    if (route.page !== 'layouts') return;
    // a layout to open shows the builder
    const arg = route.arg;
    untrack(() => {
      if (arg === 'fields') tab = 'fields';
      else if (arg) tab = 'layouts';
    });
  });
</script>

<PageTabs
  label={t('Layouts')}
  bind:value={tab}
  tabs={[
    { id: 'layouts', label: t('Layouts'), tour: 'layouts.tab' },
    { id: 'fields', label: t('Fill fields'), tour: 'layouts.fields' },
  ]}
/>
<div id="panel-{tab}" role="tabpanel" aria-labelledby="tab-{tab}">
  {#if tab === 'layouts'}<Layouts />{:else}<div class="fields"><Fields /></div>{/if}
</div>

<style>
  .fields { max-width: 900px; }
</style>
