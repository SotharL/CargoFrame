<script lang="ts">
  import { t, tn } from '../../lib/i18n/index.svelte';
  // Tags, like Jira's labels: chips with a × each, a box to type a new one (Enter, a comma or Tab
  // adds it; Backspace in the empty box takes the last one out), at most 25. Under the box: the
  // last five tags used here. With a list of the service (Jira's labels or components), the box
  // opens it when clicked: several can be picked one after the other, typing narrows it, the
  // arrows and Enter pick from it. A tag is the entry's name, as the service shows it.
  import { onMount } from 'svelte';
  import { MAX_TAGS, MAX_TAG_CHARS, type Block, type DropdownOption } from '../../lib/types';
  import type { Recent } from '../../lib/recent';

  type TagsBlock = Extract<Block, { type: 'tags' }>;

  let {
    block,
    tags = $bindable([]),
    recent = [],
    over = 0,
    suggest,
  }: {
    block: TagsBlock;
    tags: string[];
    /** The last tags used in this block, the latest first. */
    recent?: Recent[];
    /** How many tags the capture had beyond the limit. */
    over?: number;
    /** Loads the service's own tags, when the block suggests from a list. */
    suggest?: () => Promise<DropdownOption[]>;
  } = $props();

  const id = $derived(`tags-${block.id}`);
  let typed = $state('');
  let input = $state<HTMLInputElement>();
  let listed = $state<DropdownOption[]>([]);
  let problem = $state('');
  let open = $state(false);
  let active = $state(0);
  const listId = $derived(`${id}-list`);

  onMount(() => {
    if (!suggest) return;
    suggest()
      .then((o) => (listed = o))
      .catch((e) => (problem = t('The tags of the service could not be listed: {problem}', { problem: String(e) })));
  });

  const has = (t: string) => tags.some((x) => x.toLowerCase() === t.toLowerCase());
  const full = $derived(tags.length >= MAX_TAGS);

  /** A tag as it is kept: one space between words, no comma, not too long. */
  const clean = (raw: string) => raw.replace(/[,;\n]/g, ' ').split(/\s+/).filter(Boolean).join(' ').slice(0, MAX_TAG_CHARS).trim();

  /** Add what was typed or pasted: commas and lines make several tags. */
  function add(text: string) {
    const next = [...tags];
    for (const part of text.split(/[,;\n]/)) {
      const t = clean(part);
      if (!t || next.length >= MAX_TAGS || next.some((x) => x.toLowerCase() === t.toLowerCase())) continue;
      next.push(t);
    }
    tags = next;
    typed = '';
  }

  function remove(i: number) {
    tags = tags.filter((_, n) => n !== i);
    input?.focus();
  }

  function onKey(e: KeyboardEvent) {
    if (showList && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      const n = matches.length;
      active = (active + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      return;
    }
    if (showList && e.key === 'Enter' && !e.isComposing && matches[active]) {
      e.preventDefault();
      pick(matches[active]!);
      return;
    }
    if (e.key === 'Escape' && open) {
      e.preventDefault();
      e.stopPropagation();
      open = false;
      return;
    }
    if (listed.length && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      open = true;
      return;
    }
    if ((e.key === 'Enter' || e.key === ',' || (e.key === 'Tab' && typed.trim())) && !e.isComposing) {
      if (!typed.trim()) return;
      e.preventDefault();
      add(typed);
    } else if (e.key === 'Backspace' && typed === '' && tags.length > 0) {
      e.preventDefault();
      tags = tags.slice(0, -1);
    }
  }

  function onPaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData('text/plain') ?? '';
    if (!/[,;\n]/.test(text)) return;
    e.preventDefault();
    add(typed + text);
  }

  /** The last ones used that are not here yet. */
  const recents = $derived(recent.map((r) => r.value).filter((t) => !has(t)).slice(0, 5));
  /** The service's entries not picked yet that match what is typed (all of them, when nothing is). */
  const matches = $derived.by(() => {
    const q = typed.trim().toLowerCase();
    return listed.filter(
      (o) => !has(o.label) && !has(o.value) && (!q || o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q)),
    );
  });
  const showList = $derived(open && !full && matches.length > 0);

  /** Pick an entry of the service's list: its name becomes a tag, and the list stays open for the next. */
  function pick(o: DropdownOption) {
    add(o.label || o.value);
    active = Math.min(active, Math.max(0, matches.length - 2));
    open = true;
    input?.focus();
  }
