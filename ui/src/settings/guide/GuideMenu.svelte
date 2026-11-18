<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  // The list of all guides, at the foot of the sidebar.
  import { onMount } from 'svelte';
  import { startTour } from './tourState.svelte';
  import { TOURS } from './tours';
  import { getApi } from '../../lib/ipc';

  // a company build can leave services out: their guides go with them
  let built = $state<string[] | null>(null);
  onMount(async () => {
    built = (await (await getApi()).appInfo().catch(() => null))?.connectors ?? null;
  });
  const shown = $derived(
    TOURS.filter((t) => !built || !t.id.startsWith('connect-') || built.includes(t.id.slice('connect-'.length))),
  );
  // the guides about using CargoFrame first, then one per service, folded
  const usage = $derived(shown.filter((t) => !t.id.startsWith('connect-')));
  const services = $derived(shown.filter((t) => t.id.startsWith('connect-')));

  let menu = $state<HTMLDetailsElement>();
  function pick(id: string) {
    if (menu) menu.open = false;
    startTour(id);
  }
</script>

<details class="menu" bind:this={menu} data-tour="nav.guides">
  <summary>{t('Guides')}</summary>
  <ul>
    {#each usage as item (item.id)}
      <li><button type="button" onclick={() => pick(item.id)}>{item.title}</button></li>
    {/each}
  </ul>
  {#if services.length}
    <details class="services">
      <summary>{t('Connect a service')}</summary>
      <ul>
        {#each services as item (item.id)}
          <li><button type="button" onclick={() => pick(item.id)}>{item.title.replace(/^Connect /, '')}</button></li>
        {/each}
      </ul>
    </details>
  {/if}
</details>

<style>
  .menu { margin-top: auto; padding: 8px 4px 0; border-top: 1px solid var(--border); }
  summary { cursor: pointer; padding: 6px 10px; font-weight: 600; font-size: 13px; }
  .services { margin-top: 4px; }
  .services summary { font-weight: 500; font-size: 12.5px; color: var(--quiet); }
  .services ul { padding-left: 10px; }
  ul { list-style: none; margin: 4px 0 0; padding: 0; display: grid; gap: 1px; }
  button {
    width: 100%; text-align: left; border: 0; background: transparent; padding: 5px 10px;
    border-radius: var(--radius-sm); font-size: 12.5px;
  }
  button:hover { background: color-mix(in srgb, var(--text) 7%, transparent); }
</style>
