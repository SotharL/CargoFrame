<script lang="ts">
  // The editor for a text block's starting text. Instead of typing {{names}} from memory, click
  // what you want: a value to fill in, a value read from a file, a link, or a custom link where a
  // word becomes the link and part of the address is live data.
  import type { ItemEntry, Rule, VariableInfo } from '../../lib/types';
  import { LINK_STYLES, QUICK_VARIABLES, insertAt, looksLikeAddress, makeLink, quickLabel, type LinkStyle } from './automation';

  let {
    value = $bindable(''),
    variables,
    rules,
  }: { value: string; variables: VariableInfo[]; rules: ItemEntry<Rule>[] } = $props();

  let area = $state<HTMLTextAreaElement>();
  let linkOpen = $state(false);
  let word = $state('');
  let url = $state('https://');
  let style = $state<LinkStyle>('markdown');
  let urlInput = $state<HTMLInputElement>();

  const quick = $derived(QUICK_VARIABLES.filter((n) => variables.some((v) => v.name === n)));
  const others = $derived(variables.filter((v) => !QUICK_VARIABLES.includes(v.name)));
  const preview = $derived(makeLink(style, word || 'the word', url));
  const addressOk = $derived(looksLikeAddress(url));

  function put(snippet: string) {
    const r = insertAt(value, area?.selectionStart ?? value.length, area?.selectionEnd ?? value.length, snippet);
    value = r.text;
    queueMicrotask(() => {
      area?.focus();
      area?.setSelectionRange(r.caret, r.caret);
    });
  }
  const variable = (name: string) => put(`{{${name}}}`);

  function openLink() {
    // A selected word becomes the link's word.
    const sel = area ? value.slice(area.selectionStart, area.selectionEnd) : '';
    if (sel && !sel.includes('\n')) word = sel;
    linkOpen = true;
  }

  function addToAddress(token: string) {
    const el = urlInput;
    const r = insertAt(url, el?.selectionStart ?? url.length, el?.selectionEnd ?? url.length, token);
    url = r.text;
    queueMicrotask(() => {
      el?.focus();
      el?.setSelectionRange(r.caret, r.caret);
    });
  }

  function insertLink() {
    put(makeLink(style, word, url));
    linkOpen = false;
    word = '';
    url = 'https://';
  }
</script>

<div class="auto">
  <div class="bar" role="toolbar" aria-label="Insert into the text">
    {#each quick as n (n)}
      <button type="button" class="chip" title="{n}: fills in on every capture" onclick={() => variable(n)}>＋ {quickLabel(n)}</button>
    {/each}
    {#if others.length}
      <select aria-label="More values" onchange={(e) => { variable(e.currentTarget.value); e.currentTarget.value = ''; }}>
        <option value="">More values…</option>
        {#each others as v (v.name)}<option value={v.name}>{v.name} – {v.description}</option>{/each}
      </select>
    {/if}
    {#if rules.length}
      <select aria-label="A value from a file" onchange={(e) => { variable(`rule.${e.currentTarget.value}`); e.currentTarget.value = ''; }}>
        <option value="">A value from a file…</option>
        {#each rules as r (r.id)}<option value={r.id}>{r.item.name || r.id}</option>{/each}
      </select>
    {/if}
    <button type="button" class="chip" onclick={() => put('https://')}>🔗 Link</button>
    <button type="button" class="chip" aria-expanded={linkOpen} onclick={() => (linkOpen ? (linkOpen = false) : openLink())}>🔗 Custom link…</button>
  </div>

  {#if linkOpen}
    <div class="link" role="group" aria-label="Custom link">
      <label>
        <span>Word that becomes the link</span>
        <input type="text" bind:value={word} placeholder="the build" />
      </label>
      <label>
        <span>Web address</span>
        <input type="text" bind:value={url} bind:this={urlInput} placeholder="https://ci.example.com/builds/" />
      </label>
      <div class="live">
        <span class="quiet">Put live data in the address:</span>
        {#each quick as n (n)}<button type="button" class="chip" onclick={() => addToAddress(`{{${n}}}`)}>{quickLabel(n)}</button>{/each}
        {#if rules.length}
          <select aria-label="A value from a file, in the address" onchange={(e) => { addToAddress(`{{rule.${e.currentTarget.value}}}`); e.currentTarget.value = ''; }}>
            <option value="">A value from a file…</option>
            {#each rules as r (r.id)}<option value={r.id}>{r.item.name || r.id}</option>{/each}
          </select>
        {/if}
      </div>
      <label>
        <span>Link style</span>
        <select bind:value={style}>
          {#each LINK_STYLES as s (s.value)}<option value={s.value}>{s.label} – {s.example}</option>{/each}
        </select>
      </label>
      <p class="preview">Result: <code class="selectable">{preview}</code></p>
      {#if !addressOk}<p class="warn" role="note">The address should start with https:// or with a value in curly braces.</p>{/if}
      <div class="buttons">
        <button type="button" class="primary" onclick={insertLink} disabled={!url.trim() || url.trim() === 'https://'}>Insert link</button>
        <button type="button" onclick={() => (linkOpen = false)}>Cancel</button>
      </div>
    </div>
  {/if}

  <textarea rows="6" bind:value bind:this={area} aria-label="Text automation" spellcheck="true"></textarea>
  <p class="quiet help">
    Filled in for you on every capture; you can still edit it before sharing. Values in
    <code>{'{{curly braces}}'}</code> are replaced, and left visibly empty when CargoFrame cannot find them.
  </p>
</div>

<style>
  .auto { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; min-width: 0; }
  .bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .chip { padding: 2px 10px; border-radius: 999px; font-size: 13px; }
  .bar select, .live select { max-width: 100%; min-width: 0; font-size: 13px; text-overflow: ellipsis; }
  textarea {
    width: 100%; background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; font: inherit; resize: vertical;
  }
  .link {
    display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; padding: 10px; border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); background: var(--surface-2);
  }
  .link label { display: grid; gap: 3px; }
  .link label span { font-size: 12.5px; font-weight: 600; }
  .link input[type='text'], .link select { width: 100%; }
  .live { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .preview { font-size: 13px; overflow-wrap: anywhere; }
  .warn { font-size: 12.5px; color: var(--warn); }
  .buttons { display: flex; gap: 8px; }
  .help { font-size: 12.5px; }
  code { font: 12.5px var(--mono); background: var(--surface); padding: 1px 6px; border-radius: 5px; }
</style>
