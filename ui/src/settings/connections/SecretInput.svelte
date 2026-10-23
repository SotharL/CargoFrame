<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';

  // Write-only: the app never sends a stored secret back to this window.
  let {
    label,
    hint,
    stored,
    onsave,
    onclear,
  }: {
    label: string;
    hint?: string;
    stored: boolean;
    onsave: (value: string) => Promise<void>;
    onclear: () => Promise<void>;
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
    <form
      class="row"
      onsubmit={(e) => {
        e.preventDefault();
        void run(() => onsave(value));
      }}
    >
      <input
        type="password"
        bind:value
        autocomplete="off"
        spellcheck="false"
        aria-label={label}
        placeholder="Paste it here"
      />
      <button class="primary" type="submit" disabled={busy || !value}>Save</button>
      {#if replacing}<button type="button" onclick={() => (replacing = false)}>Cancel</button>{/if}
    </form>
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
