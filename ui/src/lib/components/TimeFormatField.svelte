<script lang="ts">
  // A box to write how a log writes its time ([YYYY-MM-DD HH:mm]), with what a time looks like in
  // it as it is written, or what is wrong with it. Empty: the time is found by itself.
  import { COMMON_FORMATS, FORMAT_HELP, compile, example, isFormat } from '../../capture/blocks/logFormat';

  let {
    value = $bindable(''),
    label,
    id,
    tour,
    onchange,
  }: { value?: string; label: string; id: string; tour?: string; onchange?: (v: string) => void } = $props();

  const parsed = $derived(value.trim() ? compile(value) : null);
  const sample = $derived(parsed && isFormat(parsed) ? example(parsed, Date.now()) : '');
  const problem = $derived(parsed && !isFormat(parsed) ? parsed.problem : '');
</script>

<div class="tf" data-tour={tour}>
  <label for={id}>{label}</label>
  <input {id} type="text" class="mono" bind:value oninput={() => onchange?.(value)} placeholder="Found by itself" list={`${id}-common`} autocomplete="off" spellcheck="false" />
  <datalist id={`${id}-common`}>
    {#each COMMON_FORMATS as f (f)}<option value={f}></option>{/each}
  </datalist>
  {#if problem}
    <p class="error" role="alert">{problem}</p>
  {:else if sample}
    <p class="quiet hint">Lines start like <code class="selectable">{sample}</code></p>
  {:else}
    <p class="quiet hint">Write it the way the log does: {FORMAT_HELP}. Empty, the time is found by itself.</p>
  {/if}
</div>

<style>
  .tf { display: grid; gap: 4px; }
  label { font-weight: 600; font-size: 13px; }
  input { width: 100%; }
  .hint, .error { margin: 0; font-size: 12.5px; }
  .error { color: var(--err); }
</style>
