<script lang="ts">
  import Field from '../../lib/components/Field.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import LookInEditor from './LookInEditor.svelte';
  import TextAutomation from './TextAutomation.svelte';
  import SharingPanel from './SharingPanel.svelte';
  import VideoOptions from './VideoOptions.svelte';
  import { KIND_NAME, buttonKind, kindStyle } from '../../lib/connector';
  import { marksOf, messageField } from '../../capture/blocks/formatting';
  import { blockName, lineFor } from './reach';
  import { roleHint } from '../../lib/roles';
  import type { Block, ConnectionView, InfoType, IssueView, ItemEntry, Reach, Rect, Rule, VariableInfo } from '../../lib/types';
  import { COLUMNS, typeLabel } from './model';
  import { stepLines } from '../fills';
  import { go } from '../route.svelte';
  import { getApi } from '../../lib/ipc';

  let {
    block,
    blocks,
    rules,
    infoTypes,
    connections = [],
    variables,
    issues,
    reach = {},
    layoutId = null,
    columns = COLUMNS,
    onfills,
    onrect,
    onremove,
  }: {
    block: Block;
    blocks: Block[];
    rules: ItemEntry<Rule>[];
    infoTypes: ItemEntry<InfoType>[];
    /** The connections that exist, with what each can fill and list. */
    connections?: ConnectionView[];
    variables: VariableInfo[];
    issues: IssueView[];
    /** What each sharing button sends, by button id, as worked out by the code that shares. */
    reach?: Record<string, Reach>;
    /** The layout's id, once it has one: the fields filled in it. */
    layoutId?: string | null;
    /** Columns in the layout's grid. */
    columns?: number;
    /** The field fills changed (one was made from this button). */
    onfills?: () => void;
    /** Try to give the block a new position and size; returns false when it does not fit. */
    onrect: (rect: Rect) => boolean;
    onremove: () => void;
  } = $props();

  let rectError = $state('');

  function setRect(patch: Partial<Rect>) {
    const next = { ...block.rect, ...patch };
    rectError = onrect(next) ? '' : 'That does not fit: it would overlap another block or leave the grid.';
  }

  const lines = (v: string) => v.split('\n').map((l) => l.trim()).filter(Boolean);
  const mine = $derived(issues.filter((i) => i.block_id === block.id));
  type TextBlock = Extract<Block, { type: 'text' }>;
  type DropdownBlock = Extract<Block, { type: 'dropdown' }>;
  const textBlocks = $derived(blocks.filter((b): b is TextBlock => b.type === 'text'));
  const promptBlocks = $derived(blocks.filter((b): b is TextBlock => b.type === 'text' && b.role === 'prompt'));
  const valueBlocks = $derived(
    blocks.filter((b): b is TextBlock | DropdownBlock => b.type === 'text' || b.type === 'dropdown'),
  );
  const connectionOf = (id: string | null | undefined) => connections.find((c) => c.id === id);

  type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

  /** The formatting a text block's starting text can have: that of the buttons it is the message of. */
  const textFormat = $derived(
    block.type === 'text' ? marksOf(blocks, (b) => connectionOf(b.connection_id)?.kind, block) : { marks: [], services: [] },
  );

  /** The field fills a share from this button runs. */
  const fillsFor = (b: SharingBlock) =>
    infoTypes.filter(
      (f) => f.item.connection_id === b.connection_id && (f.item.steps ?? []).length > 0 &&
        ((f.item.layouts ?? []).length === 0 || (layoutId !== null && (f.item.layouts ?? []).includes(layoutId))),
    );
  let moveError = $state('');
  /** A fixed value or a value from a file set on the button, as a field fill of this layout. */
  async function moveToFills(b: SharingBlock, m: SharingBlock['mappings'][number]) {
    moveError = '';
    if (!b.connection_id) return;
    const text = m.source.kind === 'rule' ? `{{rule.${m.source.value}}}` : m.source.value;
    const fill: InfoType = {
      name: fieldsFor(b.connection_id).find((f) => f.key === m.field)?.label ?? m.field,
      connection_id: b.connection_id, field: m.field, kind: 'text', options: { type: 'none' }, required: false,
      steps: [{ when: null, then: [{ do: 'set', text }] }],
      layouts: layoutId ? [layoutId] : [],
    };
    try {
      const api = await getApi();
      await api.saveItem('infotype', await api.newItemId('infotype', fill.name), fill);
      b.mappings = b.mappings.filter((x) => x !== m);
      onfills?.();
    } catch (e) {
      moveError = String(e);
    }
  }
  /** The sharing buttons a text can be sent by, with the field of each that takes it, if any. */
  const sendingButtons = $derived(
    blocks
      .filter((b): b is SharingBlock => b.type === 'sharing_button' && !!b.connection_id)
      .map((b) => ({ button: b, kind: connectionOf(b.connection_id)?.kind ?? null, message: messageField(connectionOf(b.connection_id)?.kind) })),
  );
  /** The field of `b` this text goes in ('' for none). */
  const fieldOf = (b: SharingBlock, textId: string) =>
    b.mappings.find((m) => m.source.kind === 'block' && m.source.value === textId)?.field ?? '';
  /** Send this text in `field` of a button (or in none): a field takes one thing, a text goes in one field. */
  function sendIn(b: SharingBlock, textId: string, field: string) {
    const rest = b.mappings.filter((m) => !(m.source.kind === 'block' && m.source.value === textId) && m.field !== field);
    b.mappings = field ? [...rest, { field, source: { kind: 'block', value: textId } }] : rest;
  }
  /** Fields to suggest for a connection: what the service offers, then the person's info types. */
  const fieldsFor = (connection: string | null) => {
    const known = connectionOf(connection)?.describe.fields ?? [];
    const out: { key: string; label: string }[] = [];
    for (const f of known) if (!out.some((o) => o.key === f.key)) out.push({ key: f.key, label: f.label });
    for (const i of infoTypes) {
      if (connection && i.item.connection_id !== connection) continue;
      if (!out.some((o) => o.key === i.item.field)) out.push({ key: i.item.field, label: i.item.name });
    }
    return out;
  };
  const fieldHint = (connection: string | null, key: string) =>
    connectionOf(connection)?.describe.fields.find((f) => f.key === key)?.hint ?? '';

  /** A dropdown's options call is `list` or `list:value` (a list that needs a project key). */
  const splitList = (id: string): [string, string] => {
    const at = id.indexOf(':');
    return at < 0 ? [id, ''] : [id.slice(0, at), id.slice(at + 1)];
  };
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
    <Field label="Role" hint={roleHint(block.role)}>
      <select bind:value={block.role}>
        <option value="plain">Plain text</option>
        <option value="prompt">Prompt</option>
        <option value="zip_name">Zip name</option>
      </select>
    </Field>
    {#if block.role === 'plain'}
      {@const textId = block.id}
      <fieldset class="message-of" data-tour="props.sent_in">
        <legend>Sent in</legend>
        {#each sendingButtons as m (m.button.id)}
          <label class="sent">
            <span>{m.button.label || blockName(m.button)} <span class="quiet">({m.kind ? KIND_NAME[m.kind] : 'service'})</span></span>
            <select
              aria-label={`Field of ${m.button.label || blockName(m.button)}`}
              value={fieldOf(m.button, textId)}
              onchange={(e) => sendIn(m.button, textId, e.currentTarget.value)}
            >
              <option value="">Not sent</option>
              {#each fieldsFor(m.button.connection_id) as f (f.key)}
                <option value={f.key}>{f.label}{f.key === m.message ? ' (formatted)' : ''}</option>
              {/each}
            </select>
          </label>
        {:else}
          <p class="quiet hint">Add a sharing button to the layout to send this box in one of its fields (a Jira summary, a description…).</p>
        {/each}
      </fieldset>
      <div class="toggles">
        <Toggle label="Formatting buttons" checked={block.formatting !== false} onchange={(v) => (block.formatting = v)} />
        <p class="quiet hint">
          Bold, italic, underline, code, quotes, lists and tables above the box, and shown in it, when it is sent in a
          service's message (marked “formatted”): only the ones that service can show, written the way it wants them.
        </p>
      </div>
    {/if}
    <div class="auto-field">
      <span class="auto-title">Text automation</span>
      <TextAutomation bind:value={block.default_text} {variables} {rules} marks={textFormat.marks} services={textFormat.services} />
    </div>
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
      {@const src = block.source}
      {@const conn = connectionOf(src.connection_id)}
      {@const [listId, listParam] = splitList(src.options_id)}
      {@const spec = conn?.describe.lists.find((l) => l.id === listId)}
      <Field label="Connection" hint={connections.length === 0 ? 'Add a connection on the Connections page first.' : ''}>
        <select bind:value={src.connection_id} onchange={() => { src.options_id = ''; block.deeper = null; }}>
          <option value="">Choose…</option>
          {#if src.connection_id && !conn}<option value={src.connection_id}>{src.connection_id} (missing)</option>{/if}
          {#each connections as c (c.id)}<option value={c.id}>{c.name} ({c.kind_label})</option>{/each}
        </select>
      </Field>
      {#if conn}
        <Field label="List" hint={conn.describe.lists.length === 0 ? 'This connection offers no lists.' : ''}>
          <select
            value={listId}
            onchange={(e) => { src.options_id = e.currentTarget.value; block.deeper = null; }}
          >
            <option value="">Choose…</option>
            {#if listId && !spec}<option value={listId}>{listId}</option>{/if}
            {#each conn.describe.lists as l (l.id)}<option value={l.id}>{l.label}</option>{/each}
          </select>
        </Field>
        {#if spec?.param}
          <Field label={spec.param[0]!.toUpperCase() + spec.param.slice(1)} hint="The list is read for this value.">
            <input
              type="text"
              value={listParam}
              oninput={(e) => (src.options_id = `${listId}:${e.currentTarget.value.trim()}`)}
            />
          </Field>
        {/if}
      {/if}
      {#if spec?.inside}
        {@const inside = spec.inside}
        <div class="toggles">
          <Toggle
            label={`Go on to the ${inside.label.toLowerCase()}`}
            checked={!!block.deeper}
            onchange={(v) => (block.deeper = v ? { list: inside.list, field: inside.field } : null)}
          />
          <p class="quiet hint">
            A second box lists what is inside the choice, with “New message or post” first. This is the one place to choose
            where to post: give the block two rows of height, and no other box is needed for it.
          </p>
        </div>
      {/if}
      <div class="toggles">
        <Toggle label="Remember the last choices" checked={block.remember !== false} onchange={(v) => (block.remember = v)} />
        <p class="quiet hint">The last five go to the top of the list, and it starts on the last one used.</p>
        <Toggle label="Allow no choice" checked={block.optional === true} onchange={(v) => (block.optional = v)} />
        <p class="quiet hint">The box can stay empty, or hold something typed or pasted that is not in the list.</p>
      </div>
      {#if textBlocks.length > 0}
        <Field
          label="Narrow the list by"
          hint={block.deeper
            ? 'Only the threads with this text in their name are shown, so one named like the title is easy to find.'
            : 'Only the choices with this text in their name are shown, for example a thread named like the title.'}
        >
          <select
            value={block.narrow_by ?? ''}
            onchange={(e) => (block.narrow_by = e.currentTarget.value || null)}
          >
            <option value="">Nothing: show them all</option>
            {#each textBlocks as t (t.id)}<option value={t.id}>{t.title}</option>{/each}
          </select>
        </Field>
      {/if}
    {/if}
  {:else if block.type === 'video'}
    <VideoOptions {block} />
  {:else if block.type === 'files'}
    <Field label="Title"><input type="text" bind:value={block.title} /></Field>
    <Field label="Show these files" hint="One pattern per line, relative to each folder looked in: *.log, logs/**/*.txt">
      <textarea rows="3" value={block.include.join('\n')} oninput={(e) => (block.include = lines(e.currentTarget.value))}></textarea>
    </Field>
    <Field label="Tick these by default" hint="One pattern per line. Only files listed above can be ticked.">
      <textarea rows="2" value={block.preselect.join('\n')} oninput={(e) => (block.preselect = lines(e.currentTarget.value))}></textarea>
    </Field>
    <LookInEditor {block} />
  {:else if block.type === 'sharing_button'}
    <Field label="Button label"><input type="text" bind:value={block.label} placeholder="Share" /></Field>
    {@const target = connectionOf(block.connection_id)}
    <Field
      label="Share to"
      hint={connections.length === 0
        ? 'Add a connection on the Connections page first.'
        : (target?.describe.summary ?? '')}
    >
      <select
        value={block.connection_id ?? ''}
        onchange={(e) => (block.connection_id = e.currentTarget.value || null)}
      >
        <option value="">Choose where this shares to</option>
        {#if block.connection_id && !target}<option value={block.connection_id}>{block.connection_id} (missing)</option>{/if}
        {#each connections as c (c.id)}<option value={c.id}>{c.name} ({c.kind_label})</option>{/each}
      </select>
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
    <SharingPanel
      {block}
      {blocks}
      kind={buttonKind(block.connection_id, connections)}
      reach={reach[block.id]}
      fallback={target?.capabilities ?? null}
      fields={fieldsFor(block.connection_id).map(({ key, label }) => ({ key, label }))}
    />
    {@const fills = fillsFor(block)}
    {@const own = block.mappings.filter((m) => m.source.kind !== 'block')}
    <div class="maps">
      <h3>Fields filled on each share</h3>
      {#each fills as f (f.id)}
        <p class="small"><strong>{f.item.name}</strong> <span class="quiet">({stepLines(f.item).length} {stepLines(f.item).length === 1 ? 'step' : 'steps'})</span></p>
      {:else}
        <p class="quiet small">None. Send a block in a field above, or fill a field from values found in files.</p>
      {/each}
      {#each own as m (m.field)}
        <div class="map">
          <span class="small"><strong>{m.field}</strong> = <span class="mono">{m.source.kind === 'rule' ? `{{rule.${m.source.value}}}` : m.source.value}</span></span>
          <button type="button" onclick={() => void moveToFills(block, m)}>Move to Fill fields</button>
          <button type="button" aria-label="Remove this field" onclick={() => (block.mappings = block.mappings.filter((x) => x !== m))}>✕</button>
        </div>
      {/each}
      {#if own.length > 0}<p class="quiet small">Set on this button before 0.3.0. Fill fields does the same, with conditions.</p>{/if}
      {#if moveError}<p class="error small" role="alert">{moveError}</p>{/if}
      <button type="button" onclick={() => go('rules', 'fields')}>Open Fill fields</button>
    </div>
  {:else}
    <p class="quiet">The captured picture, with crop, draw, square, circle and text tools. Nothing to set.</p>
  {/if}


  {#if block.type !== 'sharing_button'}
    {@const buttons = blocks.filter((b): b is Extract<Block, { type: 'sharing_button' }> => b.type === 'sharing_button')}
    {#if buttons.length > 0}
      <section class="sentby" aria-label="Sent by">
        <h3>Sent by</h3>
        <ul>
          {#each buttons as btn (btn.id)}
            {@const r = reach[btn.id]?.blocks.find((x) => x.block_id === block.id)}
            {@const k = buttonKind(btn.connection_id, connections)}
            {#if r}
              {@const l = lineFor(r, btn, k)}
              <li class={l.state} style={kindStyle(k)}>
                <span class="who">{blockName(btn)}</span>
                <span class="icon" aria-hidden="true">{l.icon}</span> {l.text}
              </li>
            {/if}
          {/each}
        </ul>
      </section>
    {/if}
  {/if}

  <fieldset class="rect">
    <legend>Position and size</legend>
    <label>Column<input type="number" min="1" max={columns} value={block.rect.x + 1} onchange={(e) => setRect({ x: Number(e.currentTarget.value) - 1 })} /></label>
    <label>Row<input type="number" min="1" value={block.rect.y + 1} onchange={(e) => setRect({ y: Number(e.currentTarget.value) - 1 })} /></label>
    <label>Width<input type="number" min="1" max={columns} value={block.rect.w} onchange={(e) => setRect({ w: Number(e.currentTarget.value) })} /></label>
    <label>Height<input type="number" min="1" value={block.rect.h} onchange={(e) => setRect({ h: Number(e.currentTarget.value) })} /></label>
  </fieldset>
  {#if rectError}<p class="error" role="alert">{rectError}</p>{/if}

  <button type="button" class="remove" onclick={onremove}>Remove this block</button>
</div>

<style>
  .props { display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; align-content: start; }
  .head { display: flex; gap: 10px; align-items: baseline; }
  .id { font-size: 12px; }
  .issue { font-size: 12.5px; padding: 6px 8px; border-radius: var(--radius-sm); border: 1px solid var(--quiet); }
  .issue.error { border-color: var(--err); background: color-mix(in srgb, var(--err) 12%, transparent); }
  .issue.warning { border-color: var(--warn); background: color-mix(in srgb, var(--warn) 12%, transparent); }
  .issue.debug { border-color: var(--dbg); background: color-mix(in srgb, var(--dbg) 12%, transparent); }
  .toggles { display: grid; gap: 4px; }
  .message-of { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 6px 10px 8px; margin: 0; display: grid; gap: 4px; }
  .message-of legend { font-weight: 600; font-size: 12.5px; padding: 0 4px; }
  .message-of .sent { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 6px; align-items: center; font-size: 13px; }
  .message-of .hint { margin: 0; font-size: 12px; }
  .toggles .hint { margin: 0 0 6px 46px; font-size: 12px; }
  .auto-field { display: grid; gap: 4px; }
  .auto-title { font-weight: 600; font-size: 13px; }
  .maps { display: grid; gap: 8px; }
  .maps h3 { font-size: 13px; }
  .map { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; padding: 8px; background: var(--surface-2); border-radius: var(--radius-sm); }
  .map > span { flex: 1 1 100%; min-width: 0; overflow-wrap: anywhere; }
  .map button { padding: 2px 8px; }
  .maps > button { justify-self: start; }
  .small { font-size: 12.5px; }
  .sentby h3 { font-size: 13px; margin: 0 0 4px; }
  .sentby ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  .sentby li { font-size: 12.5px; }
  .sentby li.cannot_send { color: var(--text); }
  .sentby li.not_used, .sentby li.switched_off { color: var(--quiet); }
  .who {
    display: inline-block; padding: 0 7px; margin-right: 4px; border-radius: 999px;
    background: var(--kind); color: var(--on-kind); border: 1px solid var(--kind-line); font-size: 11.5px;
  }
  .icon { font-weight: 700; }
  .hintline { grid-column: 1 / -1; }
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
