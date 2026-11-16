<script lang="ts">
  // Every value, with where it is read, how, what it gave in the last capture and where it is used.
  import type { ItemEntry, Layout, ValuesPreview } from '../../lib/types';
  import { token } from '../../lib/valueNames';
  import { describe } from '../ruleValues';
  import { layoutsUsingRule } from '../usage';
  import { baseName } from './files';
  import { describePick } from './pick';
  import { filesOf, type Source, type ValueEntry } from './values';

  let {
    entries,
    preview,
    layouts,
    onopen,
    q = '',
  }: {
    entries: ValueEntry[];
    /** What the list is narrowed to. */
    q?: string;
    preview: ValuesPreview | null;
    layouts: ItemEntry<Layout>[];
    onopen: (v: ValueEntry) => void;
  } = $props();

  const shown = $derived(
    entries.filter((v) => {
      const t = q.trim().toLowerCase();
      return !t || v.key.toLowerCase().includes(t) || v.name.toLowerCase().includes(t) || filesOf(v).some((f) => f.toLowerCase().includes(t));
    }),
  );

  const howOne = (s: Source) => (s.step.find.type === 'pick' ? describePick(s.step.find, s.step.which) : describe(s.step.find, s.rule));
  function how(v: ValueEntry): string {
    if (v.combined) return v.sources.map((s) => howOne(s)).join('; ');
    const all = [...new Set(v.sources.map(howOne))];
    return all.length === 1 ? all[0]! : 'A different way in each file';
  }
  function from(v: ValueEntry): { main: string; note: string } {
    if (v.combined) return { main: 'Other values', note: 'no file read' };
    const files = filesOf(v);
    const main = files.length > 3 ? `${files.slice(0, 3).join(', ')} and ${files.length - 3} more` : files.join(', ');
    const note = files.length > 1 ? (v.across === 'all' ? `${files.length} files · every different value` : `${files.length} files · the newest wins`) : '';
    return { main, note };
  }
  function last(v: ValueEntry): { value: string; note: string } | null {
    const value = preview?.values[`rule.${v.key}`];
    if (value === undefined || value === '') {
      const why = preview?.problems[v.key];
      return why ? { value: '', note: why } : null;
    }
    const src = preview?.sources[v.key];
    return { value, note: src ? baseName(src) : v.combined ? 'made from other values' : '' };
  }
  const used = (v: ValueEntry) => {
    const n = layoutsUsingRule(layouts, v.key, [v.key]);
    return n.length ? n.join(', ') : 'not used yet';
  };
</script>

<div class="values">
  <div class="table">
    <table>
      <thead>
        <tr><th>Value</th><th>Read from</th><th>How</th><th>Last capture</th><th>Used in</th></tr>
      </thead>
      <tbody>
        {#each shown as v (v.key)}
          {@const f = from(v)}
          {@const l = last(v)}
          <tr>
            <td>
              <button type="button" class="open" onclick={() => onopen(v)} aria-label={`Open ${v.name}`}>
                <code>{token(v.key)}</code>
                <span class="name">{v.name}</span>
              </button>
            </td>
            <td><div class="cut" title={f.main}>{f.main}</div>{#if f.note}<div class="note">{f.note}</div>{/if}</td>
            <td class="how"><div class="cut2" title={how(v)}>{how(v)}</div></td>
            <td>
              {#if l && l.value}
                <div class="mono cut" title={l.value}>{l.value}</div>
                {#if l.note}<div class="note cut">{l.note}</div>{/if}
              {:else}
                <div class="empty"><span class="dot" aria-hidden="true"></span>Empty</div>
                {#if l?.note}<div class="note cut" title={l.note}>{l.note}</div>{/if}
              {/if}
            </td>
            <td class="used">{used(v)}</td>
          </tr>
        {:else}
          <tr><td colspan="5" class="none">No value is called that.</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .values { display: grid; gap: 10px; }
  .table { overflow-x: auto; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); }
  table { width: 100%; min-width: 760px; border-collapse: collapse; table-layout: fixed; }
  th { text-align: left; color: var(--quiet); font-size: 12px; font-weight: 600; padding: 9px 12px; border-bottom: 1px solid var(--border); }
  th:nth-child(1) { width: 20%; } th:nth-child(2) { width: 20%; } th:nth-child(3) { width: 24%; } th:nth-child(4) { width: 20%; }
  td { padding: 10px 12px; border-bottom: 1px solid var(--border); vertical-align: top; font-size: 13px; min-width: 0; }
  tr:last-child td { border-bottom: 0; }
  .open { display: grid; gap: 4px; justify-items: start; border: 0; background: none; padding: 0; text-align: left; max-width: 100%; }
  .open:hover .name { text-decoration: underline; }
  code { font: 12.5px var(--mono); padding: 1px 7px; border-radius: 5px; background: color-mix(in srgb, var(--accent) 14%, var(--surface)); border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  .name { font-weight: 600; }
  .note { color: var(--quiet); font-size: 12px; }
  .cut { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cut2 { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .how, .used { color: var(--quiet); }
  .mono { font-family: var(--mono); }
  .empty { display: inline-flex; align-items: center; gap: 6px; color: var(--quiet); }
  .dot { width: 8px; height: 8px; border-radius: 50%; border: 1.5px solid var(--quiet); }
  .none { color: var(--quiet); text-align: center; }
</style>
