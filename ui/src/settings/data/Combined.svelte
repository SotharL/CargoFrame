<script lang="ts">
  // Values made from other values, in no file: `{stream}_{change}`, with choices tried from the
  // top and conditions ("if {shelf} is found"). Stored as a rule that reads no file
  // (core/rules/src/combine.rs).
  import Field from '../../lib/components/Field.svelte';
  import type { Rule, RuleChoice, RuleCondTest, RuleValue, VariableInfo } from '../../lib/types';
  import { token } from '../../lib/valueNames';
  import { blankCombined, newValue, slug, unique } from '../ruleValues';

  let {
    id,
    rule,
    taken,
    variables,
    onsave,
    oncancel,
  }: {
    id: string | null;
    /** null: a new one. */
    rule: Rule | null;
    /** The keys of the other values. */
    taken: string[];
    variables: VariableInfo[];
    onsave: (id: string | null, rule: Rule) => Promise<void>;
    oncancel: () => void;
  } = $props();

  // the rule as it was when the editor opened
  function first(): Rule {
    return rule ? structuredClone($state.snapshot(rule) as Rule) : blankCombined(taken);
  }
  const start = first();
  let draft = $state<Rule>(start);
  /** A new value's key follows its name. */
  let auto = $state<Record<string, boolean>>(firstAuto());
  function firstAuto(): Record<string, boolean> {
    return rule ? {} : { [start.values![0]!.key]: true };
  }
  let busy = $state(false);
  let failure = $state('');

  const COND_TESTS: { value: RuleCondTest; label: string; needsText: boolean }[] = [
    { value: 'found', label: 'is found', needsText: false },
    { value: 'missing', label: 'is not found', needsText: false },
    { value: 'is', label: 'is', needsText: true },
    { value: 'is_not', label: 'is not', needsText: true },
    { value: 'contains', label: 'contains', needsText: true },
  ];
  const needsText = (t: RuleCondTest) => COND_TESTS.find((x) => x.value === t)?.needsText ?? false;

  const takenBy = (except?: RuleValue) => [...taken, ...(draft.values ?? []).filter((v) => v !== except).map((v) => v.key)];

  function renamed(v: RuleValue) {
    if (!auto[v.key]) return;
    const old = v.key;
    v.key = unique(slug(v.name), takenBy(v));
    delete auto[old];
    auto[v.key] = true;
  }

  function addValue() {
    const v = { ...newValue(takenBy()), find: { type: 'combine' as const, choices: [{ text: '' }] } };
    draft.values = [...(draft.values ?? []), v];
    auto[v.key] = true;
  }

  function moveChoice(choices: RuleChoice[], i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= choices.length) return;
    [choices[i], choices[j]] = [choices[j]!, choices[i]!];
  }

  async function save() {
    busy = true;
    failure = '';
    try {
      const value = $state.snapshot(draft) as Rule;
      if (!value.name.trim()) value.name = value.values?.[0]?.name || 'Made from other values';
      await onsave(id, value);
    } catch (e) {
      failure = String(e);
    }
    busy = false;
  }
</script>

