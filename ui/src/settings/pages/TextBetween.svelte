<script lang="ts">
  // Reading a value out of a text or log file: the text just before it, and the text just after
  // it. The middle is the value, named as the rule is. Without text after, it is read to the end
  // of the line; with some, it can run over several lines, up to that text.
  import { readBetween, writeBetween } from '../between';

  let { query = $bindable(''), name = '' }: { query: string; name?: string } = $props();

  const start = readBetween(query) ?? { before: '', after: '' };
  let before = $state(start.before);
  let after = $state(start.after);

  $effect(() => {
    query = writeBetween({ before, after });
  });

  // The two texts are told apart by a bar inside the instruction, so a bar cannot be one of them.
  const bar = $derived(before.includes('|') || after.includes('|'));
</script>

<div class="between" data-tour="rules.query">
  <label class="side">
    <span class="label">Text before it</span>
    <input type="text" bind:value={before} placeholder="Version:" aria-label="Text before it" />
  </label>
  <div class="value" aria-hidden="true">
    <span class="label">The value</span>
    <span class="chip">{name.trim() || 'the value you need'}</span>
  </div>
  <label class="side">
    <span class="label">Text after it</span>
    <input type="text" bind:value={after} placeholder="end of the line" aria-label="Text after it" />
  </label>
</div>
{#if bar}
  <p class="error small" role="alert">The bar | cannot be used in these boxes.</p>
{:else if !before.trim()}
  <p class="quiet small">Write the text that comes just before the value, for example “Version:” in a line like “Version: 1.4.2”.</p>
{:else}
  <p class="quiet small">
    Leave “Text after it” empty to read to the end of the line. Write one to stop there: the value can then run
    over several lines. Capitals do not matter.
  </p>
{/if}

<style>
  .between { display: flex; gap: 8px; align-items: end; flex-wrap: wrap; }
  .side { display: grid; gap: 4px; flex: 1 1 160px; min-width: 0; }
  .value { display: grid; gap: 4px; flex: 0 1 auto; max-width: 40%; }
  .label { font-weight: 600; font-size: 13px; }
  .side input { width: 100%; }
  .chip {
    display: block; padding: 5px 10px; border-radius: var(--radius-sm); background: var(--surface-2);
    border: 1px dashed var(--border-strong); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .small { font-size: 12.5px; margin: 0; }
  .error { color: var(--err); }
</style>
