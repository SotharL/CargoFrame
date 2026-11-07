<script lang="ts">
  // The list of all guides, at the foot of the sidebar.
  import { startTour } from './tourState.svelte';
  import { TOURS } from './tours';

  let menu = $state<HTMLDetailsElement>();
  function pick(id: string) {
    if (menu) menu.open = false;
    startTour(id);
  }
</script>

<details class="menu" bind:this={menu}>
  <summary>Guides</summary>
  <ul>
    {#each TOURS as t (t.id)}
      <li><button type="button" onclick={() => pick(t.id)}>{t.title}</button></li>
    {/each}
  </ul>
</details>

<style>
  .menu { margin-top: auto; padding: 8px 4px 0; border-top: 1px solid var(--border); }
  summary { cursor: pointer; padding: 6px 10px; font-weight: 600; font-size: 13px; }
  ul { list-style: none; margin: 4px 0 0; padding: 0; display: grid; gap: 1px; }
  button {
    width: 100%; text-align: left; border: 0; background: transparent; padding: 5px 10px;
    border-radius: var(--radius-sm); font-size: 12.5px;
  }
  button:hover { background: color-mix(in srgb, var(--text) 7%, transparent); }
</style>
