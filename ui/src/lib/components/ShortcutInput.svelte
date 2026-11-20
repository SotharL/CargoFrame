<script lang="ts">
  import { t } from '../i18n/index.svelte';
  import { Combination, keyLabel, keycaps } from '../shortcut';

  // Any keys held together make the shortcut (Ctrl+A+S, the <> key, the number pad): it is
  // taken when they have all been let go.
  let { value = $bindable(''), label }: { value: string; label: string } = $props();
  let listening = $state(false);
  let live = $state('');
  let unknown = $state('');
  let combo = new Combination();

  function reset() {
    combo = new Combination();
    live = '';
  }

  function onkeydown(e: KeyboardEvent) {
    // Tab and Escape keep their normal job so the keyboard is never trapped here.
    if (e.code === 'Tab' && !live) return;
    if (e.code === 'Escape' && !live) {
      listening = false;
      reset();
      return;
    }
    if ((e.code === 'Backspace' || e.code === 'Delete') && !live && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      value = '';
      reset();
      return;
    }
    e.preventDefault();
    if (e.repeat) return;
    combo.press(e);
    unknown = combo.unknown;
    live = combo.text;
  }

  function onkeyup(e: KeyboardEvent) {
    if (!listening) return;
    if (!live) {
      // a key whose press the system keeps (Print Screen) only comes up: it is taken alone
      const single = new Combination();
      single.press(e);
      if (single.complete) {
        e.preventDefault();
        value = single.text;
        listening = false;
      }
      return;
    }
    e.preventDefault();
    if (!combo.release(e.code)) return;
    if (combo.complete) {
      value = combo.text;
      listening = false;
    }
    reset();
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
    onblur={() => { listening = false; reset(); }}
    {onkeydown}
    {onkeyup}
  >
    {#if shown}
      {#each keycaps(shown) as k, i (i)}<kbd>{keyLabel(k)}</kbd>{/each}
      {#if listening && live.endsWith('+')}<span class="quiet">…</span>{/if}
    {:else}
      <span class="quiet">{listening ? t('Press the keys…') : t('No keys (tray menu only)')}</span>
    {/if}
  </button>
  {#if unknown && listening}<span class="quiet small" role="status">{t('{key} cannot be used in a shortcut', { key: unknown })}</span>{/if}
  {#if value}
    <button type="button" class="clear" onclick={() => (value = '')} aria-label={t('Clear the keys')}>{t('Clear')}</button>
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
  .small { font-size: 12px; }
</style>
