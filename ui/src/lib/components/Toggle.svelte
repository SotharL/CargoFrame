<script lang="ts">
  let {
    checked = $bindable(false),
    label,
    onchange,
  }: { checked?: boolean; label: string; onchange?: (value: boolean) => void } = $props();
</script>

<label class="toggle">
  <input
    type="checkbox"
    role="switch"
    bind:checked
    onchange={() => onchange?.(checked)}
  />
  <span class="track" aria-hidden="true"><span class="thumb"></span></span>
  <span>{label}</span>
</label>

<style>
  /* relative: the hidden input below is absolute, and without a positioned parent it escapes the
     scrolling page and gives the whole window a second scroll bar */
  .toggle { position: relative; display: inline-flex; align-items: center; gap: 10px; cursor: pointer; }
  input { position: absolute; opacity: 0; width: 1px; height: 1px; }
  .track {
    position: relative; width: 36px; height: 20px; flex: none;
    border-radius: 999px; background: var(--border-strong); border: 1px solid var(--border-strong);
  }
  .thumb {
    position: absolute; top: 2px; left: 2px; width: 14px; height: 14px;
    border-radius: 50%; background: var(--surface);
  }
  input:checked + .track { background: var(--accent); border-color: var(--accent); }
  input:checked + .track .thumb { left: 18px; background: var(--on-accent); }
  input:focus-visible + .track { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
