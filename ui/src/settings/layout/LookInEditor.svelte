<script lang="ts">
  import Field from '../../lib/components/Field.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Block, FilesPreview, LookFolder, LookIn } from '../../lib/types';

  type FilesBlock = Extract<Block, { type: 'files' }>;
  let { block }: { block: FilesBlock } = $props();

  const CHOICES: { value: LookFolder; label: string }[] = [
    { value: 'project', label: 'The project folder' },
    { value: 'program', label: "The program's own folder" },
    { value: 'above_1', label: 'One folder above that' },
    { value: 'above_2', label: 'Two folders above' },
    { value: 'above_3', label: 'Three folders above' },
  ];

  const look = $derived<LookIn>(block.look_in ?? { folders: ['project'], subfolder: '' });
  const chosen = $derived(look.folders.length ? look.folders : (['project'] as LookFolder[]));

  /** The settings as they are right now, read fresh so two quick changes never overwrite each other. */
  function current(): LookIn {
    const l = block.look_in ?? { folders: [], subfolder: '' };
    return { folders: l.folders.length ? [...l.folders] : ['project'], subfolder: l.subfolder };
  }

  function toggle(which: LookFolder, on: boolean) {
    const now = current();
    const next = on ? [...new Set([...now.folders, which])] : now.folders.filter((f) => f !== which);
    // Something is always chosen: with nothing, the project folder is used anyway.
    block.look_in = { ...now, folders: next.length ? next : ['project'] };
  }

  // The live count: what the settings above would find for a real program. Asked a moment after
  // the last change, and an answer that arrives late for an older setting is ignored.
  let tryWith = $state('');
  let preview = $state<FilesPreview | null>(null);
  let waiting = $state(false);
  let latest = 0;
  $effect(() => {
    const snapshot = $state.snapshot(block) as FilesBlock;
    const program = tryWith.trim() || null;
    const mine = ++latest;
    waiting = true;
    const timer = setTimeout(async () => {
      let result: FilesPreview;
      try {
        result = await (await getApi()).previewFiles(snapshot, program);
      } catch (e) {
        result = { program, places: [], missing: [], total: 0, ticked: 0, truncated: false, problem: String(e) };
      }
      if (mine === latest) {
        preview = result;
        waiting = false;
      }
    }, 300);
    return () => clearTimeout(timer);
  });

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
</script>

<Field
  label="Look in"
  hint="Found from the program that had focus when you captured. Patterns above are matched inside each folder you tick."
>
  <div class="choices">
    {#each CHOICES as c (c.value)}
      <label class="choice">
        <input
          type="checkbox"
          checked={chosen.includes(c.value)}
          disabled={chosen.length === 1 && chosen[0] === c.value}
          onchange={(e) => toggle(c.value, e.currentTarget.checked)}
        />
        {c.label}
      </label>
    {/each}
  </div>
</Field>
<Field label="Subfolder in each" hint="Optional. A folder name such as logs, searched inside every folder ticked above.">
  <input
    type="text"
    value={look.subfolder}
    placeholder="logs"
    oninput={(e) => (block.look_in = { ...current(), subfolder: e.currentTarget.value })}
  />
</Field>
<div class="count" aria-live="polite">
  {#if preview === null}
    <span class="quiet">Counting…</span>
  {:else if preview.problem && preview.places.length === 0}
    <span class="quiet">{preview.problem}</span>
  {:else}
    <strong>{plural(preview.total, 'file')} found</strong>{#if preview.ticked}, {preview.ticked} ticked by default{/if}{#if preview.truncated}, more exist{/if}
    <ul>
      {#each preview.places as p (p.folder)}
        <li class:gone={!p.exists} title={p.folder}>
          {p.label}: {p.exists ? plural(p.found, 'file') : 'folder not found'}
        </li>
      {/each}
      {#each preview.missing as m (m)}
        <li class="gone">{m}</li>
      {/each}
    </ul>
    {#if preview.problem}<span class="quiet">{preview.problem}</span>{/if}
  {/if}
  {#if waiting && preview !== null}<span class="quiet"> updating…</span>{/if}
</div>
<Field label="Try with" hint="Optional: the path of a program file to count against. Empty uses the program of your last capture.">
  <input type="text" bind:value={tryWith} placeholder={preview?.program ?? 'C:\\Dev\\Foo\\bin\\Foo.exe'} />
</Field>

<style>
  .choices { display: grid; gap: 4px; }
  .choice { display: flex; gap: 8px; align-items: center; font-size: 13px; }
  .count { font-size: 12.5px; display: grid; gap: 2px; }
  .count ul { margin: 2px 0 0; padding-left: 18px; }
  .gone { color: var(--quiet); }
</style>
