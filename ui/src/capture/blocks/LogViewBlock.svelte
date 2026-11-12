<script lang="ts">
  // A log viewer: one of the files the window offers, line by line. The search row shows only the
  // lines with its words in them (case aside); Copy all copies those. When the lines say when
  // they were written, the button at the start of the row turns it from a text search (the
  // magnifier) into a time search (the clock), which keeps the lines of a moment: during the
  // recording (or around the capture), or between two times. Both searches keep applying; the
  // button shows a dot while the one not shown is on. A line picked is shown whole under the list,
  // to read and copy. "Open large" fills the window with it. Nothing of it is sent: the files go
  // as the Files blocks say.
  import { untrack } from 'svelte';
  import type { Api } from '../../lib/ipc';
  import type { Block, LogText, SessionView } from '../../lib/types';
  import { formatSize } from '../values';
  import { matching, splitLines, type LogLine } from './logLines';
  import { during, hms, onDay, stamps, type Span } from './logTimes';

  type LogViewBlock = Extract<Block, { type: 'log_view' }>;

  let {
    block,
    view,
    files,
    api,
  }: {
    block: LogViewBlock;
    view: SessionView;
    /** The ticked files of each Files block: they come first in the list. */
    files: Record<string, string[]>;
    api: Api | null;
  } = $props();

  /** The most lines drawn at once: the last ones that match. */
  const SHOWN = 2000;

  /** Every file the window offers, ticked ones first, each once. */
  const offered = $derived.by(() => {
    const ticked = Object.values(files).flat();
    const all = Object.values(view.files).flatMap((f) => f.entries.map((e) => e.path));
    return [...new Set([...ticked, ...all])];
  });

  let path = $state('');
  let loaded = $state<LogText | null>(null);
  let problem = $state('');
  let loading = $state(false);
  // the filter the layout starts it with; the person changes it from there
  let query = $state(untrack(() => block.filter ?? ''));
  let picked = $state<number | null>(null);
  let large = $state(false);
  let copied = $state<'line' | 'all' | null>(null);

  // The moment of the capture: the recording (a few seconds either side, as the clock and the log
  // may not agree to the second), or the minute before a screenshot.
  const MARGIN = 5_000;
  const recorded = $derived(view.video?.started_at_ms ? { from: view.video.started_at_ms, to: view.video.started_at_ms + view.video.duration_ms } : null);
  const moment = $derived<Span | null>(
    recorded
      ? { from: recorded.from - MARGIN, to: recorded.to + MARGIN }
      : view.captured_at_ms
        ? { from: view.captured_at_ms - 60_000, to: view.captured_at_ms + MARGIN }
        : null,
  );
  const day = $derived(view.video?.started_at_ms ?? view.captured_at_ms ?? Date.now());
  let utc = $state(untrack(() => block.utc === true));
  let when = $state<'all' | 'moment' | 'between'>(untrack(() => (block.during ? 'moment' : 'all')));
  /** Which search the row shows: the words (always at first, even when the block starts on the
   *  recording: the time search then applies, and the clock says so with its dot), or the time. */
  let mode = $state<'text' | 'time'>('text');
  let fromText = $state('');
  let toText = $state('');

  // start on the first ticked file (a log, if there is one)
  $effect(() => {
    if (path || offered.length === 0) return;
    path = offered.find((p) => /\.(log|txt)$/i.test(p)) ?? offered[0]!;
  });

  let latest = 0;
  $effect(() => {
    const p = path;
    if (!p || !api) return;
    const mine = ++latest;
    loading = true;
    problem = '';
    picked = null;
    api
      .readOfferedFile(view.session_id, p)
      .then((t) => mine === latest && (loaded = t))
      .catch((e) => mine === latest && ((loaded = null), (problem = String(e))))
      .finally(() => mine === latest && (loading = false));
  });

  const all = $derived<LogLine[]>(loaded ? splitLines(loaded.text) : []);
  const times = $derived(stamps(all, day, utc));
  const timed = $derived(times.some((t) => t !== null));
  /** The part of the day kept, when the lines are kept by time. */
  const span = $derived.by<Span | null>(() => {
    if (when === 'moment') return moment;
    if (when === 'between') {
      const from = onDay(day, fromText);
      const to = onDay(day, toText);
      return from !== null && to !== null ? { from: Math.min(from, to), to: Math.max(from, to) + 999 } : null;
    }
    return null;
  });
  const byTime = $derived(timed && span ? during(all, times, span) : all);
  const lines = $derived(matching(byTime, query));

  /** The time search starts on the moment of the capture, when there is one. */
  function switchMode() {
    mode = mode === 'text' ? 'time' : 'text';
    if (mode === 'time' && when === 'all' && moment) when = 'moment';
  }
  /** The search not shown is on: the button says so with a dot. */
  const otherOn = $derived(mode === 'text' ? !!(timed && span) : query.trim() !== '');

  /** Between two times: it starts on the moment of the capture, or on the file's own first and last. */
  function pickBetween() {
    const known = times.filter((t): t is number => t !== null);
    const from = moment?.from ?? known[0];
    const to = moment?.to ?? known[known.length - 1];
    if (!fromText && from !== undefined) fromText = hms(from);
    if (!toText && to !== undefined) toText = hms(to);
  }
  const shown = $derived(lines.length > SHOWN ? lines.slice(-SHOWN) : lines);
  const total = $derived(all.length);
  const pickedLine = $derived(picked === null ? null : (lines.find((l) => l.n === picked) ?? null));
  const words = $derived(query.trim().toLowerCase().split(/\s+/).filter(Boolean));

  /** A line cut into what matches the search and what does not, to mark the words. */
  function pieces(text: string): { t: string; hit: boolean }[] {
    if (words.length === 0) return [{ t: text, hit: false }];
    const lower = text.toLowerCase();
    const marks = new Array<boolean>(text.length).fill(false);
    for (const w of words) {
      for (let at = lower.indexOf(w); at >= 0; at = lower.indexOf(w, at + w.length)) marks.fill(true, at, at + w.length);
    }
    const out: { t: string; hit: boolean }[] = [];
    for (let i = 0; i < text.length; i++) {
      const last = out[out.length - 1];
      if (last && last.hit === marks[i]) last.t += text[i];
      else out.push({ t: text[i]!, hit: marks[i]! });
    }
    return out;
  }

  async function copy(text: string, what: 'line' | 'all') {
    try {
      await navigator.clipboard.writeText(text);
      copied = what;
      setTimeout(() => (copied = null), 1500);
    } catch {
      problem = 'The text could not be copied: select it and press Ctrl+C';
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && large) {
      e.preventDefault();
      large = false;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="log" class:large data-content role="region" aria-label={block.title || 'Log'}>
  <div class="bar">
    <span class="title">{block.title}</span>
    {#if offered.length > 1}
      <select bind:value={path} aria-label="File to show">
        {#each offered as p (p)}<option value={p}>{p}</option>{/each}
      </select>
    {:else if offered.length === 1}
      <span class="quiet mono one" title={offered[0]}>{offered[0]}</span>
    {/if}
    <button type="button" class="small" onclick={() => (large = !large)} aria-pressed={large}>{large ? 'Close' : 'Open large'}</button>
  </div>
  <div class="bar">
    <button
      type="button"
      class="mode"
      class:other={otherOn}
      onclick={switchMode}
      aria-label={mode === 'text' ? 'Search by time' : 'Search by text'}
      title={mode === 'text' ? 'Searching by text: click to search by time' : 'Searching by time: click to search by text'}
    >
      {#if mode === 'text'}
        <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="6.5" cy="6.5" r="4.5" /><path d="M10 10l4.5 4.5" /></svg>
      {:else}
        <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6.5" /><path d="M8 4v4.2l2.8 1.8" /></svg>
      {/if}
    </button>
    {#if mode === 'text'}
      <input type="search" class="search" bind:value={query} placeholder="Show only lines with…" aria-label="Show only the lines with these words" />
    {:else}
      <select class="when" bind:value={when} onchange={() => when === 'between' && pickBetween()} aria-label="Lines written">
        <option value="all">Written any time</option>
        {#if moment}
          <option value="moment">{recorded ? 'During the recording' : 'Around the capture'} ({hms(moment.from)} to {hms(moment.to)})</option>
        {/if}
        <option value="between">Written between…</option>
      </select>
      {#if when === 'between'}
        <input type="time" step="1" class="time" bind:value={fromText} aria-label="From" />
        <span class="quiet">to</span>
        <input type="time" step="1" class="time" bind:value={toText} aria-label="To" />
      {/if}
      {#if loaded && !timed}<span class="quiet none">No times in this file</span>{/if}
      <label class="utc quiet" title="The times written in this file are UTC, not this computer's time (Unreal Engine's logs are, by default)">
        <input type="checkbox" bind:checked={utc} /> UTC
      </label>
    {/if}
    <button type="button" class="small" disabled={lines.length === 0} onclick={() => copy(lines.map((l) => l.text).join('\n'), 'all')}>
      {copied === 'all' ? 'Copied' : query.trim() || (timed && span) ? `Copy ${lines.length} lines` : 'Copy all'}
    </button>
  </div>
  {#if offered.length === 0}
    <p class="quiet note">No file to show: the Files blocks of this capture list none.</p>
  {:else if problem}
    <p class="quiet note" role="alert">{problem}</p>
  {:else if loading && !loaded}
    <p class="quiet note">Reading the file…</p>
  {:else if loaded}
    <ol class="lines mono" aria-label="Lines">
      {#each shown as l (l.n)}
        <li class:on={picked === l.n}>
          <button type="button" class="line" onclick={() => (picked = picked === l.n ? null : l.n)} aria-pressed={picked === l.n}>
            <span class="n" aria-hidden="true">{l.n}</span>
            <span class="t">{#each pieces(l.text) as p, i (i)}{#if p.hit}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</span>
          </button>
        </li>
      {:else}
        <li class="quiet note">{query.trim() ? 'No line has these words.' : timed && span ? 'No line was written then.' : 'The file is empty.'}</li>
      {/each}
    </ol>
    <p class="quiet foot">
      {query.trim() || (timed && span) ? `${lines.length} of ${total} lines` : `${total} lines`}{query.trim() ? ` with “${query.trim()}”` : ''}{timed && span ? `, written ${hms(span.from)} to ${hms(span.to)}` : ''}{lines.length > SHOWN ? `, the last ${SHOWN} shown` : ''}{loaded.cut ? ` · the end of a ${formatSize(loaded.size)} file` : ''}{!timed && (mode === 'time' || when !== 'all') && total > 0 ? ' · no times CargoFrame can read in this file, so every line shows' : ''}
    </p>
    {#if pickedLine}
      <div class="picked" role="group" aria-label={`Line ${pickedLine.n}`}>
        <pre class="mono selectable">{pickedLine.text}</pre>
        <button type="button" class="small" onclick={() => copy(pickedLine.text, 'line')}>{copied === 'line' ? 'Copied' : 'Copy line'}</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  /* a column: the bars (the time one only when the lines have times), the lines taking the rest */
  .log { display: flex; flex-direction: column; gap: 4px; height: 100%; min-height: 0; background: var(--surface); }
  .log.large {
    position: fixed; inset: 16px; z-index: 50; padding: 12px; border: 1px solid var(--border-strong);
    border-radius: var(--radius); box-shadow: 0 8px 40px rgb(0 0 0 / 0.35);
  }
  .bar { display: flex; gap: 6px; align-items: center; min-width: 0; }
  .title { font-weight: 600; font-size: 13px; }
  .bar select { min-width: 0; flex: 1; font-size: 12.5px; }
  .one { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .search { flex: 1; min-width: 0; font-size: 13px; }
  .bar select.when { flex: 0 1 auto; min-width: 0; }
  .none { font-size: 12px; white-space: nowrap; }
  .mode {
    position: relative; flex: none; width: 28px; height: 26px; padding: 0; display: grid; place-items: center;
  }
  .mode svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; }
  /* the search not shown is on */
  .mode.other::after {
    content: ''; position: absolute; top: 3px; right: 3px; width: 6px; height: 6px; border-radius: 50%; background: var(--accent);
  }
  .time { font-size: 12.5px; padding: 1px 4px; }
  .utc { display: inline-flex; gap: 4px; align-items: center; font-size: 12px; margin-left: auto; white-space: nowrap; }
  .small { padding: 2px 8px; font-size: 12.5px; white-space: nowrap; }
  .lines {
    margin: 0; padding: 0; list-style: none; overflow: auto; min-height: 0; flex: 1 1 auto; font-size: 12px;
    border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-2);
  }
  .line { display: flex; gap: 8px; width: 100%; text-align: left; border: 0; background: transparent; padding: 0 6px; font: inherit; color: inherit; cursor: pointer; white-space: pre; }
  .line:hover { background: color-mix(in srgb, var(--accent) 8%, transparent); }
  .on .line { background: color-mix(in srgb, var(--accent) 20%, transparent); }
  .n { color: var(--quiet); min-width: 4ch; text-align: right; user-select: none; }
  .t { overflow: hidden; text-overflow: ellipsis; }
  mark { background: color-mix(in srgb, #ffd400 55%, transparent); color: inherit; border-radius: 2px; }
  .foot { margin: 0; font-size: 11.5px; }
  .picked { display: flex; gap: 6px; align-items: flex-start; }
  .picked pre {
    flex: 1; margin: 0; padding: 6px 8px; max-height: 8em; overflow: auto; font-size: 12px; white-space: pre-wrap; overflow-wrap: anywhere;
    border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface-2);
  }
  .note { margin: 0; padding: 6px; font-size: 12.5px; }
</style>
