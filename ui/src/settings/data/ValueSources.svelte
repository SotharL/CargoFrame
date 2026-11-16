<script lang="ts">
  // One value and the files it is read in: what each gives in the last capture's folder, which one
  // is used, how several are settled, and what it is when no file has it.
  import { getApi } from '../../lib/ipc';
  import type { ItemEntry, Layout, Rule, RuleAcross, RuleTest, RuleValue, ValuesPreview } from '../../lib/types';
  import { describe } from '../ruleValues';
  import { layoutsUsingRule, usedIn } from '../usage';
  import Choices from './Choices.svelte';
  import { describePick } from './pick';
  import { removeSource, removeValue, replaceStep, setAll, type Changes, type Source, type ValueEntry } from './values';

  let {
    entry,
    rules,
    layouts,
    preview,
    apply,
    onadd,
    onchangesource,
    onback,
  }: {
    entry: ValueEntry;
    rules: ItemEntry<Rule>[];
    layouts: ItemEntry<Layout>[];
    preview: ValuesPreview | null;
    apply: (c: Changes) => Promise<void>;
    onadd: () => void;
    onchangesource: (s: Source) => void;
    onback: () => void;
  } = $props();

  let tests = $state<Record<string, RuleTest>>({});
  let failure = $state('');
  let confirming = $state(false);
  let fallback = $state('');
  let writing = $state(false);
  /** A value found the way of 0.4 (after a name, between two texts…), changed here. */
  let legacy = $state<{ ruleId: string; step: RuleValue } | null>(null);

  $effect(() => {
    fallback = entry.fallback ?? '';
    writing = entry.fallback !== null;
  });

  // what each file gives, tried again when the value changes
  $effect(() => {
    const list = [...new Map(entry.sources.map((s) => [s.ruleId, s.rule])).entries()];
    let gone = false;
    void (async () => {
      const api = await getApi();
      const out: Record<string, RuleTest> = {};
      for (const [id, rule] of list) {
        const t = await api.testRule(rule, null).catch(() => null);
        if (t) out[id] = t;
      }
      if (!gone) tests = out;
    })();
    return () => (gone = true);
  });

  const result = (s: Source) => tests[s.ruleId]?.values?.find((v) => v.key === entry.key);
  const fileFound = (s: Source) => tests[s.ruleId]?.file ?? null;
  const has = (s: Source) => {
    const r = result(s);
    return !!r && r.value !== null && !r.used_fallback;
  };
  const used = (s: Source) =>
    has(s) && (entry.across === 'all' || (!!preview?.sources[entry.key] && fileFound(s) === preview.sources[entry.key]));
  const how = (s: Source) => (s.step.find.type === 'pick' ? describePick(s.step.find, s.step.which) : describe(s.step.find, s.rule));
  const usedBy = $derived(usedIn(layoutsUsingRule(layouts, entry.key, [entry.key])));

  async function run(c: Changes) {
    failure = '';
    try {
      await apply(c);
    } catch (e) {
      failure = String(e);
    }
  }

  const ACROSS: { value: RuleAcross; label: string }[] = [
    { value: 'newest', label: 'the newest file' },
    { value: 'all', label: 'all, when they differ' },
  ];

  function setFallback(text: string | null) {
    void run(setAll(rules, entry.key, { fallback: text }));
  }

  async function removeFrom(s: Source) {
    await run(entry.sources.length === 1 ? removeValue(rules, entry.key) : removeSource(rules, s.ruleId, entry.key));
    if (entry.sources.length === 1) onback();
  }
</script>

