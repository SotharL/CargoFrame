<script lang="ts">
  // Status chip: colour + icon + label, so colour is never the only signal.
  type Kind = 'ok' | 'warn' | 'err' | 'dbg';
  let { kind, label }: { kind: Kind; label: string } = $props();
  const icons: Record<Kind, string> = { ok: '✓', warn: '!', err: '✕', dbg: '?' };
</script>

<span class="chip {kind}">
  <span class="icon" aria-hidden="true">{icons[kind]}</span>
  {label}
</span>

<style>
  .chip {
    --c: var(--quiet);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 8px;
    border: 1px solid var(--c);
    border-radius: 999px;
    background: color-mix(in srgb, var(--c) 14%, transparent);
    font-size: 12px;
    line-height: 18px;
    white-space: nowrap;
  }
  .icon {
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--c);
    color: #1f1e1d;
    font-size: 10px;
    font-weight: 700;
    line-height: 1;
  }
  .ok { --c: var(--ok); }
  .warn { --c: var(--warn); }
  .err { --c: var(--err); }
  .dbg { --c: var(--dbg); }
</style>