</script>

<div class="tags">
  <label for={id}>{block.title}<span class="quiet count">{tags.length}/{MAX_TAGS}</span></label>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="box" data-content onclick={() => input?.focus()}>
    <ul class="chips" aria-label={`${block.title || 'Tags'}: ${tags.length} tags`}>
      {#each tags as item, i (item)}
        <li class="chip">
          <span class="selectable">{item}</span>
          <button type="button" class="x" aria-label={t('Remove {name}', { name: item })} onclick={() => remove(i)}>×</button>
        </li>
      {/each}
    </ul>
    <input
      {id}
      bind:this={input}
      bind:value={typed}
      onkeydown={onKey}
      onpaste={onPaste}
      onfocus={() => (open = listed.length > 0)}
      oninput={() => {
        open = listed.length > 0;
        active = 0;
      }}
      onblur={() => {
        open = false;
        if (typed.trim()) add(typed);
      }}
      role={listed.length ? 'combobox' : undefined}
      aria-expanded={listed.length ? showList : undefined}
      aria-controls={listed.length ? listId : undefined}
      aria-autocomplete={listed.length ? 'list' : undefined}
      disabled={full}
      placeholder={full ? t('{max} tags, the most', { max: MAX_TAGS }) : tags.length ? t('Add a tag…') : t('Type a tag, then Enter')}
      aria-label={t('Add a tag to {what}', { what: block.title || t('the tags') })}
      autocomplete="off"
    />
  </div>
  {#if showList}
    <ul class="list" id={listId} role="listbox" aria-label={t('Tags of the service')} aria-multiselectable="true">
      {#each matches as o, i (o.value)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          role="option"
          aria-selected={i === active}
          class:active={i === active}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => pick(o)}
        >{o.label}</li>
      {/each}
    </ul>
  {/if}
  {#if recents.length && !full && !showList}
    <div class="offer" role="group" aria-label={t('Last tags used')}>
      <span class="quiet">{t('Last used:')}</span>
      {#each recents as item (item)}
        <button type="button" class="pick" onclick={() => add(item)}>{item}</button>
      {/each}
    </div>
  {/if}
  {#if over > 0}<p class="quiet note">{tn('{n} more tag was left out: a block holds {max}.', t('{n} more tags were left out: a block holds {max}.'), over, { max: MAX_TAGS })}</p>{/if}
  {#if problem}<p class="quiet note">{problem}</p>{/if}
</div>

<style>
  .tags { display: grid; gap: 4px; align-content: start; min-height: 0; }
  label { font-weight: 600; font-size: 13px; display: flex; gap: 8px; align-items: baseline; }
  .count { font-weight: 400; font-size: 12px; }
  .box {
    display: flex; flex-wrap: wrap; gap: 4px; align-items: center; padding: 4px 6px; min-height: 32px;
    border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); cursor: text;
  }
  .box:focus-within { outline: 2px solid var(--accent); outline-offset: -1px; }
  .chips { display: contents; list-style: none; margin: 0; padding: 0; }
  .chip {
    display: inline-flex; align-items: center; gap: 2px; padding: 1px 2px 1px 8px; border-radius: 4px;
    background: color-mix(in srgb, var(--accent) 16%, var(--surface)); font-size: 12.5px; max-width: 100%;
  }
  .chip span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x { border: 0; background: transparent; padding: 0 4px; font-size: 14px; line-height: 1; cursor: pointer; color: inherit; }
  .x:hover { color: var(--err); }
  input { flex: 1; min-width: 90px; border: 0; background: transparent; padding: 2px; font: inherit; font-size: 13px; outline: none; }
  .list {
    list-style: none; margin: 0; padding: 4px; max-height: 180px; overflow-y: auto; font-size: 13px;
    border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface);
  }
  .list li { padding: 3px 8px; border-radius: 4px; cursor: pointer; }
  .list li.active, .list li:hover { background: color-mix(in srgb, var(--accent) 14%, var(--surface)); }
  .offer { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; font-size: 12px; }
  .pick { padding: 1px 8px; font-size: 12px; border-radius: 999px; }
  .note { margin: 0; font-size: 12px; }
  /* Room at its end for the dots of the sharing buttons (App sets --corner). */
  .tags > label { padding-right: var(--corner, 0px); }
</style>
