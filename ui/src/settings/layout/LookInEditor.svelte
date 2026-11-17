<script lang="ts">
  import { t, tn } from '../../lib/i18n/index.svelte';
  import Field from '../../lib/components/Field.svelte';
  import { getApi } from '../../lib/ipc';
  import type { Block, FilesPreview, LookFolder, LookIn } from '../../lib/types';

  type FilesBlock = Extract<Block, { type: 'files' }>;
  let { block }: { block: FilesBlock } = $props();

  const CHOICES: { value: LookFolder; label: string }[] = [
    { value: 'project', label: t('The project folder') },
    { value: 'program', label: t("The program's own folder") },
    { value: 'above_1', label: t('One folder above that') },
    { value: 'above_2', label: t('Two folders above') },
    { value: 'above_3', label: t('Three folders above') },
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

</script>

<Field
  label={t('Look in')}
  hint={t('Found from the program that had focus when you captured. Patterns above are matched inside each folder you tick.')}
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
<Field label={t('Subfolders in each')} hint={t('Optional. Folder names such as logs, crash, separated by commas: each is searched inside every folder ticked above.')}>
  <input
    type="text"
    value={look.subfolder}
    placeholder={t('logs, crash')}
    oninput={(e) => (block.look_in = { ...current(), subfolder: e.currentTarget.value })}
  />
</Field>
<div class="count" aria-live="polite">
  {#if preview === null}
    <span class="quiet">{t('Counting…')}</span>
  {:else if preview.problem && preview.places.length === 0}
    <span class="quiet">{preview.problem}</span>
  {:else}
    <strong>{tn('{n} file found', t('{n} files found'), preview.total)}</strong>{#if preview.ticked}, {tn('{n} ticked by default', t('{n} ticked by default'), preview.ticked)}{/if}{#if preview.truncated}, {t('more exist')}{/if}
    <ul>
      {#each preview.places as p (p.folder)}
        <li class:gone={!p.exists} title={p.folder}>
          {p.label}: {p.exists ? tn('{n} file', '{n} files', p.found) : t('folder not found')}
        </li>
      {/each}
      {#each preview.missing as m (m)}
        <li class="gone">{m}</li>
      {/each}
    </ul>
    {#if preview.problem}<span class="quiet">{preview.problem}</span>{/if}
  {/if}
  {#if waiting && preview !== null}<span class="quiet"> {t('updating…')}</span>{/if}
</div>
<Field label={t('Try with')} hint={t('Optional: the path of a program file to count against. Empty uses the program of your last capture.')}>
  <input type="text" bind:value={tryWith} placeholder={preview?.program ?? 'C:\\Dev\\Foo\\bin\\Foo.exe'} />
</Field>

<style>
  .choices { display: grid; gap: 4px; }
  .choice { display: flex; gap: 8px; align-items: center; font-size: 13px; }
  .count { font-size: 12.5px; display: grid; gap: 2px; }
  .count ul { margin: 2px 0 0; padding-left: 18px; }
  .gone { color: var(--quiet); }
</style>
