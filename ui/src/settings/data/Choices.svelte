<script lang="ts" generics="T extends string">
  // One choice of a few, as chips that wrap: "right after “LogAI:”", "at the start of the line".
  let {
    value,
    options,
    label,
    onchange,
  }: {
    value: T;
    options: { value: T; label: string; disabled?: boolean }[];
    label: string;
    onchange: (value: T) => void;
  } = $props();
</script>

<span class="choices" role="radiogroup" aria-label={label}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.value}
      class:on={value === o.value}
      disabled={o.disabled}
      onclick={() => onchange(o.value)}>{o.label}</button>
  {/each}
</span>

<style>
  .choices { display: flex; flex-wrap: wrap; gap: 6px; min-width: 0; }
  button {
    font-size: 13px; padding: 3px 10px; border: 1px solid var(--border-strong); border-radius: var(--radius-sm);
    background: var(--surface); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  button.on { border-color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent); background: color-mix(in srgb, var(--accent) 10%, var(--surface)); }
  button:disabled { border-style: dashed; color: var(--quiet); cursor: not-allowed; }
</style>