<div class="sources">
  <header>
    <button type="button" class="back" onclick={onback}>← Values</button>
    <h2><code class="token">{`{${entry.key}}`}</code> {entry.name}</h2>
    <p class="quiet">{usedBy}</p>
  </header>

  <section class="box">
    <h3>Read in</h3>
    {#each entry.sources as s (s.ruleId)}
      <div class="source" class:used={used(s)}>
        <div class="what">
          <b class="mono">{s.rule.file || s.rule.name}</b>
          <span class="quiet small">{how(s)}</span>
          {#if legacy && legacy.ruleId === s.ruleId}
            {@const f = legacy.step.find}
            <form class="legacy" onsubmit={(e) => { e.preventDefault(); void run(replaceStep(rules, legacy!.ruleId, $state.snapshot(legacy!.step) as RuleValue)); legacy = null; }}>
              {#if f.type === 'value_of'}
                <input type="text" bind:value={f.name} aria-label="The name written before the value, or a key" />
              {:else if f.type === 'between'}
                <input type="text" bind:value={f.start} aria-label="The text before the value" />
                <input type="text" value={f.end ?? ''} oninput={(e) => (f.end = e.currentTarget.value || null)} placeholder="the end of the line" aria-label="The text after the value" />
              {:else if f.type === 'line_with'}
                <input type="text" bind:value={f.text} aria-label="A text the line holds" />
              {/if}
              <button type="submit" class="small">Save</button>
              <button type="button" class="small" onclick={() => (legacy = null)}>Cancel</button>
            </form>
          {/if}
        </div>
        <div class="got">
          {#if !tests[s.ruleId]}
            <span class="quiet small">…</span>
          {:else if has(s)}
            <span class="mono value" title={result(s)?.value ?? ''}>{result(s)?.value}</span>
            <span class="quiet small">{fileFound(s) ?? ''}</span>
          {:else}
            <span class="quiet small">{fileFound(s) ? 'Not found in it' : 'Not in this capture'}</span>
          {/if}
        </div>
        <span class="badge" class:on={used(s)} class:older={has(s) && !used(s)}>{used(s) ? 'Used' : has(s) ? 'Older' : ''}</span>
        <div class="actions">
          {#if ['value_of', 'between', 'line_with'].includes(s.step.find.type)}
            <button type="button" class="small" onclick={() => (legacy = { ruleId: s.ruleId, step: structuredClone($state.snapshot(s.step) as RuleValue) })}>Edit</button>
          {/if}
          <button type="button" class="small" onclick={() => onchangesource(s)}>✎ Select again</button>
          <button type="button" class="small" aria-label={`Stop reading it in ${s.rule.file}`} onclick={() => removeFrom(s)}>×</button>
        </div>
      </div>
    {/each}
    <div><button type="button" onclick={onadd}>＋ Read it in another file</button></div>
  </section>

  <section class="box">
    <div class="line">
      <h3>When several files have it, take</h3>
      <Choices label="When several files have it" value={entry.across} options={ACROSS} onchange={(v) => run(setAll(rules, entry.key, { across: v }))} />
    </div>
    <div class="line">
      <h3>When no file has it</h3>
      <Choices
        label="When no file has it"
        value={writing ? 'text' : 'empty'}
        options={[{ value: 'empty', label: 'leave it empty' }, { value: 'text', label: 'write a text' }]}
        onchange={(v) => { writing = v === 'text'; if (!writing) setFallback(null); }}
      />
      {#if writing}
        <input
          type="text"
          bind:value={fallback}
          placeholder="unknown"
          aria-label="Text when no file has it"
          onchange={() => setFallback(fallback)}
        />
      {/if}
    </div>
  </section>

  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  <div class="buttons">
    {#if confirming}
      <button type="button" class="danger" onclick={async () => { await run(removeValue(rules, entry.key)); onback(); }}>Delete {`{${entry.key}}`} for good</button>
      <button type="button" onclick={() => (confirming = false)}>Keep</button>
    {:else}
      <button type="button" onclick={() => (confirming = true)}>Delete this value</button>
    {/if}
  </div>
</div>

<style>
  .sources { display: grid; gap: 12px; max-width: 900px; }
  header { display: grid; gap: 4px; justify-items: start; }
  header h2 { margin: 0; font-size: 18px; display: flex; gap: 8px; align-items: center; }
  header p { margin: 0; font-size: 13px; }
  .back { border: 0; background: none; padding: 0; color: var(--accent); font-size: 13px; }
  .box { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 14px; display: grid; gap: 10px; }
  .box h3 { margin: 0; font-size: 14px; }
  .token { font: 13px var(--mono); padding: 2px 8px; border-radius: 5px; background: color-mix(in srgb, var(--accent) 14%, var(--surface)); border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); }
  .source {
    display: grid; grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr) 56px auto; gap: 4px 12px; align-items: center;
    padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  }
  .source.used { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 7%, var(--surface)); }
  .what, .got { display: grid; gap: 2px; min-width: 0; }
  .what .small, .got .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .value { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .mono { font-family: var(--mono); }
  .small { font-size: 12.5px; }
  button.small { padding: 2px 8px; }
  .badge { visibility: hidden; justify-self: center; font-size: 11.5px; font-weight: 600; padding: 1px 8px; border-radius: 99px; }
  .badge.on { visibility: visible; background: var(--accent); color: var(--on-accent); }
  .badge.older { visibility: visible; background: var(--border); color: var(--quiet); }
  .actions { display: flex; gap: 6px; justify-content: flex-end; flex-wrap: wrap; }
  .legacy { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 4px; }
  .legacy input { flex: 1 1 120px; min-width: 0; }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; }
  .line h3 { font-size: 13px; font-weight: 600; min-width: 220px; }
  .line input { flex: 1 1 160px; max-width: 280px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; margin: 0; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
</style>
