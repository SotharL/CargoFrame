<script lang="ts">
  import { describePress, keycaps } from '../shortcut';

  let { value = $bindable(''), label }: { value: string; label: string } = $props();
  let listening = $state(false);
  let live = $state('');

  function onkeydown(e: KeyboardEvent) {
    // Tab and Escape keep their normal job so the keyboard is never trapped here.
    if (e.code === 'Tab') return;
    if (e.code === 'Escape') {
      listening = false;
      live = '';
      return;
    }
    if ((e.code === 'Backspace' || e.code === 'Delete') && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      value = '';
      live = '';
      return;
    }
    e.preventDefault();
    const p = describePress(e);
    live = p.text;
    if (p.complete) {
      value = p.text;
      listening = false;
      live = '';
    }
  }

  const shown = $derived(listening ? live : value);
</script>

<div class="wrap">
  <button
    type="button"
    class="box"
    class:listening
    aria-label={label}
    onclick={() => { listening = true; live = ''; }}
    onfocus={() => (listening = true)}
    onblur={() => { listening = false; live = ''; }}
    {onkeydown}
  >
    {#if shown}
      {#each keycaps(shown) as k, i (i)}<kbd>{k}</kbd>{/each}
      {#if listening && live.endsWith('+')}<span class="quiet">…</span>{/if}
    {:else}
      <span class="quiet">{listening ? 'Press the keys…' : 'No shortcut (tray menu only)'}</span>
    {/if}
  </button>
  {#if value}
    <button type="button" class="clear" onclick={() => (value = '')} aria-label="Clear shortcut">Clear</button>
  {/if}
</div>

<style>
  .wrap { display: flex; gap: 8px; align-items: center; }
  .box {
    display: inline-flex; gap: 4px; align-items: center; min-height: 34px; min-width: 220px;
    justify-content: flex-start; text-align: left;
  }
  .box.listening { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  kbd {
    font: 600 12px var(--mono); padding: 1px 7px; border-radius: 5px;
    background: var(--surface-2); border: 1px solid var(--border-strong);
    border-bottom-width: 2px;
  }
  .clear { padding: 3px 10px; font-size: 12.5px; }
</style>
