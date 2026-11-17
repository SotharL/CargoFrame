<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  // Test-only: a page with things for a guide to point at.
  import Guide from './Guide.svelte';

  let { hide = '', onpress = () => {} }: { hide?: string; onpress?: (name: string) => void } = $props();
</script>

<button data-tour="one" onclick={() => onpress('one')}>{t('One')}</button>
{#if hide !== 'two'}<button data-tour="two" onclick={() => onpress('two')}>{t('Two')}</button>{/if}
<button data-tour="three">{t('Three')}</button>
<Guide />
