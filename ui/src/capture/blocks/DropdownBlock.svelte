<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import Combobox from '../../lib/components/Combobox.svelte';
  import type { Block, DropdownState } from '../../lib/types';
  import type { Recent } from '../../lib/recent';
  import type { DeeperPick } from '../values';
  import DeeperStep from './DeeperStep.svelte';
  import {
    ALL, NO_CATEGORY, categoriesIn, comboOptions, hasLevels, narrow, needsCategoryStep, recentIn, scope, serversOf,
  } from './cascade';

  /** From this many choices on, the list can be searched instead of scrolled. */
  const SEARCH_FROM = 8;

  type DropdownBlock = Extract<Block, { type: 'dropdown' }>;
  let {
    block,
    info,
    value = $bindable(null),
    recent = [],
    narrowText = '',
    sessionId = 0,
    layoutId = null,
    deeper = $bindable({ value: '', label: '' }),
  }: {
    block: DropdownBlock;
    info: DropdownState;
    value: string | null;
    /** The choices used before in this block, newest first. */
    recent?: Recent[];
    /** The words of the text block that narrows this list (a thread named like the title). */
    narrowText?: string;
    /** For a block that goes deeper: the capture and layout the second list is asked for and kept under. */
    sessionId?: number;
    layoutId?: string | null;
    /** The second step (a thread inside the channel); only used when the block goes deeper. */
    deeper?: DeeperPick;
  } = $props();
  const id = $derived(`dd-${block.id}`);
  const optional = $derived(block.optional === true);
  // Chats have their own order (pinned, recent): the memory is for lists a service sends.
  const remembers = $derived(block.remember !== false && block.source.type === 'web_api');

  let showAll = $state(false);
  // A block that goes deeper narrows the second list by the title (a post named like it), not the first.
  const narrowed = $derived(
    showAll || block.deeper ? { options: info.options, applied: false, matched: true } : narrow(info.options, narrowText),
  );
  const options = $derived(narrowed.options);

  // Server and category: followed from the choice made, until the person changes them.
  let pickedServer = $state<string | null>(null);
  let pickedCategory = $state<string | null>(null);
  const levels = $derived(hasLevels(options));
  const servers = $derived(serversOf(options));
  const current = $derived(options.find((o) => o.value === value));
  const server = $derived(
    !levels ? null : servers.includes(pickedServer ?? '') ? pickedServer : (current?.server ?? servers[0] ?? null),
  );
  const category = $derived(pickedCategory ?? ALL);
  const showServer = $derived(servers.length > 1);
  const categories = $derived(categoriesIn(options, server));
  const showCategory = $derived(levels && needsCategoryStep(options, server));
  const inScope = $derived(scope(options, server, showCategory ? category : ALL));

  const recentHere = $derived(remembers ? recentIn(options, recent) : []);
  const choices = $derived(comboOptions(inScope, recentHere, levels));
  const elsewhere = $derived(
    levels
      ? comboOptions(options.filter((o) => !inScope.includes(o)), [], true)
      : [],
  );

  const searchable = $derived(info.status === 'ready' && options.length > SEARCH_FROM);
  const asCombo = $derived(
    info.status === 'ready' && (levels || searchable || optional || recentHere.length > 0),
  );
  const empty = $derived(info.status === 'ready' && info.options.length === 0);
  const groups = $derived.by(() => {
    const out: { name: string | null; options: DropdownState['options'] }[] = [];
    for (const o of options) {
      const last = out[out.length - 1];
      if (last && last.name === o.group) last.options.push(o);
      else out.push({ name: o.group, options: [o] });
    }
    return out;
  });

  // What is shown above the choice (server, category) stays where the person put it. It only
  // moves when the choice is made from somewhere else: a search or a recent choice that lies
  // outside the server or the category being shown.
  function choose(next: string) {
    value = next === '' && !optional ? value : next;
    const to = options.find((o) => o.value === next);
    if (!levels || !to?.server) return;
    if (to.server !== server) {
      pickedServer = to.server;
      pickedCategory = ALL;
    } else if (category !== ALL && !scope(options, server, category).some((o) => o.value === next)) {
      pickedCategory = to.category ?? NO_CATEGORY;
    }
  }
  function pickServer(s: string) {
    pickedServer = s;
    pickedCategory = ALL;
    value = optional ? value : (scope(options, s, ALL)[0]?.value ?? null);
  }
  function pickCategory(c: string) {
    pickedCategory = c;
    const within = scope(options, server, c);
    if (!within.some((o) => o.value === value)) value = optional ? value : (within[0]?.value ?? null);
  }
  const categoryLabel = (c: string) => (c === NO_CATEGORY ? 'No category' : c);
