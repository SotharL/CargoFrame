<script lang="ts">
  import Field from '../../lib/components/Field.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import CustomForm from './CustomForm.svelte';
  import type { Connection } from '../../lib/types';

  let { value = $bindable() }: { value: Connection } = $props();
</script>

<Field label="Name" hint="How you will see it in lists.">
  <input type="text" bind:value={value.name} placeholder="Work Jira" />
</Field>

{#if value.kind === 'jira'}
  <Field label="Jira address" hint="For example https://yourteam.atlassian.net">
    <input type="text" bind:value={value.site_url} placeholder="https://yourteam.atlassian.net" />
  </Field>
  <Field label="Sign in with">
    <select bind:value={value.auth}>
      <option value="api_token">Jira Cloud: e-mail and API token</option>
      <option value="pat">Jira Server or Data Center: personal access token</option>
    </select>
  </Field>
  {#if value.auth === 'api_token'}
    <Field label="Account e-mail" hint="The address you sign in to Jira with.">
      <input type="text" bind:value={value.email} placeholder="you@example.com" />
    </Field>
  {/if}
{:else if value.kind === 'google'}
  <Field label="Client ID" hint="From a “Desktop app” OAuth client in your own Google Cloud project.">
    <input type="text" bind:value={value.client_id} />
  </Field>
  <Toggle label="Also add a row to a spreadsheet" bind:checked={value.sheets} />
  <Toggle label="Upload into any of my Drive folders" bind:checked={value.full_drive} />
  <p class="quiet">
    Off, CargoFrame can only use folders it created itself, which needs a narrower permission.
  </p>
{:else if value.kind === 'reddit'}
  {@const r = value}
  <Field label="Client ID" hint="From an “installed app” you register at reddit.com/prefs/apps.">
    <input type="text" bind:value={value.client_id} />
  </Field>
  <Field
    label="Redirect port"
    hint="The app's redirect address must be http://127.0.0.1:{r.redirect_port}/callback"
  >
    <input type="text" inputmode="numeric" value={String(r.redirect_port)}
      oninput={(e) => (r.redirect_port = Number(e.currentTarget.value) || 0)} />
  </Field>
{:else if value.kind === 'claude'}
  {@const claude = value}
  <Field label="Open Claude with" hint="The chat is opened by link, then CargoFrame pastes the screenshot, the ticked files and your message into it. Leave Claude in front for a few seconds while it works.">
    <select bind:value={claude.open_with}>
      <option value="auto">The desktop app on Windows, the browser on Linux</option>
      <option value="app">The desktop app</option>
      <option value="browser">The browser</option>
    </select>
  </Field>
  <Toggle label="Send the message for me (press Enter)" bind:checked={claude.send_automatically} />
  <p class="quiet">Turn this off to look at the message in Claude first and press Enter yourself.</p>
  <Field label="Extra seconds to wait before pasting" hint="Raise this if the chat is slow to open and the paste lands too early.">
    <input type="text" inputmode="numeric" value={String(claude.extra_wait_seconds)}
      oninput={(e) => (claude.extra_wait_seconds = Math.min(30, Math.max(0, Math.floor(Number(e.currentTarget.value)) || 0)))} />
  </Field>
{:else if value.kind === 'slack'}
  <p class="quiet">
    CargoFrame posts as a Slack app you create in your own workspace. Nothing here to fill in: the
    token you paste below says which workspace.
  </p>
  <details class="steps">
    <summary>How to get the token</summary>
    <ol>
      <li>Open <span class="mono">api.slack.com/apps</span> and choose Create New App, From scratch. Pick your workspace.</li>
      <li>
        Under OAuth &amp; Permissions, add these Bot Token Scopes:
        <span class="mono">chat:write</span>, <span class="mono">files:write</span>,
        <span class="mono">channels:read</span> and <span class="mono">groups:read</span>. Add
        <span class="mono">chat:write.public</span> too if you want to post in public channels the app has not joined.
      </li>
      <li>Choose Install to Workspace. Copy the Bot User OAuth Token, which starts with <span class="mono">xoxb-</span>.</li>
      <li>Save this connection, then paste the token below.</li>
      <li>In each channel you post to, type <span class="mono">/invite @</span> and the app's name. Files and private channels need this.</li>
    </ol>
  </details>
{:else if value.kind === 'discord'}
  <p class="quiet">
    CargoFrame posts as a Discord bot you create. Nothing here to fill in: the token you paste
    below, and the servers you add the bot to, decide where it can post.
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
{:else if value.kind === 'teams'}
  {@const t = value}
  <p class="quiet">
    CargoFrame posts as you, through an app you register in your own Microsoft Entra ID (free).
    Work and school accounts only; a personal Microsoft account cannot be used.
  </p>
  <Field label="Application (client) ID" hint="From the Overview page of your app registration.">
    <input type="text" bind:value={t.client_id} placeholder="00000000-0000-0000-0000-000000000000" />
  </Field>
  <Field
    label="Directory"
    hint="“organizations” works for any work or school account. Or write your Directory (tenant) ID or domain, to allow only your organisation."
  >
    <input type="text" bind:value={t.tenant} />
  </Field>
  <details class="steps">
    <summary>How to register the app</summary>
    <ol>
      <li>Open <span class="mono">entra.microsoft.com</span>, then App registrations, New registration. Name it CargoFrame.</li>
      <li>Under Supported account types choose <span class="mono">Accounts in any organizational directory</span> (or this directory only).</li>
      <li>
        Under Redirect URI choose <span class="mono">Public client/native (mobile &amp; desktop)</span> and write
        <span class="mono">http://localhost</span>. Register.
      </li>
      <li>On Overview, copy the Application (client) ID and paste it above.</li>
      <li>
        Under API permissions, Add a permission, Microsoft Graph, Delegated permissions:
        <span class="mono">Chat.ReadBasic</span>, <span class="mono">ChatMessage.Send</span>,
        <span class="mono">ChannelMessage.Send</span>, <span class="mono">Team.ReadBasic.All</span>,
        <span class="mono">Channel.ReadBasic.All</span> and <span class="mono">User.Read</span>.
      </li>
      <li>Under Authentication, make sure “Allow public client flows” is Yes.</li>
      <li>Save this connection and press Sign in. If your organisation asks for an administrator's approval, send them the page that appears.</li>
    </ol>
  </details>
{:else if value.kind === 'custom'}
  <CustomForm bind:value />
{/if}

<Toggle label="Enabled" bind:checked={value.enabled} />

<style>
  .steps { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px; }
  .steps summary { cursor: pointer; font-weight: 600; font-size: 13px; }
  .steps ol { margin: 8px 0 2px; padding-left: 20px; display: grid; gap: 6px; font-size: 13px; }
</style>
