<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import GetLink from '../../lib/components/GetLink.svelte';
  import type { GetLinkSpec } from '../../lib/connectorHelp';

  // Write-only: the app never sends a stored secret back to this window.
  let {
    label,
    hint,
    stored,
    onsave,
    onclear,
    get = null,
  }: {
    label: string;
    hint?: string;
    stored: boolean;
    onsave: (value: string) => Promise<void>;
    onclear: () => Promise<void>;
    /** The page where this is obtained, as a button to the right of the box. */
    get?: GetLinkSpec | null;
  } = $props();

  let value = $state('');
  let replacing = $state(false);
  let error = $state('');
  let busy = $state(false);

  async function run(f: () => Promise<void>) {
    busy = true;
    error = '';
    try {
      await f();
      value = '';
      replacing = false;
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }
</script>

<div class="secret">
  <span class="label">{label}</span>
  {#if stored && !replacing}
    <div class="row">
      <Chip kind="ok" label="Stored in your system's password store" />
      <button type="button" onclick={() => (replacing = true)}>Replace</button>
      <button type="button" disabled={busy} onclick={() => run(onclear)}>Remove</button>
    </div>
  {:else}
    <!-- Not a <form>: this sits inside the connection's own form, and forms must not nest. -->
    <div class="row">
      <input
        type="password"
        bind:value
        autocomplete="off"
        spellcheck="false"
        aria-label={label}
        placeholder="Paste it here"
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            if (value && !busy) void run(() => onsave(value));
          }
        }}
      />
      <button class="primary" type="button" disabled={busy || !value} aria-label={`Save ${label}`} onclick={() => run(() => onsave(value))}>Save</button>
      {#if replacing}<button type="button" onclick={() => (replacing = false)}>Cancel</button>{/if}
      <GetLink link={get} />
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{:else if hint}<p class="quiet hint">{hint}</p>{/if}
</div>

<style>
  .secret { display: grid; gap: 4px; }
  .label { font-weight: 600; font-size: 13px; }
  .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  input[type='password'] {
    flex: 1; min-width: 200px; background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px;
  }
  .hint, .error { font-size: 12.5px; }
  .error { color: var(--err); }
</style>
