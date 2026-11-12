<script lang="ts">
  // The part of a sharing button's settings that says what it can send and what it does send:
  // the destination's capability table, one line per block, and the rules for the picture and
  // the files. The answers come from the same Rust code that does the sharing.
  import Field from '../../lib/components/Field.svelte';
  import { KIND_NAME, kindKey, kindStyle } from '../../lib/connector';
  import type { Block, Capabilities, Reach, ServiceKind } from '../../lib/types';
  import { DEFAULT_SEND_RULES, MAX_INLINE_KB } from '../../lib/types';
  import { blockName, CAPABILITY_ROWS, lineFor, SUPPORT_ICON, SUPPORT_WORD } from './reach';
  import { applyRoute, currentRoute, isAutomatic, routeOptions, type RouteField } from './routes';

  type SharingBlock = Extract<Block, { type: 'sharing_button' }>;
  let {
    block,
    blocks,
    kind,
    reach,
    fallback = null,
    fields = [],
  }: {
    block: SharingBlock;
    blocks: Block[];
    /** The destination the button really sends to; `null`: it saves a zip on this computer. */
    kind: ServiceKind | null;
    reach: Reach | undefined;
    /** What to show before the backend has answered. */
    fallback?: Capabilities | null;
    /** The fields the destination offers, to choose from when a block is sent in one. */
    fields?: RouteField[];
  } = $props();

  const caps = $derived(reach?.capabilities ?? fallback);
  const rules = $derived({ ...DEFAULT_SEND_RULES, ...block.send });
  const others = $derived(blocks.filter((b) => b.type !== 'sharing_button'));
  const verdicts = $derived(
    others.map((b) => ({ b, line: reach ? (reach.blocks.find((r) => r.block_id === b.id) ?? null) : null })),
  );
  const heading = $derived(kind ? `${KIND_NAME[kindKey(kind)]} can take` : 'Saving a zip on this computer keeps');

  /** Send `blockId` as `route` (not sent, the prompt, or a field) by changing the button itself. */
  function route(blockId: string, to: string) {
    const next = applyRoute(block, blockId, to);
    block.prompt_block_id = next.prompt_block_id;
    block.mappings = next.mappings;
  }

  function setRules(patch: Partial<typeof rules>) {
    block.send = { ...rules, ...patch };
  }
  // a layout with a video block sends the recording where a screenshot layout sends the picture
  const isVideo = $derived(blocks.some((b) => b.type === 'video'));
  const pictureCap = $derived(isVideo ? caps?.video : caps?.screenshot);
  const pictureOff = $derived(pictureCap?.support === 'no');
  const filesOff = $derived(caps?.files.support === 'no');
  const limit = $derived(Math.min(MAX_INLINE_KB, Math.max(1, rules.inline_limit_kb)));
</script>

