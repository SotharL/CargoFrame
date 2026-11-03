<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  import Logo from '../../lib/components/Logo.svelte';
  import { ACCENT_PRESETS, DEFAULT_ACCENT, accentIsVisible, normalizeAccent } from '../../lib/theme';

  // The accent colour: buttons, focus rings, links, the selected block and the arrow of the logo.
  // `onpreview` is called while the colour is being picked, `onsave` once a colour is chosen.
  let {
    value,
    onpreview,
    onsave,
  }: { value: string; onpreview: (hex: string) => void; onsave: (hex: string) => void } = $props();

  const shown = $derived(normalizeAccent(value) ?? DEFAULT_ACCENT);
  let text = $state('');
  let typing = $state(false);
  const field = $derived(typing ? text : shown);
  const typed = $derived(normalizeAccent(field));
  const visible = $derived(accentIsVisible(typed ?? shown));

  function choose(hex: string) {
    typing = false;
    onpreview(hex);
    onsave(hex);
  }

  function onText(e: Event) {
    typing = true;
    text = (e.currentTarget as HTMLInputElement).value;
    const hex = normalizeAccent(text);
    if (hex) onpreview(hex);
  }

  function commitText() {
    const hex = normalizeAccent(text);
    if (typing && hex) choose(hex);
    typing = false;
  }
</script>

<div class="accent">
  <div class="row">
    <Logo size={48} label="The logo in your accent colour" />
    <input
      class="picker"
      type="color"
      aria-label="Accent colour"
      value={shown}
      oninput={(e) => onpreview((e.currentTarget as HTMLInputElement).value)}
      onchange={(e) => choose((e.currentTarget as HTMLInputElement).value)}
    />
    <input
      class="hex mono"
      type="text"
      aria-label="Accent colour as a code, like #4f8ee0"
      spellcheck="false"
      autocomplete="off"
      maxlength="7"
      value={field}
      oninput={onText}
      onchange={commitText}
      onkeydown={(e) => e.key === 'Enter' && commitText()}
    />
    <button type="button" onclick={() => choose(DEFAULT_ACCENT)} disabled={shown === DEFAULT_ACCENT}>
      Reset to blue
    </button>
  </div>

  <div class="swatches" role="group" aria-label="Suggested accent colours">
    {#each ACCENT_PRESETS as p (p.hex)}
      <button
        type="button"
        class="swatch"
        style:--c={p.hex}
        title={p.name}
        aria-label={p.name}
        aria-pressed={shown === p.hex}
        onclick={() => choose(p.hex)}
      ></button>
    {/each}
  </div>

  {#if typing && !typed}
    <p class="error" role="alert">Type a colour like #4f8ee0.</p>
  {:else if !visible.light || !visible.dark}
    <Chip
      kind="warn"
      label={!visible.light && !visible.dark
        ? 'Hard to see on both the light and the dark theme'
        : !visible.light
          ? 'Hard to see on the light theme'
          : 'Hard to see on the dark theme'}
    />
  {/if}
  <p class="quiet hint">
    Used for buttons, focus rings, links, the selected block and the arrow of the logo.
  </p>
</div>

<style>
  .accent { display: grid; gap: 10px; }
  .row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .picker {
    width: 44px; height: 32px; padding: 2px; cursor: pointer;
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--radius-sm);
  }
  .hex { width: 96px; }
  .swatches { display: flex; gap: 8px; }
  .swatch {
    width: 28px; height: 28px; padding: 0; border-radius: 50%;
    background: var(--c); border: 2px solid var(--surface); box-shadow: 0 0 0 1px var(--border-strong);
  }
  .swatch[aria-pressed='true'] { box-shadow: 0 0 0 2px var(--text); }
  .hint, .error { font-size: 12.5px; margin: 0; }
  .error { color: var(--err); }
</style>
