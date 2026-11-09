<script lang="ts">
  // The editor for a text block's starting text. Instead of typing {{names}} from memory, click
  // what you want in the list on the right: a value to fill in, a value read from a file, a picture
  // shown in the text, a link, or a custom link where a word becomes the link and part of the
  // address is live data. The list has a search box, and the values most texts use come first.
  //
  // When the box is the message of a sharing button, it is the formatted box of the capture window
  // here too, with the same buttons: "- Seen in `{{app.exe}}` **({{project.name}})**" is written
  // and shown formatted, and each capture fills in the values inside the marks.
  import type { ItemEntry, Rule, ServiceKind, VariableInfo } from '../../lib/types';
  import RichBox from '../../capture/blocks/RichBox.svelte';
  import { MARKS, insertBlock, type Mark } from '../../capture/blocks/formatting';
  import { KIND_NAME } from '../../lib/connector';
  import { valuesOf } from '../ruleValues';
  import {
    GROUPS,
    LINK_STYLES,
    MEDIA_SERVICES,
    MEDIA_TABLE,
    QUICK_VARIABLES,
    groupOf,
    insertAt,
    looksLikeAddress,
    makeLink,
    matches,
    quickLabel,
    type LinkStyle,
  } from './automation';

  let {
    value = $bindable(''),
    variables,
    rules,
    marks = [],
    services = [],
  }: {
    value: string;
    variables: VariableInfo[];
    rules: ItemEntry<Rule>[];
    /** The formatting on offer (the box is the message of a button); none: a plain text area. */
    marks?: Mark[];
    services?: ServiceKind[];
  } = $props();

  let area = $state<HTMLTextAreaElement>();
  let rich = $state<RichBox>();
  const buttons = $derived(MARKS.filter((m) => marks.includes(m.mark)));
  const forWhom = $derived(services.map((k) => KIND_NAME[k]).join(', '));

  type Value = { name: string; label: string; about: string; group: string };
  /** Every value on offer: the built-in ones, then each one read from a file. */
  const all = $derived<Value[]>([
    ...variables
      // a picture in the text only where a service the text goes to shows it there
      .filter((v) => !MEDIA_SERVICES[v.name] || services.some((k) => MEDIA_SERVICES[v.name]!.includes(k)))
      .map((v) => ({ name: v.name, label: quickLabel(v.name), about: v.description, group: groupOf(v.name) })),
    ...rules.flatMap((r) =>
      valuesOf(r.id, r.item).map((v) => ({
        name: `rule.${v.key}`,
        label: v.name || v.key,
        about: `Read from ${r.item.file || 'a file'} by “${r.item.name || r.id}”`,
        group: 'From files',
      })),
    ),
  ]);
  const fileValues = $derived(all.filter((v) => v.group === 'From files'));
  const quick = $derived(QUICK_VARIABLES.filter((n) => variables.some((v) => v.name === n)));
  const pictures = $derived(all.filter((v) => v.group === 'In the text'));
  const mediaFor = $derived(
    services.filter((k) => MEDIA_SERVICES['capture.screenshot']!.includes(k)).map((k) => KIND_NAME[k]).join(', '),
  );

  let search = $state('');
  const found = $derived(all.filter((v) => matches(search, v.name, v.label, v.about, v.group)));
  /** The values most texts use first (unless searching), then each group. */
  const groups = $derived.by(() => {
    const out: { name: string; values: Value[] }[] = [];
    if (!search.trim()) {
      const most = quick.map((n) => all.find((v) => v.name === n)).filter((v): v is Value => !!v);
      if (most.length) out.push({ name: 'Most used', values: most });
    }
    for (const g of GROUPS) {
      const values = found.filter((v) => v.group === g);
      if (values.length) out.push({ name: g, values });
    }
    return out;
  });
  const showLinks = $derived(matches(search, 'link web address url custom'));
  const showTable = $derived(pictures.length === 2 && matches(search, 'table repro highlight screenshot recording picture'));

  let linkOpen = $state(false);
  let word = $state('');
  let url = $state('https://');
  let style = $state<LinkStyle>('markdown');
  let urlInput = $state<HTMLInputElement>();
  const preview = $derived(makeLink(style, word || 'the word', url));
  const addressOk = $derived(looksLikeAddress(url));

  function put(snippet: string) {
    if (rich) {
      rich.insert(snippet);
      return;
    }
    const r = insertAt(value, area?.selectionStart ?? value.length, area?.selectionEnd ?? value.length, snippet);
    value = r.text;
    queueMicrotask(() => {
      area?.focus();
      area?.setSelectionRange(r.caret, r.caret);
    });
  }
  const variable = (name: string) => put(`{{${name}}}`);

  /** Lines of their own after the line the cursor is in. */
  function putLines(block: string) {
    if (rich) {
      rich.insertLines(block);
      return;
    }
    const e = insertBlock(value, area?.selectionEnd ?? value.length, block);
    value = e.text;
    queueMicrotask(() => {
      area?.focus();
      area?.setSelectionRange(e.end, e.end);
    });
  }

  function openLink() {
    // A selected word becomes the link's word.
    const sel = rich ? rich.selected() : area ? value.slice(area.selectionStart, area.selectionEnd) : '';
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

<div class="wrap">
<div class="auto">
  <div class="editor">
    {#if buttons.length > 0}
      <div class="tools" role="toolbar" aria-label={`Formatting for ${forWhom}`} title={`Written the way ${forWhom} shows it when sent`}>
        {#each buttons as b (b.mark)}
          <button type="button" class={b.mark} aria-label={b.label} title={b.hint} onmousedown={(e) => e.preventDefault()} onclick={() => rich?.format(b.mark)}>{b.text}</button>
        {/each}
      </div>
      <div class="richwrap"><RichBox bind:this={rich} bind:value id="text-automation" {marks} label="Text automation" /></div>
    {:else}
      <textarea rows="8" bind:value bind:this={area} aria-label="Text automation" spellcheck="true"></textarea>
    {/if}

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
          {#if fileValues.length}
            <select aria-label="A value from a file, in the address" onchange={(e) => { addToAddress(`{{${e.currentTarget.value}}}`); e.currentTarget.value = ''; }}>
              <option value="">A value from a file…</option>
              {#each fileValues as v (v.name)}<option value={v.name}>{v.label}</option>{/each}
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
    <p class="quiet help">
      Filled in for you on every capture; you can still edit it before sharing. Values in
      <code>{'{{curly braces}}'}</code> are replaced, and left visibly empty when CargoFrame cannot find them.
    </p>
  </div>

  <div class="values" role="region" aria-label="Values to put in the text">
    <input type="search" class="search" bind:value={search} placeholder="Search values…" aria-label="Search values" />
    <div class="list">
      {#each groups as g (g.name)}
        <div class="group" role="group" aria-label={g.name}>
          <div class="head">{g.name}</div>
          {#each g.values as v (v.name)}
            <button type="button" class="value" title={`${v.about}\n{{${v.name}}}`} onmousedown={(e) => e.preventDefault()} onclick={() => variable(v.name)}>
              <span class="label">{v.label}</span>
            </button>
          {/each}
          {#if g.name === 'In the text'}
            {#if showTable}
              <button type="button" class="value" title="A table with the recording and the screenshot side by side" onmousedown={(e) => e.preventDefault()} onclick={() => putLines(MEDIA_TABLE)}>
                <span class="label">Repro | Highlight table</span>
              </button>
            {/if}
            <p class="quiet note">Shown in the text by {mediaFor}, instead of as an attachment at the end.</p>
          {/if}
        </div>
      {/each}
      {#if showLinks}
        <div class="group" role="group" aria-label="Links">
          <div class="head">Links</div>
          <button type="button" class="value" onmousedown={(e) => e.preventDefault()} onclick={() => put('https://')}><span class="label">🔗 Link</span></button>
          <button type="button" class="value" aria-expanded={linkOpen} onclick={() => (linkOpen ? (linkOpen = false) : openLink())}><span class="label">🔗 Custom link…</span></button>
        </div>
      {/if}
      {#if groups.length === 0 && !showLinks}
        <p class="quiet note">No value matches “{search}”.</p>
      {/if}
    </div>
  </div>
</div>
</div>

<style>
  /* the text on the left, the values on the right; one above the other when there is no room */
  .wrap { container-type: inline-size; min-width: 0; }
  .auto { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; min-width: 0; }
  @container (min-width: 440px) {
    .auto { grid-template-columns: minmax(0, 1fr) minmax(150px, 34%); align-items: start; }
    .list { max-height: 22em; }
  }
  .editor { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; min-width: 0; align-content: start; }
  .values {
    display: grid; grid-template-rows: auto minmax(0, 1fr); gap: 6px; min-width: 0;
    border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface-2); padding: 6px;
  }
  .search { width: 100%; font-size: 13px; }
  .list { overflow-y: auto; max-height: 16em; display: grid; gap: 8px; align-content: start; }
  .group { display: grid; gap: 2px; }
  .head { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; color: var(--quiet); padding: 0 4px; }
  .value {
    display: block; width: 100%; text-align: left; font-size: 13px; padding: 3px 6px; border: none;
    background: none; border-radius: 4px; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .value:hover, .value:focus-visible { background: color-mix(in srgb, var(--accent) 12%, transparent); }
  .value[aria-expanded='true'] { background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .note { font-size: 12px; padding: 0 4px; }
  textarea {
    width: 100%; background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 6px 8px; font: inherit; resize: vertical;
  }
  .chip { padding: 2px 10px; border-radius: 999px; font-size: 13px; }
  .live select { max-width: 100%; min-width: 0; font-size: 13px; text-overflow: ellipsis; }
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
  .tools { display: flex; flex-wrap: wrap; gap: 4px; }
  .tools button { min-width: 30px; padding: 2px 8px; font-size: 13px; }
  .tools .bold { font-weight: 700; }
  .tools .italic { font-style: italic; }
  .tools .underline { text-decoration: underline; }
  .tools .code { font-family: var(--mono); font-size: 12px; }
  .richwrap { height: 12em; resize: vertical; overflow: hidden; }
  code { font: 12.5px var(--mono); background: var(--surface); padding: 1px 6px; border-radius: 5px; }
</style>