</script>

<div class="dd">
  <label for={id}>{block.title}</label>
  {#if asCombo}
    <div class="row">
      {#if showServer}
        <select class="step" aria-label="Server" value={server ?? ''} onchange={(e) => pickServer(e.currentTarget.value)}>
          {#each servers as s (s)}<option value={s}>{s}</option>{/each}
        </select>
      {/if}
      {#if showCategory}
        <select class="step" aria-label="Category" value={category} onchange={(e) => pickCategory(e.currentTarget.value)}>
          <option value={ALL}>All categories</option>
          {#each categories as c (c)}<option value={c}>{c}</option>{/each}
          {#if scope(options, server, ALL).some((o) => !o.category)}<option value={NO_CATEGORY}>{categoryLabel(NO_CATEGORY)}</option>{/if}
        </select>
      {/if}
      <Combobox
        {id}
        options={choices}
        also={elsewhere}
        value={value ?? ''}
        free={optional}
        onchange={choose}
        placeholder={optional
          ? empty ? 'Paste a link (optional)' : 'None: type to search, or paste a link'
          : `Search ${options.length} choices…`}
        emptyText={empty ? 'Nothing to choose from' : 'Nothing matches'}
      />
    </div>
  {:else}
    <select {id} bind:value disabled={info.status !== 'ready'}>
      {#if info.status === 'loading'}
        <option value={null}>Loading…</option>
      {:else if info.status === 'needs_connection'}
        <option value={null}>Not connected</option>
      {:else}
        {#each groups as g, i (i)}
          {#if g.name}
            <optgroup label={g.name}>
              {#each g.options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
            </optgroup>
          {:else}
            {#each g.options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
          {/if}
        {/each}
      {/if}
    </select>
  {/if}
  {#if block.deeper && info.status === 'ready' && value}
    <DeeperStep
      {sessionId}
      blockId={block.id}
      {layoutId}
      parent={value}
      {remembers}
      bind:pick={deeper}
      {narrowText}
    />
  {:else if narrowText.trim() && !showAll && info.status === 'ready' && info.options.length > 0}
    <p class="quiet note">
      {#if narrowed.applied}
        Showing the {options.length} with “{narrowText.trim()}” in the name.
        <button type="button" class="link" onclick={() => (showAll = true)}>Show all</button>
      {:else if !narrowed.matched}
        None has “{narrowText.trim()}” in the name, so all are shown.
      {/if}
    </p>
  {/if}
  {#if info.status === 'needs_connection'}
    <Chip kind="err" label={info.message ?? 'Not connected'} />
  {:else if empty && !optional}
    <Chip kind="warn" label="Nothing to post in yet. Save locally keeps it on this computer." />
  {/if}
</div>

<style>
  .dd { display: grid; gap: 4px; align-content: start; }
  label { font-weight: 600; font-size: 13px; }
  select { width: 100%; }
  .row { display: flex; gap: 6px; align-items: center; min-width: 0; }
  .row > :global(.combo) { flex: 1 1 0; min-width: 90px; }
  .step { flex: 0 1 34%; min-width: 0; width: auto; text-overflow: ellipsis; }
  .note { margin: 0; font-size: 12px; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font: inherit; }
</style>
