<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { GROUP_TITLES, PAGES, type PageGroup } from './nav';
  import { go, parseTarget, restore, route } from './route.svelte';
  import General from './pages/General.svelte';
  import Triggers from './pages/Triggers.svelte';
  import DataFields from './pages/DataFields.svelte';
  import Layouts from './pages/Layouts.svelte';
  import Connections from './pages/Connections.svelte';
  import VideoPage from './pages/Video.svelte';
  import Setup from './pages/Setup.svelte';
  import DataPage from './pages/DataPage.svelte';
  import About from './pages/About.svelte';
  import Guide from './guide/Guide.svelte';
  import GuideMenu from './guide/GuideMenu.svelte';
  import { setupState } from './setupState.svelte';
  import Placeholder from './pages/Placeholder.svelte';
  import Chip from '../lib/components/Chip.svelte';
  import Logo from '../lib/components/Logo.svelte';
  import { getApi } from '../lib/ipc';

  let trayAvailable = $state(true);
  const page = $derived(route.page);

  onMount(() => {
    restore(location.search);
    let stop: (() => void) | undefined;
    getApi().then(async (api) => {
      trayAvailable = (await api.appInfo()).tray_available;
      // the number on the Setup tab, before that page is ever opened
      api.getSetup().then((r) => (setupState.missing = r.missing)).catch(() => {});
      // "Edit layout" in the CargoFrame window asks an open Settings window to move.
      stop = await api.onNavigate((target) => {
        const t = parseTarget(target);
        if (t) go(t.page, t.arg);
      });
    });
    return () => stop?.();
  });

  // "Connections" is the lower half of the Setup page: an address that names it scrolls there
  $effect(() => {
    if (route.page === 'setup' && route.arg === 'connections') {
      void tick().then(() => setTimeout(() => document.getElementById('connections')?.scrollIntoView({ block: 'start' }), 60));
    }
  });

  const current = $derived(PAGES.find((p) => p.id === page)!);
  const groups = (['start', 'capture', 'app'] as PageGroup[]).map((g) => ({ id: g, pages: PAGES.filter((p) => p.group === g) }));

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
    <div class="brand"><Logo variant="wordmark" size={26} /></div>
    {#each groups as g (g.id)}
      <p class="group" id="group-{g.id}">{GROUP_TITLES[g.id]}</p>
      <div role="group" aria-labelledby="group-{g.id}" class="pages">
        {#each g.pages as p (p.id)}
          <button
            id="nav-{p.id}"
            class="item"
            class:on={page === p.id}
            aria-current={page === p.id ? 'page' : undefined}
            tabindex={page === p.id ? 0 : -1}
            onkeydown={onNavKey}
            onclick={() => go(p.id)}
            >{p.title}{#if p.id === 'setup' && setupState.missing > 0}<span class="count" aria-label="{setupState.missing} missing">{setupState.missing}</span>{/if}</button
          >
        {/each}
      </div>
    {/each}
    <GuideMenu />
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
    {:else if page === 'rules'}
      <DataFields />
    {:else if page === 'layouts'}
      <Layouts />
    {:else if page === 'setup'}
      <Setup />
      <section id="connections" class="connections" aria-labelledby="connections-title">
        <h2 id="connections-title">Connections</h2>
        <p class="quiet">Services you share to: Jira, Google, Reddit, Claude, Slack, Discord, Teams, GitHub, X, Bluesky, Instagram, or your own API.</p>
        <Connections />
      </section>
    {:else if page === 'video'}
      <VideoPage />
    {:else if page === 'data'}
      <DataPage />
    {:else if page === 'about'}
      <About />
    {:else}
      <Placeholder title={current.title} />
    {/if}
  </main>
</div>
<Guide />

<style>
  /* the window never scrolls as a whole: the sidebar and the page each scroll on their own */
  .shell { display: grid; grid-template-columns: 220px 1fr; height: 100vh; overflow: hidden; }
  nav {
    border-right: 1px solid var(--border);
    background: var(--surface-2);
    padding: 16px 12px;
    display: flex; flex-direction: column; gap: 2px;
    min-height: 0; overflow-y: auto; scrollbar-gutter: stable;
  }
  .brand { padding: 4px 10px 16px; }
  .group {
    margin: 14px 10px 4px; font-size: 11.5px; font-weight: 600; letter-spacing: 0.04em;
    text-transform: uppercase; color: var(--quiet);
  }
  .group:first-of-type { margin-top: 0; }
  .pages { display: flex; flex-direction: column; gap: 2px; }
  .item {
    text-align: left; border: 0; background: transparent;
    padding: 8px 10px; border-radius: var(--radius-sm);
  }
  .count {
    margin-left: 8px; padding: 0 7px; border-radius: 999px; font-size: 12px; font-weight: 600;
    background: var(--warn); color: var(--on-warn, #1b1b1b);
  }
  .item:hover { background: color-mix(in srgb, var(--text) 7%, transparent); }
  .item.on { background: var(--surface); box-shadow: inset 3px 0 0 var(--accent); font-weight: 600; }
  main { overflow: auto; min-height: 0; padding: 24px 32px 40px; display: grid; align-content: start; gap: 20px; }
  header { display: grid; gap: 4px; }
  .connections { display: grid; gap: 14px; padding-top: 16px; border-top: 1px solid var(--border); }
  .connections h2 { margin: 0; font-size: 20px; }
  .connections > p { margin: -8px 0 0; }
  main > :global(*) { max-width: 720px; }
  main.wide > :global(*) { max-width: none; }
  .banner {
    display: flex; gap: 10px; align-items: center; flex-wrap: wrap;
    padding: 10px 12px; border: 1px solid var(--warn); border-radius: var(--radius);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
  }
</style>
