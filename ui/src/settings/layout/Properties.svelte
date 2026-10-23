<script lang="ts">
  import Field from '../../lib/components/Field.svelte';
  import type { Block, InfoType, IssueView, ItemEntry, Rect, Rule, VariableInfo } from '../../lib/types';
  import { COLUMNS, typeLabel } from './model';

  let {
    block,
    blocks,
    rules,
    infoTypes,
    variables,
    issues,
    onrect,
    onremove,
  }: {
    block: Block;
    blocks: Block[];
    rules: ItemEntry<Rule>[];
    infoTypes: ItemEntry<InfoType>[];
    variables: VariableInfo[];
    issues: IssueView[];
    /** Try to give the block a new position and size; returns false when it does not fit. */
    onrect: (rect: Rect) => boolean;
    onremove: () => void;
  } = $props();

  let rectError = $state('');
  let textarea = $state<HTMLTextAreaElement>();

  function setRect(patch: Partial<Rect>) {
    const next = { ...block.rect, ...patch };
    rectError = onrect(next) ? '' : 'That does not fit: it would overlap another block or leave the grid.';
  }

  const lines = (v: string) => v.split('\n').map((l) => l.trim()).filter(Boolean);
  const mine = $derived(issues.filter((i) => i.block_id === block.id));
  type TextBlock = Extract<Block, { type: 'text' }>;
  type DropdownBlock = Extract<Block, { type: 'dropdown' }>;
  const promptBlocks = $derived(blocks.filter((b): b is TextBlock => b.type === 'text' && b.role === 'prompt'));
  const valueBlocks = $derived(
    blocks.filter((b): b is TextBlock | DropdownBlock => b.type === 'text' || b.type === 'dropdown'),
  );
  const fieldsFor = (connection: string | null) =>
    infoTypes.filter((i) => !connection || i.item.connection_id === connection).map((i) => i.item.field);

  function insertVariable(name: string) {
    if (block.type !== 'text' || !name) return;
    const token = `{{${name}}}`;
    const el = textarea;
    const at = el?.selectionStart ?? block.default_text.length;
    const end = el?.selectionEnd ?? at;
    block.default_text = block.default_text.slice(0, at) + token + block.default_text.slice(end);
    queueMicrotask(() => {
      el?.focus();
      el?.setSelectionRange(at + token.length, at + token.length);
    });
  }
</script>

