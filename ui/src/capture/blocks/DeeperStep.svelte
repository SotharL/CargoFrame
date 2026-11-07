<script lang="ts">
  // The second step of a path: what is inside the first choice (the threads of a channel, the
  // posts of a forum). "New" is always the first entry; the list can be long, so it is searched,
  // narrowed by the title being typed, and a link can be pasted for one that is not listed.
  import Combobox from '../../lib/components/Combobox.svelte';
  import { getApi } from '../../lib/ipc';
  import { recentChoices, deeperKey, type Recent } from '../../lib/recent';
  import type { ComboOption, DropdownOption } from '../../lib/types';
  import type { DeeperPick } from '../values';
  import { narrow } from './cascade';

  let {
    sessionId,
    blockId,
    layoutId = null,
    parent,
    title = 'Thread or post',
    remembers = true,
    pick = $bindable({ value: '', label: '' }),
    narrowText = '',
  }: {
    sessionId: number;
    blockId: string;
    layoutId?: string | null;
    /** The first choice (a channel): the list is the one inside it. */
    parent: string;
    title?: string;
    /** Start on the post used last time in this parent. */
    remembers?: boolean;
    pick?: DeeperPick;
    /** The words of the text block that narrows the list (a post named like the title). */
    narrowText?: string;
  } = $props();

  const NEW = '';
  const NEW_LABEL = 'New message or post';

  let status = $state<'loading' | 'ready' | 'failed'>('loading');
  let failure = $state('');
  let listed = $state<DropdownOption[]>([]);
  let showAll = $state(false);
  let asked = '';

  const recent = $derived<Recent[]>(recentChoices(layoutId, deeperKey(blockId, parent)));
  const narrowed = $derived(showAll ? { options: listed, applied: false, matched: true } : narrow(listed, narrowText));
  const choices = $derived.by<ComboOption[]>(() => {
    const here = new Set(narrowed.options.map((o) => o.value));
    const kept = recent.filter((r) => !listed.some((o) => o.value === r.value) || here.has(r.value));
    const out: ComboOption[] = [{ value: NEW, label: NEW_LABEL, group: null }];
    for (const r of kept) out.push({ value: r.value, label: r.label, group: 'Used before' });
    const seen = new Set(kept.map((r) => r.value));
    for (const o of narrowed.options) if (!seen.has(o.value)) out.push({ value: o.value, label: o.label, group: o.group });
    return out;
  });
  const labelOf = (value: string) => choices.find((c) => c.value === value)?.label ?? value;

  // A new parent (a channel just chosen) starts over: its own list, and the post used last in it.
  $effect(() => {
    const now = parent;
    if (asked === now) return;
    asked = now;
    status = 'loading';
    failure = '';
    listed = [];
    showAll = false;
    const last = remembers ? recentChoices(layoutId, deeperKey(blockId, now))[0] : undefined;
    pick = last ? { value: last.value, label: last.label } : { value: NEW, label: NEW_LABEL };
    void getApi()
      .then((api) => api.fetchOptions(sessionId, blockId, now))
      .then((options) => {
        if (asked !== now) return;
        listed = options;
        status = 'ready';
      })
      .catch((e: unknown) => {
        if (asked !== now) return;
        // not being able to list is not a dead end: the post can still be a new one or a pasted link
        failure = e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : String(e);
        status = 'failed';
      });
  });

  // A link or an id pasted for a post that is not listed is a choice too; half a word is not.
  const isLink = (v: string) => /^https?:\/\//i.test(v.trim()) || /^\d{15,25}$/.test(v.trim());
  const stray = $derived(pick.value !== NEW && !choices.some((c) => c.value === pick.value) && !isLink(pick.value));
  const empty = $derived(status === 'ready' && listed.length === 0 && recent.length === 0);
  const placeholder = $derived(
    status === 'loading'
      ? 'Looking for threads…'
      : listed.length > 8
        ? `Search ${listed.length} or paste a link…`
        : 'Pick one, or paste a link',
  );

  function change(value: string) {
    pick = { value, label: value === NEW ? NEW_LABEL : labelOf(value) };
  }
</script>

<div class="deeper">
  <label for="deeper-{blockId}">{title}</label>
  <Combobox
    id="deeper-{blockId}"
    options={choices}
    value={pick.value}
    free
    disabled={status === 'loading'}
    {placeholder}
    onchange={change}
    emptyText="Nothing matches: paste a link to use a post that is not listed"
  />
  {#if narrowText.trim() && !showAll && listed.length > 0}
    <p class="quiet note">
      {#if narrowed.applied}
        Showing the {narrowed.options.length} with “{narrowText.trim()}” in the name.
        <button type="button" class="link" onclick={() => (showAll = true)}>Show all</button>
      {:else if !narrowed.matched}
        None has “{narrowText.trim()}” in the name, so all are shown.
      {/if}
    </p>
  {/if}
  {#if stray}
    <p class="quiet note" role="note">“{pick.value.trim()}” is not in the list and is not a link: pick one, or paste a link.</p>
  {/if}
  {#if status === 'failed'}
    <p class="quiet note" role="note">{failure}</p>
  {:else if empty}
    <p class="quiet note">No threads are listed here. Paste a link to post in one, or leave it to start a new one.</p>
  {/if}
</div>

<style>
  .deeper { display: grid; gap: 4px; align-content: start; }
  label { font-weight: 600; font-size: 13px; }
  .note { margin: 0; font-size: 12px; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font: inherit; }
</style>