<form class="editor" onsubmit={(e) => { e.preventDefault(); void save(); }}>
  <button type="button" class="back" onclick={oncancel}>← Values</button>
  <h2>{id ? 'Values made from other values' : 'New value made from other values'}</h2>
  <p class="quiet small">Tried from the top: the first choice that fits gives the value. A choice with no condition fits when every value it names is found, so put the one with the most values first.</p>

  {#each draft.values ?? [] as v, i (i)}
    {#if v.find.type === 'combine'}
      {@const choices = v.find.choices}
      <div class="value">
        <div class="line">
          <input type="text" class="name" value={v.name} oninput={(e) => { v.name = e.currentTarget.value; renamed(v); }} placeholder="Build id" aria-label="Name of the value" />
          <code class="token">{token(v.key)}</code>
          {#if (draft.values?.length ?? 0) > 1}
            <button type="button" class="tiny end" aria-label={`Remove ${v.name || 'this value'}`} onclick={() => (draft.values = draft.values!.filter((_, n) => n !== i))}>×</button>
          {/if}
        </div>
        {#each choices as c, ci (ci)}
          <div class="choice">
            <div class="line">
              <span class="word">{ci === 0 ? 'Use' : 'else'}</span>
              <input type="text" class="grow mono" bind:value={c.text} list="combine-values" placeholder={'{branch}_{build}'} aria-label={`Choice ${ci + 1}`} />
              <button type="button" class="tiny" aria-label={`Move choice ${ci + 1} up`} disabled={ci === 0} onclick={() => moveChoice(choices, ci, -1)}>↑</button>
              <button type="button" class="tiny" aria-label={`Move choice ${ci + 1} down`} disabled={ci === choices.length - 1} onclick={() => moveChoice(choices, ci, 1)}>↓</button>
              <button type="button" class="tiny" aria-label={`Remove choice ${ci + 1}`} disabled={choices.length === 1} onclick={() => choices.splice(ci, 1)}>×</button>
            </div>
            {#each c.when ?? [] as w, wi (wi)}
              <div class="line cond">
                <span class="word quiet">{wi === 0 ? 'if' : 'and'}</span>
                <input type="text" class="name mono" bind:value={w.value} list="combine-values" placeholder="shelf" aria-label={`Value condition ${wi + 1} of choice ${ci + 1} looks at`} />
                <select bind:value={w.test} aria-label={`Test of condition ${wi + 1} of choice ${ci + 1}`}>
                  {#each COND_TESTS as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
                </select>
                {#if needsText(w.test)}
                  <input type="text" class="grow" bind:value={w.text} placeholder="Release" aria-label={`Text of condition ${wi + 1} of choice ${ci + 1}`} />
                {/if}
                <button type="button" class="tiny" aria-label={`Remove condition ${wi + 1} of choice ${ci + 1}`} onclick={() => c.when?.splice(wi, 1)}>×</button>
              </div>
            {/each}
            <div><button type="button" class="link" onclick={() => (c.when = [...(c.when ?? []), { value: '', test: 'found', text: '' }])}>Add a condition</button></div>
          </div>
        {/each}
        <div><button type="button" onclick={() => choices.push({ text: '' })}>Add a choice</button></div>
        <div class="line">
          <span class="word quiet">If none fits</span>
          <input type="text" class="grow" value={v.fallback ?? ''} oninput={(e) => (v.fallback = e.currentTarget.value === '' ? null : e.currentTarget.value)} placeholder="leave it empty" aria-label="Value if no choice fits" />
        </div>
      </div>
    {/if}
  {/each}
  <div><button type="button" onclick={addValue}>Add a value</button></div>

  <Field label="Group name" hint="Shown when something cannot be made. Optional.">
    <input type="text" bind:value={draft.name} placeholder="Build id" />
  </Field>

  <datalist id="combine-values">
    {#each variables as x (x.name)}<option value={token(x.name)}>{x.description}</option>{/each}
  </datalist>

  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  <div class="buttons">
    <button class="primary" type="submit" disabled={busy}>Save</button>
    <button type="button" onclick={oncancel}>Cancel</button>
  </div>
</form>

<style>
  .editor { display: grid; gap: 12px; max-width: 860px; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px; justify-items: stretch; }
  .editor > .back { justify-self: start; }
  h2 { margin: 0; font-size: 18px; }
  .back { border: 0; background: none; padding: 0; color: var(--accent); font-size: 13px; }
  .value { display: grid; gap: 6px; padding: 10px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-2); }
  .choice { display: grid; gap: 4px; padding: 8px; border: 1px dashed var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); }
  .line { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .line select { width: auto; }
  .grow { flex: 1; min-width: 120px; }
  .name { width: 160px; }
  .word { font-size: 13px; }
  .cond { padding-left: 18px; }
  .tiny { width: 26px; height: 26px; padding: 0; }
  .end { margin-left: auto; }
  .token { font: 12.5px var(--mono); padding: 2px 8px; border-radius: 5px; background: color-mix(in srgb, var(--accent) 14%, var(--surface)); }
  .mono { font-family: var(--mono); }
  .small { font-size: 12.5px; margin: 0; }
  .link { border: 0; background: transparent; color: var(--accent); padding: 0; text-decoration: underline; font-size: 12.5px; }
  .buttons { display: flex; gap: 8px; }
  .error { color: var(--err); font-size: 13px; }
</style>
