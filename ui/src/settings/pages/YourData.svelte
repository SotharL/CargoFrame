<script lang="ts">
  // What CargoFrame keeps on this computer, with a way to clear each kind, and a way back to a new
  // install. Nothing here touches the program itself or anything outside CargoFrame's own folders.
  import { onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { getApi } from '../../lib/ipc';
  import { forgetRecent, remembered } from '../../lib/recent';
  import { formatSize } from '../../capture/values';
  import type { Cleaned, DataReport } from '../../lib/types';

  let { reload = () => location.reload() }: { reload?: () => void } = $props();

  let report = $state<DataReport | null>(null);
  let remembering = $state(0);
  let message = $state<{ kind: 'ok' | 'warn' | 'err'; text: string } | null>(null);
  let confirming = $state(false);
  let busy = $state(false);

  async function read() {
    try {
      report = await (await getApi()).getDataReport();
    } catch (e) {
      message = { kind: 'err', text: String(e) };
    }
    remembering = remembered();
  }
  onMount(() => void read());

  function said(done: Cleaned): { kind: 'ok' | 'warn'; text: string } {
    const text =
      done.files === 0 && done.left === 0
        ? 'Nothing to remove.'
        : `Removed ${done.files} ${done.files === 1 ? 'file' : 'files'} (${formatSize(done.bytes)}).`;
    return done.left > 0
      ? { kind: 'warn', text: `${text} ${done.left} in use ${done.left === 1 ? 'was' : 'were'} left.` }
      : { kind: 'ok', text };
  }

  async function clear(ids: string[]) {
    busy = true;
    message = null;
    try {
      message = said(await (await getApi()).cleanData(ids));
    } catch (e) {
      message = { kind: 'err', text: String(e) };
    }
    await read();
    busy = false;
  }

  function forget() {
    const n = forgetRecent();
    message = { kind: 'ok', text: n === 0 ? 'Nothing was remembered.' : 'The last choices of every dropdown are forgotten.' };
    remembering = remembered();
  }

  async function reset() {
    busy = true;
    message = null;
    try {
      await (await getApi()).resetEverything();
      forgetRecent();
      confirming = false;
      message = { kind: 'ok', text: 'CargoFrame is back to a new install.' };
      reload();
    } catch (e) {
      confirming = false;
      message = { kind: 'err', text: String(e) };
    }
    await read();
    busy = false;
  }
</script>

<Section title="Your data" hint="Everything CargoFrame keeps stays in these two folders on this computer.">
  {#if report}
    <p class="quiet">Settings: <span class="mono selectable">{report.config_dir}</span></p>
    <p class="quiet">Logs, kept zips and work in progress: <span class="mono selectable">{report.data_dir}</span></p>
    <div>
      <button type="button" onclick={async () => (await getApi()).openConfigFolder()}>Open settings folder</button>
    </div>

    <ul class="areas">
      {#each report.areas as a (a.id)}
        <li>
          <div class="what">
            <strong>{a.label}</strong>
            <span class="quiet">{a.what}</span>
          </div>
          <span class="size">{a.files === 0 ? 'Empty' : `${a.files} ${a.files === 1 ? 'file' : 'files'}, ${formatSize(a.bytes)}`}</span>
          <button type="button" onclick={() => clear([a.id])} disabled={busy || a.files === 0} aria-label={`Clear ${a.label}`}>Clear</button>
        </li>
      {/each}
      <li>
        <div class="what">
          <strong>Remembered choices</strong>
          <span class="quiet">The last choices of your dropdowns (a channel, a project), so the next capture starts on them.</span>
        </div>
        <span class="size">{remembering === 0 ? 'None' : remembering === 1 ? '1 dropdown' : `${remembering} dropdowns`}</span>
        <button type="button" onclick={forget} disabled={busy || remembering === 0} aria-label="Forget the remembered choices">Forget</button>
      </li>
    </ul>

    <div class="reset">
      <h3>Start again</h3>
      <p class="quiet">
        Removes every trigger, layout, value from a file, extra field and connection, forgets the sign-ins and tokens kept for
        them, and clears all of the above. CargoFrame itself stays installed, and so do the files you saved zips to.
      </p>
      {#if confirming}
        <div class="sure" role="alert">
          <span>This cannot be undone. Export your settings first if you may want them back.</span>
          <button type="button" class="danger" onclick={reset} disabled={busy}>Yes, reset CargoFrame</button>
          <button type="button" onclick={() => (confirming = false)}>Keep everything</button>
        </div>
      {:else}
        <div><button type="button" class="danger-outline" onclick={() => (confirming = true)} disabled={busy}>Reset CargoFrame…</button></div>
      {/if}
    </div>
  {:else if !message}
    <p class="quiet" role="status">Looking at what is kept…</p>
  {/if}
  {#if message}
    <div role="status" aria-live="polite"><Chip kind={message.kind} label={message.text} /></div>
  {/if}
</Section>

<style>
  .areas { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .areas li { display: grid; grid-template-columns: 1fr auto auto; gap: 12px; align-items: center; padding-top: 10px; border-top: 1px solid var(--border); }
  .areas li:first-child { border-top: 0; padding-top: 0; }
  .what { display: grid; gap: 2px; min-width: 0; }
  .what .quiet { font-size: 12.5px; }
  .size { font-size: 13px; color: var(--quiet); white-space: nowrap; }
  .reset { display: grid; gap: 8px; padding-top: 12px; border-top: 1px solid var(--border); }
  .reset h3 { font-size: 13.5px; }
  .sure { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .danger { border-color: var(--err); background: color-mix(in srgb, var(--err) 14%, transparent); }
  .danger-outline { border-color: var(--err); }
  @media (max-width: 560px) { .areas li { grid-template-columns: 1fr auto; } .size { grid-column: 1; } }
</style>
