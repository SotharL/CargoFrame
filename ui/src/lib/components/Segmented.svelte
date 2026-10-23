<script lang="ts" generics="T extends string">
  let {
    value = $bindable(),
    options,
    label,
    onchange,
  }: {
    value: T;
    options: { value: T; label: string }[];
    label: string;
    onchange?: (value: T) => void;
  } = $props();

  function pick(v: T) {
    value = v;
    onchange?.(v);
  }
</script>

<div class="segmented" role="radiogroup" aria-label={label}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.value}
      class:on={value === o.value}
      onclick={() => pick(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .segmented { display: inline-flex; border: 1px solid var(--border-strong); border-radius: var(--radius-sm); overflow: hidden; }
  button { border: 0; border-radius: 0; background: var(--surface); padding: 6px 14px; }
  button + button { border-left: 1px solid var(--border-strong); }
  button.on { background: var(--accent); color: var(--on-accent); font-weight: 600; }
</style>
