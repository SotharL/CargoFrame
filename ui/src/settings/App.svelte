<script lang="ts">
  import { onMount } from 'svelte';
  import { PAGES } from './nav';
  import { go, parseTarget, restore, route } from './route.svelte';
  import General from './pages/General.svelte';
  import Triggers from './pages/Triggers.svelte';
  import Rules from './pages/Rules.svelte';
  import InfoTypes from './pages/InfoTypes.svelte';
  import Layouts from './pages/Layouts.svelte';
  import Connections from './pages/Connections.svelte';
  import VideoPage from './pages/Video.svelte';
  import Placeholder from './pages/Placeholder.svelte';
  import Chip from '../lib/components/Chip.svelte';
  import { getApi } from '../lib/ipc';

  let trayAvailable = $state(true);
  const page = $derived(route.page);

  onMount(() => {
    restore(location.search);
    let stop: (() => void) | undefined;
    getApi().then(async (api) => {
      trayAvailable = (await api.appInfo()).tray_available;
      // "Edit layout" in the CargoFrame window asks an open Settings window to move.
      stop = await api.onNavigate((target) => {
        const t = parseTarget(target);
        if (t) go(t.page, t.arg);
      });
    });
    return () => stop?.();
  });

  const current = $derived(PAGES.find((p) => p.id === page)!);

  function onNavKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const i = PAGES.findIndex((p) => p.id === page);
    const next = PAGES[(i + (e.key === 'ArrowDown' ? 1 : PAGES.length - 1)) % PAGES.length]!;
    go(next.id);
    queueMicrotask(() => document.getElementById(`nav-${next.id}`)?.focus());
  }
</script>

<div class="shell">
  <nav aria-label="Settings sections">
    <div class="brand">CargoFrame</div>
    {#each PAGES as p (p.id)}
      <button
        id="nav-{p.id}"
        class="item"
        class:on={page === p.id}
        aria-current={page === p.id ? 'page' : undefined}
        tabindex={page === p.id ? 0 : -1}
        onkeydown={onNavKey}
        onclick={() => go(p.id)}>{p.title}</button
      >
    {/each}
  </nav>

  <main class:wide={page === 'layouts'}>
    {#if !trayAvailable}
      <div class="banner" role="status">
        <Chip kind="warn" label="No tray icon" />
        <span>
          This desktop shows no tray icons. On GNOME, install the AppIndicator extension. Until
          then, run CargoFrame again to open this window.
        </span>
      </div>
    {/if}
    <header>
      <h1>{current.title}</h1>
      <p class="quiet">{current.blurb}</p>
    </header>
    {#if page === 'general'}
      <General />
    {:else if page === 'triggers'}
      <Triggers />
    {:else if page === 'connections'}
      <Connections />
    {:else if page === 'rules'}
      <Rules />
    {:else if page === 'infotypes'}
      <InfoTypes />
    {:else if page === 'layouts'}
      <Layouts />
    {:else if page === 'video'}
      <VideoPage />
    {:else}
      <Placeholder title={current.title} />
    {/if}
  </main>
</div>

<style>
  .shell { display: grid; grid-template-columns: 220px 1fr; height: 100%; }
  nav {
    border-right: 1px solid var(--border);
    background: var(--surface-2);
    padding: 16px 12px;
    display: flex; flex-direction: column; gap: 2px;
  }
  .brand { font-weight: 700; padding: 4px 10px 14px; }
  .item {
    text-align: left; border: 0; background: transparent;
    padding: 8px 10px; border-radius: var(--radius-sm);
  }
  .item:hover { background: color-mix(in srgb, var(--text) 7%, transparent); }
  .item.on { background: var(--surface); box-shadow: inset 3px 0 0 var(--accent); font-weight: 600; }
  main { overflow: auto; padding: 24px 32px 40px; display: grid; align-content: start; gap: 20px; }
  header { display: grid; gap: 4px; }
  main > :global(*) { max-width: 720px; }
  main.wide > :global(*) { max-width: none; }
  .banner {
    display: flex; gap: 10px; align-items: center; flex-wrap: wrap;
    padding: 10px 12px; border: 1px solid var(--warn); border-radius: var(--radius);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
  }
</style>