<section class="panel" style={kindStyle(kind)} aria-label="What this button sends">
  {#if caps}
    <h3>{heading}</h3>
    <ul class="caps">
      {#each CAPABILITY_ROWS as row (row.key)}
        {@const c = caps[row.key]}
        <li class="cap {c.support}">
          <span class="icon" aria-hidden="true">{SUPPORT_ICON[c.support]}</span>
          <span class="what"><strong>{row.label}</strong> <span class="word">{SUPPORT_WORD[c.support]}</span></span>
          <span class="quiet note">{c.note}</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if reach && verdicts.length > 0}
    <h3>What this button does with each block</h3>
    <ul class="sends">
      {#each verdicts as v (v.b.id)}
        {#if v.line}
          {@const l = lineFor(v.line, block, kind)}
          {@const choosable = kind !== null && (v.b.type === 'text' || v.b.type === 'dropdown' || v.b.type === 'tags')}
          <li class="send {l.state}">
            <span class="icon" aria-hidden="true">{l.icon}</span>
            {#if choosable}
              {@const current = currentRoute(v.line)}
              {@const automatic = isAutomatic(block, kind, v.line)}
              <span class="choose">
                <label>
                  <strong>{blockName(v.b)}</strong>
                  <select
                    aria-label="{blockName(v.b)}: what this button does with it"
                    value={current}
                    disabled={v.line.state === 'cannot_send' || automatic}
                    onchange={(e) => route(v.b.id, e.currentTarget.value)}
                  >
                    {#each routeOptions(block, blocks, v.b, fields, current) as o (o.value)}
                      <option value={o.value} disabled={o.disabled}>{o.label}</option>
                    {/each}
                  </select>
                </label>
                {#if v.line.state === 'cannot_send'}
                  <span class="quiet note">{l.text}</span>
                {:else if automatic}
                  <span class="quiet note">Always picks the chat to open</span>
                {/if}
              </span>
            {:else}
              <span><strong>{blockName(v.b)}</strong>: {l.text}</span>
            {/if}
          </li>
        {/if}
      {/each}
    </ul>
  {/if}

  <h3>Send rules</h3>
  <Field
    label={isVideo ? 'Video' : 'Screenshot'}
    hint={pictureOff ? pictureCap?.note : isVideo ? 'Turn off to leave the video out of what this button sends.' : 'Turn off to leave the picture out of what this button sends.'}
  >
    <span class="check">
      <input
        type="checkbox"
        aria-label={isVideo ? 'Send the video' : 'Send the screenshot'}
        checked={rules.screenshot && !pictureOff}
        disabled={pictureOff}
        onchange={(e) => setRules({ screenshot: e.currentTarget.checked })}
      />
      {isVideo ? 'Send the video' : 'Send the screenshot'}
    </span>
  </Field>
  <Field
    label="Ticked files"
    hint={filesOff
      ? caps?.files.note
      : kind === 'claude'
        ? 'Text files can go inside the message, so Claude reads them without opening attachments. Others stay attached.'
        : 'Attach sends them as a zip.'}
  >
    <select
      aria-label="Ticked files"
      value={filesOff ? 'none' : rules.files}
      disabled={filesOff}
      onchange={(e) => setRules({ files: e.currentTarget.value as typeof rules.files })}
    >
      <option value="attach">Attach them</option>
      {#if kind === 'claude'}<option value="inline">Write text files into the message</option>{/if}
      <option value="none">Leave them out</option>
    </select>
  </Field>
  {#if kind === 'claude' && rules.files === 'inline'}
    <Field label="Largest text file to write in" hint={`Bigger ones stay attached. 1 to ${MAX_INLINE_KB} KB.`}>
      <input
        type="number"
        min="1"
        max={MAX_INLINE_KB}
        aria-label="Largest text file to write in (KB)"
        value={limit}
        onchange={(e) => setRules({ inline_limit_kb: Math.min(MAX_INLINE_KB, Math.max(1, Math.round(Number(e.currentTarget.value) || DEFAULT_SEND_RULES.inline_limit_kb))) })}
      />
    </Field>
  {/if}
</section>

<style>
  .panel { display: grid; gap: 8px; border: 2px solid var(--kind-line); border-radius: var(--radius); padding: 8px 10px 10px; }
  h3 { font-size: 13px; margin: 4px 0 0; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  .cap { display: grid; grid-template-columns: 16px 1fr; gap: 0 6px; font-size: 12.5px; }
  .cap .note { grid-column: 2; }
  .icon { font-weight: 700; text-align: center; }
  .cap.no .icon { color: var(--err); }
  .cap.limited .icon { color: var(--warn); }
  .cap.yes .icon { color: var(--ok); }
  .word { color: var(--quiet); font-size: 11.5px; }
  .send { display: grid; grid-template-columns: 16px 1fr; gap: 6px; font-size: 12.5px; padding: 3px 6px; border-radius: var(--radius-sm); }
  .send.cannot_send { background: color-mix(in srgb, var(--err) 10%, transparent); }
  .send.switched_off, .send.not_used { color: var(--quiet); }
  .choose { display: grid; gap: 2px; min-width: 0; }
  .choose label { display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: center; }
  .choose select { min-width: 0; max-width: 100%; }
  .check { display: flex; gap: 6px; align-items: center; font-size: 13px; }
</style>