<div class="props">
  <div class="head">
    <h2>{typeLabel(block.type)}</h2>
    <span class="id mono quiet">{block.id}</span>
  </div>

  {#each mine as i, n (n)}
    <p class="issue {i.severity}" role="note"><span aria-hidden="true">{i.severity === 'error' ? '✕' : i.severity === 'warning' ? '!' : '?'}</span> {i.message}</p>
  {/each}

  {#if block.type === 'text'}
    <Field label="Title"><input type="text" bind:value={block.title} /></Field>
    <Field label="Title position">
      <select bind:value={block.placement}>
        <option value="above">Above the box</option>
        <option value="inside">Inside the box</option>
      </select>
    </Field>
    <Field label="Role" hint={block.role === 'prompt' ? 'The text sent to an AI chat. A sharing button to Claude needs one.' : 'Plain notes that are sent along with the capture.'}>
      <select bind:value={block.role}>
        <option value="plain">Plain text</option>
        <option value="prompt">Prompt</option>
      </select>
    </Field>
    <Field label="Starting text" hint="Filled in for you each time; edit it before sharing.">
      <textarea rows="5" bind:value={block.default_text} bind:this={textarea}></textarea>
    </Field>
    <label class="insert">
      <span class="quiet">Insert a variable</span>
      <select onchange={(e) => { insertVariable(e.currentTarget.value); e.currentTarget.value = ''; }}>
        <option value="">Choose…</option>
        {#each variables as v (v.name)}<option value={v.name}>{v.name} – {v.description}</option>{/each}
      </select>
    </label>
  {:else if block.type === 'dropdown'}
    <Field label="Title"><input type="text" bind:value={block.title} /></Field>
    <Field label="Choices come from">
      <select
        value={block.source.type}
        onchange={(e) => {
          block.source = e.currentTarget.value === 'claude_chats'
            ? { type: 'claude_chats' }
            : { type: 'web_api', connection_id: '', options_id: '' };
        }}
      >
        <option value="claude_chats">Your pinned and recent Claude chats</option>
        <option value="web_api">A service's API</option>
      </select>
    </Field>
    {#if block.source.type === 'web_api'}
      <Field label="Connection" hint="The id of the connection to ask (for example jira).">
        <input type="text" bind:value={block.source.connection_id} list="connection-ids" />
      </Field>
      <Field label="Options call" hint="The name of the options call defined on that connection.">
        <input type="text" bind:value={block.source.options_id} placeholder="projects" />
      </Field>
    {/if}
  {:else if block.type === 'files'}
    <Field label="Title"><input type="text" bind:value={block.title} /></Field>
    <Field label="Show these files" hint="One pattern per line, relative to the project folder: *.log, logs/**/*.txt">
      <textarea rows="3" value={block.include.join('\n')} oninput={(e) => (block.include = lines(e.currentTarget.value))}></textarea>
    </Field>
    <Field label="Tick these by default" hint="One pattern per line. Only files listed above can be ticked.">
      <textarea rows="2" value={block.preselect.join('\n')} oninput={(e) => (block.preselect = lines(e.currentTarget.value))}></textarea>
    </Field>
  {:else if block.type === 'sharing_button'}
    <Field label="Button label"><input type="text" bind:value={block.label} placeholder="Share" /></Field>
    <Field label="Share to" hint="The id of a connection (for example jira or claude).">
      <input
        type="text"
        value={block.connection_id ?? ''}
        list="connection-ids"
        oninput={(e) => (block.connection_id = e.currentTarget.value.trim() || null)}
        placeholder="Choose where this shares to"
      />
    </Field>
    <Field label="Prompt" hint={promptBlocks.length ? 'Required for AI chats: the button stays off until it has text.' : 'Add a Text block and set its role to Prompt to use it here.'}>
      <select
        value={block.prompt_block_id ?? ''}
        onchange={(e) => (block.prompt_block_id = e.currentTarget.value || null)}
      >
        <option value="">None</option>
        {#each promptBlocks as p (p.id)}<option value={p.id}>{p.title}</option>{/each}
      </select>
    </Field>
    <Field label="After sharing">
      <select bind:value={block.on_success}>
        <option value="close_window">Close the window</option>
        <option value="keep_open">Keep it open</option>
      </select>
    </Field>
    <div class="maps">
      <h3>Fill the service's fields</h3>
      {#each block.mappings as m, i (i)}
        <div class="map">
          <input type="text" bind:value={m.field} list="fields-{block.id}" placeholder="Field" aria-label="Field" />
          <select
            value={m.source.kind}
            aria-label="Value comes from"
            onchange={(e) => (m.source = { kind: e.currentTarget.value as 'block' | 'rule' | 'fixed', value: '' })}
          >
            <option value="block">A block</option>
            <option value="rule">A data rule</option>
            <option value="fixed">A fixed value</option>
          </select>
          {#if m.source.kind === 'block'}
            <select bind:value={m.source.value} aria-label="Block">
              <option value="">Choose…</option>
              {#each valueBlocks as v (v.id)}<option value={v.id}>{v.title}</option>{/each}
            </select>
          {:else if m.source.kind === 'rule'}
            <select bind:value={m.source.value} aria-label="Data rule">
              <option value="">Choose…</option>
              {#each rules as r (r.id)}<option value={r.id}>{r.item.name}</option>{/each}
            </select>
          {:else}
            <input type="text" bind:value={m.source.value} aria-label="Fixed value" placeholder="Value" />
          {/if}
          <button type="button" aria-label="Remove this field" onclick={() => block.mappings.splice(i, 1)}>✕</button>
        </div>
      {:else}
        <p class="quiet small">None yet. The screenshot, prompt and ticked files are always sent.</p>
      {/each}
      <datalist id="fields-{block.id}">
        {#each fieldsFor(block.connection_id) as f (f)}<option value={f}></option>{/each}
      </datalist>
      <button type="button" onclick={() => block.mappings.push({ field: '', source: { kind: 'block', value: '' } })}>Add a field</button>
    </div>
  {:else}
    <p class="quiet">The captured picture, with crop, draw, square, circle and text tools. Nothing to set.</p>
  {/if}

  <datalist id="connection-ids">
    {#each [...new Set(infoTypes.map((i) => i.item.connection_id).filter(Boolean))] as c (c)}<option value={c}></option>{/each}
  </datalist>

  <fieldset class="rect">
    <legend>Position and size</legend>
    <label>Column<input type="number" min="1" max={COLUMNS} value={block.rect.x + 1} onchange={(e) => setRect({ x: Number(e.currentTarget.value) - 1 })} /></label>
    <label>Row<input type="number" min="1" value={block.rect.y + 1} onchange={(e) => setRect({ y: Number(e.currentTarget.value) - 1 })} /></label>
    <label>Width<input type="number" min="1" max={COLUMNS} value={block.rect.w} onchange={(e) => setRect({ w: Number(e.currentTarget.value) })} /></label>
    <label>Height<input type="number" min="1" value={block.rect.h} onchange={(e) => setRect({ h: Number(e.currentTarget.value) })} /></label>
  </fieldset>
  {#if rectError}<p class="error" role="alert">{rectError}</p>{/if}

  <button type="button" class="remove" onclick={onremove}>Remove this block</button>
</div>

<style>
  .props { display: grid; gap: 12px; align-content: start; }
  .head { display: flex; gap: 10px; align-items: baseline; }
  .id { font-size: 12px; }
  .issue { font-size: 12.5px; padding: 6px 8px; border-radius: var(--radius-sm); border: 1px solid var(--quiet); }
  .issue.error { border-color: var(--err); background: color-mix(in srgb, var(--err) 12%, transparent); }
  .issue.warning { border-color: var(--warn); background: color-mix(in srgb, var(--warn) 12%, transparent); }
  .issue.debug { border-color: var(--dbg); background: color-mix(in srgb, var(--dbg) 12%, transparent); }
  .insert { display: grid; gap: 4px; font-size: 13px; }
  .maps { display: grid; gap: 8px; }
  .maps h3 { font-size: 13px; }
  .map { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; padding: 8px; background: var(--surface-2); border-radius: var(--radius-sm); }
  .map button { grid-column: 1 / -1; justify-self: end; padding: 2px 8px; }
  .small { font-size: 12.5px; }
  .rect { border: 1px solid var(--border); border-radius: var(--radius-sm); display: grid; grid-template-columns: 1fr 1fr; gap: 8px; padding: 8px 10px 10px; }
  .rect legend { font-size: 12px; font-weight: 600; padding: 0 4px; }
  .rect label { display: grid; gap: 2px; font-size: 12.5px; }
  .rect input {
    width: 100%; background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm); padding: 4px 6px;
  }
  .error { color: var(--err); font-size: 12.5px; }
  .remove { border-color: var(--err); background: color-mix(in srgb, var(--err) 10%, transparent); justify-self: start; }
</style>
