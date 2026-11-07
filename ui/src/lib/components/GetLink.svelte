<script lang="ts">
  // A button beside a field that opens the page where its value is obtained, in the browser.
  import { getApi } from '../ipc';
  import type { GetLinkSpec } from '../connectorHelp';

  let { link }: { link: GetLinkSpec | null } = $props();
  let failed = $state('');

  async function open() {
    if (!link) return;
    failed = '';
    try {
      await (await getApi()).openLink(link.url);
    } catch (e) {
      failed = String(e);
    }
  }
</script>

{#if link}
  <button type="button" class="get" title={link.url} onclick={open}>{link.label} <span aria-hidden="true">↗</span></button>
  {#if failed}<span class="error" role="alert">{failed}</span>{/if}
{/if}

<style>
  .get { white-space: nowrap; }
  .error { color: var(--err); font-size: 12.5px; }
</style>
