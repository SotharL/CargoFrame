<script lang="ts">
  // The apps CargoFrame uses, whether they are on this computer, and one button for what is
  // missing. Windows installs ffmpeg by itself (winget); Linux gets the command to run.
  import { onDestroy, onMount } from 'svelte';
  import Section from '../../lib/components/Section.svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import Spinner from '../../lib/components/Spinner.svelte';
  import { getApi } from '../../lib/ipc';
  import { setupState } from '../setupState.svelte';
  import type { SetupItem, SetupReport } from '../../lib/types';

  // `minCheckMs`: a look at the computer is shown for at least this long, so the turning ring is
  // seen and the check mark that follows reads as an answer (and not as a flash).
  let { pollMs = 2500, minCheckMs = 350 }: { pollMs?: number; minCheckMs?: number } = $props();

  let report = $state<SetupReport | null>(null);
  let error = $state('');
  let checking = $state(true);
  let copied = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;
  let gone = false;

  function take(r: SetupReport) {
    report = r;
    setupState.missing = r.missing;
    // while an install runs, ask again until it is over
    if (r.install.type === 'running' && !timer && !gone) {
      timer = setInterval(() => void refresh(false), pollMs);
    } else if (r.install.type !== 'running' && timer) {
      clearInterval(timer);
      timer = undefined;
    }
  }

  async function refresh(showSpinner = true) {
    if (showSpinner) checking = true;
    try {
      const [next] = await Promise.all([
        (await getApi()).getSetup(),
        showSpinner ? new Promise((r) => setTimeout(r, minCheckMs)) : undefined,
      ]);
      take(next);
      error = '';
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }

  async function runInstall() {
    error = '';
    try {
      take(await (await getApi()).installMissing());
    } catch (e) {
      error = String(e);
    }
  }

  async function copy(command: string) {
    try {
      await navigator.clipboard.writeText(command);
      copied = true;
      setTimeout(() => (copied = false), 2500);
    } catch {
      // the command is on the page to select by hand
      error = 'Could not use the clipboard. Select the command and copy it.';
    }
  }

  async function openSound() {
    try {
      await (await getApi()).openSoundSettings();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => void refresh());
  onDestroy(() => {
    gone = true;
    if (timer) clearInterval(timer);
  });

  const chip = (i: SetupItem): { kind: 'ok' | 'warn' | 'dbg'; label: string } =>
    i.status === 'ok'
      ? { kind: 'ok', label: 'Found' }
      : i.status === 'missing'
        ? { kind: 'warn', label: 'Missing' }
        : { kind: 'dbg', label: 'Not checked yet' };
</script>

<p class="quiet lead">
  CargoFrame itself needs nothing else: screenshots and sharing work as soon as it is installed. These apps only
  switch on video, its sound, and pasting into a Claude chat.
</p>

<Section title="This computer">
  <div class="summary">
    {#if checking}
      <Spinner label="Checking this computer" />
      <span class="quiet">Checking this computer…</span>
    {:else if report && report.missing === 0}
      <Chip kind="ok" label="Everything CargoFrame can use is here" />
    {:else if report}
      <Chip kind="warn" label={report.missing === 1 ? '1 app is missing' : `${report.missing} apps are missing`} />
    {/if}
    <button type="button" class="again" onclick={() => refresh()} disabled={checking || report?.install.type === 'running'}>
      Check again
    </button>
  </div>

  <!-- Every line shows its name and whether it was found; the rest opens by itself only when
       something needs attention. -->
  <ul class="items">
    {#if !report}
      <li class="item"><div class="head"><strong>Looking at this computer…</strong><span class="state"><Spinner label="Checking" /></span></div></li>
    {:else}
      {#each report.items as i (i.id)}
        {@const c = chip(i)}
        <li class="item">
          <details open={i.status !== 'ok'}>
            <summary>
              <span class="caret" aria-hidden="true">▸</span>
              <strong>{i.name}</strong>
              <span class="state">
                {#if checking}<Spinner label={`Checking ${i.name}`} />{:else}<Chip kind={c.kind} label={c.label} />{/if}
              </span>
            </summary>
            <div class="body">
              <p class="quiet what">{i.what_for}</p>
              <p class="detail selectable">{i.detail}</p>
              {#if i.action === 'sound_settings'}
                <button type="button" onclick={openSound}>Open the Windows sound settings</button>
              {/if}
            </div>
          </details>
        </li>
      {/each}
    {/if}
  </ul>
</Section>

{#if report}
  {@const step = report.install}
  {#if step.type !== 'none'}
    <Section title="Install what is missing">
      {#if step.type === 'run'}
        <p class="quiet">
          Uses Windows' own package manager (winget). It downloads ffmpeg from its official catalogue, checks
          the download, and installs it for you only: no administrator password is asked.
        </p>
        <div><button type="button" class="primary" onclick={runInstall}>{step.label}</button></div>
      {:else if step.type === 'running'}
        <div role="status" class="running">
          <Chip kind="dbg" label="Installing ffmpeg…" />
          <span class="quiet">This can take a minute or two. You can keep using CargoFrame.</span>
        </div>
      {:else if step.type === 'failed'}
        <div role="alert" class="failed">
          <Chip kind="err" label="The install did not work" />
          <p class="selectable">{step.message}</p>
        </div>
        <div><button type="button" class="primary" onclick={runInstall}>Try again</button></div>
      {:else if step.type === 'copy'}
        <p class="quiet">
          Linux asks for your password to install programs, so CargoFrame cannot do it for you. Run this in a
          terminal, then press <em>Check again</em>.
        </p>
        <code class="command selectable">{step.command}</code>
        <div>
          <button type="button" class="primary" onclick={() => copy(step.command)}>
            {copied ? 'Copied' : 'Copy the command'}
          </button>
        </div>
      {:else if step.type === 'manual'}
        <p class="selectable">{step.text}</p>
      {/if}
    </Section>
  {/if}
{/if}

{#if error}
  <p class="error" role="alert">{error}</p>
{/if}

<style>
  .lead { margin: 0 0 12px; max-width: 70ch; }
  .summary { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .items { list-style: none; margin: 0; padding: 0; display: grid; gap: 12px; }
  .item { display: grid; gap: 4px; padding-top: 12px; border-top: 1px solid var(--border); }
  .item:first-child { border-top: 0; padding-top: 0; }
  .head { display: flex; align-items: center; gap: 8px; }
  .state { margin-left: auto; display: inline-flex; align-items: center; min-height: 22px; }
  summary { display: flex; align-items: center; gap: 8px; cursor: pointer; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  .caret { display: inline-block; width: 12px; color: var(--quiet); transition: transform 0.15s; }
  details[open] > summary .caret { transform: rotate(90deg); }
  .body { display: grid; gap: 4px; padding: 6px 0 0 20px; }
  .what, .detail { margin: 0; }
  .detail { font-size: 13px; }
  .body button { justify-self: start; margin-top: 4px; }
  @media (prefers-reduced-motion: reduce) { .caret { transition: none; } }
  .command {
    display: block; padding: 8px 10px; border: 1px solid var(--border);
    border-radius: var(--radius-sm); background: var(--surface-2); overflow-x: auto; white-space: pre;
  }
  .primary { font-weight: 600; }
  .running, .failed { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .failed { display: grid; gap: 4px; }
  .failed p { margin: 0; }
  .error { color: var(--err, #c0392b); }
</style>
