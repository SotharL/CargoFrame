<script lang="ts">
  // Discord: a webhook per channel (no bot, the easy way) or a bot (can list servers and channels).
  import Field from '../../lib/components/Field.svelte';
  import Segmented from '../../lib/components/Segmented.svelte';
  import GetLink from '../../lib/components/GetLink.svelte';
  import SecretInput from './SecretInput.svelte';
  import { getLinkFor } from '../../lib/connectorHelp';
  import type { Connection } from '../../lib/types';

  type Discord = Extract<Connection, { kind: 'discord' }>;
  /** The webhook links are kept by slot; the connection must exist before one can be stored. */
  export interface HookStore {
    stored: (slot: number) => boolean;
    save: (slot: number, link: string) => Promise<void>;
    clear: (slot: number) => Promise<void>;
  }

  /** Most channels one connection holds a link for (core/connectors model::MAX_HOOKS). */
  const MAX = 10;

  let { value = $bindable(), hooks = null }: { value: Discord; hooks?: HookStore | null } = $props();

  const nextSlot = () => Array.from({ length: MAX }, (_, i) => i).find((i) => !value.hooks.some((h) => h.slot === i));
  function add() {
    const slot = nextSlot();
    if (slot !== undefined) value.hooks.push({ slot, name: '', server: '' });
  }
  async function remove(slot: number) {
    value.hooks = value.hooks.filter((h) => h.slot !== slot);
    // the link goes with the channel; a connection that is not saved yet has none to clear
    if (hooks) await hooks.clear(slot).catch(() => {});
  }
</script>

<Field label="How should CargoFrame post?" tour="discord.mode">
  <Segmented
    label="How CargoFrame posts to Discord"
    bind:value={value.mode}
    options={[
      { value: 'webhook', label: 'Webhook (easiest)' },
      { value: 'bot', label: 'Bot (advanced)' },
    ]}
  />
</Field>

{#if value.mode === 'webhook'}
  <p class="quiet">
    A webhook is a private link to one channel. No bot, no developer account: you copy the link
    from the channel's settings. Posts appear under the name you choose, and nobody is pinged.
  </p>
  <Field label="Name shown on posts (optional)" hint="Leave it empty to keep the webhook's own name. Discord does not allow “discord” in it.">
    <input type="text" bind:value={value.shown_as} placeholder="Your name" maxlength="80" />
  </Field>

  <div class="channels" data-tour="discord.channels">
    <span class="title">Channels</span>
    {#each value.hooks as h (h.slot)}
      <div class="channel">
        <div class="pair">
          <Field label="Name" hint="How you will see it in the list.">
            <input type="text" bind:value={h.name} placeholder="bugs" />
          </Field>
          <Field label="Server (optional)" hint="Groups the list when you post in several servers.">
            <input type="text" bind:value={h.server} placeholder="My team" />
          </Field>
        </div>
        {#if hooks}
          <SecretInput
            label="Webhook link"
            hint="Kept in your system's password store. You cannot read it back from here."
            stored={hooks.stored(h.slot)}
            get={getLinkFor(value, 'webhook')}
            onsave={(link) => hooks.save(h.slot, link)}
            onclear={() => hooks.clear(h.slot)}
          />
        {:else}
          <p class="quiet">Save this connection first, then paste the link here.</p>
        {/if}
        <button type="button" class="remove" onclick={() => remove(h.slot)}>Remove this channel</button>
      </div>
    {/each}
    {#if value.hooks.length < MAX}
      <button type="button" data-tour="discord.add-channel" onclick={add}>{value.hooks.length === 0 ? 'Add a channel' : 'Add another channel'}</button>
    {/if}
  </div>

  <details class="steps" data-tour="discord.steps">
    <summary>How to make a webhook</summary>
    <ol>
      <li>In Discord, open the channel's settings (the gear beside its name), then Integrations, then Webhooks.</li>
      <li>Choose New Webhook, then Copy Webhook URL. You need the Manage Webhooks permission in that channel.</li>
      <li>Add a channel above, give it a name, and paste the link. Do this once for each channel you post in.</li>
      <li>Forums work too: the webhook starts a new post named from your title.</li>
      <li>To reply in an existing thread, paste the thread's link in the Thread box of the layout (Copy Link in Discord).</li>
    </ol>
    <p class="quiet">Anyone who has a webhook link can post to that channel, so keep it private. If one leaks, delete the webhook in Discord and make a new one.</p>
    <GetLink link={getLinkFor(value, 'webhook')} />
  </details>
{:else}
  <p class="quiet">
    CargoFrame posts as a Discord bot you create. Nothing here to fill in: the token you paste
    below, and the servers you add the bot to, decide where it can post. A bot can list your
    servers, categories, channels and threads, so you pick instead of pasting links.
  </p>
  <details class="steps">
    <summary>How to get the token</summary>
    <ol>
      <li>Open <span class="mono">discord.com/developers/applications</span> and choose New Application.</li>
      <li>Under Bot, choose Reset Token and copy it. You do not need any of the privileged intents.</li>
      <li>
        Under OAuth2, URL Generator: tick <span class="mono">bot</span>, then the permissions
        View Channels, Send Messages, Send Messages in Threads, Attach Files and Create Public
        Threads (for forum posts).
      </li>
      <li>Open the address it makes and add the bot to your server.</li>
      <li>Save this connection, then paste the token below. Test connection says how many servers the bot is in.</li>
    </ol>
  </details>
{/if}

<style>
  .channels { display: grid; gap: 10px; }
  .channels > button { justify-self: start; }
  .title { font-weight: 600; font-size: 13px; }
  .channel { display: grid; gap: 8px; padding: 10px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm); }
  .pair { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .remove { justify-self: start; }
  .steps { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px; }
  .steps summary { cursor: pointer; font-weight: 600; font-size: 13px; }
  .steps ol { margin: 8px 0 2px; padding-left: 20px; display: grid; gap: 6px; font-size: 13px; }
</style>
