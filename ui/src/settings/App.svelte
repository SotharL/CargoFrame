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
  import { hasAll, pagesNamed, placeOf, search, wordsOf, type Finding } from './search';

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

  // Search: while there are words in the box, the sidebar lists only what has them, and the
  // sections of the page shown that do not are greyed out.
  let query = $state('');
  let searchBox = $state<HTMLInputElement>();
  const words = $derived(wordsOf(query));
  const found = $derived(search(query));
  const results = $derived.by(() => {
    const named = pagesNamed(query);
    return PAGES.map((p) => ({ page: p, findings: found.filter((f) => f.page === p.id) })).filter(
      (r) => r.findings.length > 0 || named.includes(r.page.id),
    );
  });
  let mainEl = $state<HTMLElement>();

  /** What a section says: its text, and the names its controls carry (a switch's label…). */
  function said(sec: HTMLElement): string {
    const names = [...sec.querySelectorAll('[aria-label], [title], [placeholder]')].map(
      (e) => `${e.getAttribute('aria-label') ?? ''} ${e.getAttribute('title') ?? ''} ${e.getAttribute('placeholder') ?? ''}`,
    );
    return `${sec.textContent ?? ''} ${names.join(' ')}`;
  }

  /** Grey out the sections of the page shown that do not have the words (in what they say, or
   *  in the words the search knows them by). */
  function dim() {
    if (!mainEl) return;
    const known = found.filter((f) => f.page === page && f.section).map((f) => f.section);
    for (const sec of mainEl.querySelectorAll<HTMLElement>('section')) {
      const title = sec.querySelector('h2')?.textContent?.trim() ?? '';
      const keep = words.length === 0 || known.includes(title) || hasAll(said(sec), words);
      sec.classList.toggle('search-dim', !keep);
    }
  }
  $effect(() => {
    void found;
    void page;
    if (!mainEl) return;
    dim();
    if (words.length === 0) return;
    // the page fills in as it loads: look again when it changes, at most once a frame
    let queued = false;
    const watch = new MutationObserver(() => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => {
        queued = false;
        dim();
      });
    });
    watch.observe(mainEl, { childList: true, subtree: true });
    return () => watch.disconnect();
  });

  /** Go to a finding and show it: the page, scrolled to it, which lights up a moment. */
  async function show(f: Finding) {
    go(f.page, f.arg ?? null);
    for (let i = 0; i < 20; i++) {
      await tick();
      const el = placeOf(f);
      if (el) {
        el.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
        el.classList.remove('search-found');
        void el.offsetWidth;
        el.classList.add('search-found');
        setTimeout(() => el.classList.remove('search-found'), 1800);
        return;
      }
      await new Promise((r) => setTimeout(r, 50));
    }
  }

  function clearSearch() {
    query = '';
    searchBox?.focus();
  }

  function onWindowKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      searchBox?.focus();
      searchBox?.select();
    }
  }

  function onNavKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const i = PAGES.findIndex((p) => p.id === page);
    const next = PAGES[(i + (e.key === 'ArrowDown' ? 1 : PAGES.length - 1)) % PAGES.length]!;
    go(next.id);
    queueMicrotask(() => document.getElementById(`nav-${next.id}`)?.focus());
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<div class="shell">
  <nav aria-label="Settings sections">
    <div class="brand"><Logo variant="wordmark" size={26} /></div>
    <div class="search" role="search">
      <input
        bind:this={searchBox}
        bind:value={query}
        type="text"
        placeholder="Search settings"
        aria-label="Search settings"
        title="Search settings (Ctrl+F)"
        autocomplete="off"
        spellcheck="false"
        onkeydown={(e) => {
          if (e.key === 'Escape' && query) {
            e.preventDefault();
            clearSearch();
          } else if (e.key === 'Enter' && found[0]) {
            e.preventDefault();
            void show(found[0]);
          }
        }}
      />
      {#if query}
        <button type="button" class="clear" aria-label="Clear the search" title="Clear the search" onclick={clearSearch}>×</button>
      {/if}
    </div>
    {#if words.length > 0}
      <div class="results" aria-live="polite">
        {#each results as r (r.page.id)}
          <button type="button" class="item" class:on={page === r.page.id} aria-current={page === r.page.id ? 'page' : undefined} onclick={() => go(r.page.id)}>{r.page.title}</button>
          {#each r.findings as f (f.title)}
            <button type="button" class="found" onclick={() => show(f)}>{f.title}</button>
          {/each}
        {:else}
          <p class="quiet none">No setting has these words.</p>
        {/each}
      </div>
    {:else}
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
    {/if}
    <GuideMenu />
  </nav>

  <main class:wide={page === 'layouts'} bind:this={mainEl}>
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
  .brand { padding: 4px 10px 12px; }
  .search { position: relative; margin: 0 0 10px; }
  .search input { width: 100%; padding: 6px 28px 6px 10px; font-size: 13px; }
  .search .clear {
    position: absolute; right: 4px; top: 50%; transform: translateY(-50%); width: 22px; height: 22px; padding: 0;
    border: 0; border-radius: 999px; background: transparent; color: var(--quiet); font-size: 16px; line-height: 1; cursor: pointer;
  }
  .search .clear:hover { background: color-mix(in srgb, var(--text) 10%, transparent); color: var(--text); }
  .results { display: flex; flex-direction: column; gap: 2px; }
  .found {
    text-align: left; border: 0; background: transparent; padding: 5px 10px 5px 22px; border-radius: var(--radius-sm);
    font-size: 13px; color: var(--quiet);
  }
  .found:hover { background: color-mix(in srgb, var(--text) 7%, transparent); color: var(--text); }
  .none { margin: 6px 10px; font-size: 13px; }
  /* the parts of the page shown without the words looked for */
  main :global(section.search-dim) { opacity: 0.35; filter: grayscale(1); transition: opacity 120ms; }
  main :global(.search-found) { animation: found 1.8s ease-out; }
  @keyframes found {
    0%, 30% { box-shadow: 0 0 0 3px var(--accent); }
    100% { box-shadow: 0 0 0 3px transparent; }
  }
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
