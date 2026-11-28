<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import Tr from '../../lib/i18n/Tr.svelte';
  import Field from '../../lib/components/Field.svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import CustomForm from './CustomForm.svelte';
  import DiscordForm, { type HookStore } from './DiscordForm.svelte';
  import ClaudeAppNote from './ClaudeAppNote.svelte';
  import GetLink from '../../lib/components/GetLink.svelte';
  import { getLinkFor } from '../../lib/connectorHelp';
  import type { Connection } from '../../lib/types';

  let { value = $bindable(), hooks = null }: { value: Connection; hooks?: HookStore | null } = $props();
</script>

<Field label={t('Name')} hint={t('How you will see it in lists.')} tour="conn.name">
  <input type="text" bind:value={value.name} placeholder={t('Work Jira')} />
</Field>

{#if value.kind === 'jira'}
  <Field label={t('Jira address')} hint={t('For example https://yourteam.atlassian.net')} tour="jira.address">
    <input type="text" bind:value={value.site_url} placeholder="https://yourteam.atlassian.net" />
  </Field>
  <Field label={t('Sign in with')} tour="jira.auth">
    <select bind:value={value.auth}>
      <option value="api_token">{t('Jira Cloud: e-mail and API token')}</option>
      <option value="pat">{t('Jira Server or Data Center: personal access token')}</option>
    </select>
  </Field>
  {#if value.auth === 'api_token'}
    <Field label={t('Account e-mail')} hint={t('The address you sign in to Jira with.')} tour="jira.email">
      <input type="text" bind:value={value.email} placeholder="you@example.com" />
    </Field>
  {/if}
{:else if value.kind === 'google'}
  <Field label={t('Client ID')} hint={t('From a “Desktop app” OAuth client in your own Google Cloud project.')} tour="google.client_id">
    <span class="with-link"><input type="text" bind:value={value.client_id} /><GetLink link={getLinkFor(value, 'client_id')} /></span>
  </Field>
  <Toggle label={t('Also add a row to a spreadsheet')} bind:checked={value.sheets} />
  <Toggle label={t('Upload into any of my Drive folders')} bind:checked={value.full_drive} />
  <p class="quiet">
    {t('Off, CargoFrame can only use folders it created itself, which needs a narrower permission.')}
  </p>
{:else if value.kind === 'reddit'}
  {@const r = value}
  <Field label={t('Client ID')} hint={t('From an “installed app” you register at reddit.com/prefs/apps.')} tour="reddit.client_id">
    <span class="with-link"><input type="text" bind:value={value.client_id} /><GetLink link={getLinkFor(value, 'client_id')} /></span>
  </Field>
  <Field
    label={t('Redirect port')}
    hint={t("The app's redirect address must be {address}", { address: `http://127.0.0.1:${r.redirect_port}/callback` })}
  >
    <input type="text" inputmode="numeric" value={String(r.redirect_port)}
      oninput={(e) => (r.redirect_port = Number(e.currentTarget.value) || 0)} />
  </Field>
{:else if value.kind === 'claude'}
  {@const claude = value}
  <Field label={t('Open Claude with')} hint={t('The chat is opened by link, then CargoFrame pastes the screenshot, the ticked files and your message into it. Leave Claude in front for a few seconds while it works.')} tour="claude.open_with">
    <select bind:value={claude.open_with}>
      <option value="auto">{t('The desktop app on Windows, the browser on Linux')}</option>
      <option value="app">{t('The desktop app')}</option>
      <option value="browser">{t('The browser')}</option>
    </select>
  </Field>
  <ClaudeAppNote />
  <Toggle label={t('Send the message for me (press Enter)')} bind:checked={claude.send_automatically} />
  <p class="quiet">{t('Turn this off to look at the message in Claude first and press Enter yourself.')}</p>
  <Field label={t('Extra seconds to wait before pasting')} hint={t('Raise this if the chat is slow to open and the paste lands too early.')}>
    <input type="text" inputmode="numeric" value={String(claude.extra_wait_seconds)}
      oninput={(e) => (claude.extra_wait_seconds = Math.min(30, Math.max(0, Math.floor(Number(e.currentTarget.value)) || 0)))} />
  </Field>
{:else if value.kind === 'slack'}
  <p class="quiet">
    {t('CargoFrame posts as a Slack app you create in your own workspace. Nothing here to fill in: the token you paste below says which workspace.')}
  </p>
  <details class="steps" data-tour="slack.steps">
    <summary>{t('How to get the token')}</summary>
    <ol>
      <li><Tr text={t('Open {site} and choose Create New App, From scratch. Pick your workspace.')}>{#snippet site()}<span class="mono">api.slack.com/apps</span>{/snippet}</Tr></li>
      <li>
        <Tr text={t('Under OAuth & Permissions, add these Bot Token Scopes: {scopes}. Add {everywhere} too if you want to post in public channels the app has not joined.')}>
          {#snippet scopes()}<span class="mono">chat:write, files:write, channels:read, groups:read</span>{/snippet}
          {#snippet everywhere()}<span class="mono">chat:write.public</span>{/snippet}
        </Tr>
      </li>
      <li><Tr text={t('Choose Install to Workspace. Copy the Bot User OAuth Token, which starts with {start}.')}>{#snippet start()}<span class="mono">xoxb-</span>{/snippet}</Tr></li>
      <li>{t('Save this connection, then paste the token below.')}</li>
      <li><Tr text={t("In each channel you post to, type {invite} and the app's name. Files and private channels need this.")}>{#snippet invite()}<span class="mono">/invite @</span>{/snippet}</Tr></li>
    </ol>
  </details>
{:else if value.kind === 'discord'}
  <DiscordForm bind:value {hooks} />
{:else if value.kind === 'teams'}
  {@const teams = value}
  <p class="quiet">
    {t('CargoFrame posts as you, through an app you register in your own Microsoft Entra ID (free). Work and school accounts only; a personal Microsoft account cannot be used.')}
  </p>
  <Field label={t('Application (client) ID')} hint={t('From the Overview page of your app registration.')} tour="teams.client_id">
    <span class="with-link"><input type="text" bind:value={teams.client_id} placeholder="00000000-0000-0000-0000-000000000000" /><GetLink link={getLinkFor(teams, 'client_id')} /></span>
  </Field>
  <Field
    label={t('Directory')}
    hint={t('“organizations” works for any work or school account. Or write your Directory (tenant) ID or domain, to allow only your organisation.')}
  >
    <input type="text" bind:value={teams.tenant} />
  </Field>
  <details class="steps" data-tour="teams.steps">
    <summary>{t('How to register the app')}</summary>
    <ol>
      <li><Tr text={t('Open {site}, then App registrations, New registration. Name it CargoFrame.')}>{#snippet site()}<span class="mono">entra.microsoft.com</span>{/snippet}</Tr></li>
      <li><Tr text={t('Under Supported account types choose {choice} (or this directory only).')}>{#snippet choice()}<span class="mono">Accounts in any organizational directory</span>{/snippet}</Tr></li>
      <li>
        <Tr text={t('Under Redirect URI choose {choice} and write {address}. Register.')}>
          {#snippet choice()}<span class="mono">Public client/native (mobile &amp; desktop)</span>{/snippet}
          {#snippet address()}<span class="mono">http://localhost</span>{/snippet}
        </Tr>
      </li>
      <li>{t('On Overview, copy the Application (client) ID and paste it above.')}</li>
      <li>
        <Tr text={t('Under API permissions, Add a permission, Microsoft Graph, Delegated permissions: {permissions}.')}>
          {#snippet permissions()}<span class="mono">Chat.ReadBasic, ChatMessage.Send, ChannelMessage.Send, Team.ReadBasic.All, Channel.ReadBasic.All, Files.ReadWrite.All, User.Read</span>{/snippet}
        </Tr>
      </li>
      <li>{t('Under Authentication, make sure “Allow public client flows” is Yes.')}</li>
      <li>{t('Save this connection and press Sign in. If your organisation asks for an administrator\'s approval, send them the page that appears.')}</li>
    </ol>
  </details>
{:else if value.kind === 'x'}
  {@const x = value}
  <p class="quiet">
    {t('CargoFrame posts as you, through an app you make in the X developer portal. Your app, your account: X may charge for posting through its API.')}
  </p>
  <Field label={t('Client ID')} hint={t('OAuth 2.0 Client ID, from your app\'s Keys and tokens page.')} tour="x.client_id">
    <span class="with-link"><input type="text" bind:value={x.client_id} /><GetLink link={getLinkFor(x, 'client_id')} /></span>
  </Field>
  <Field label={t('Callback port')} hint={t("The app's callback address must be {address}", { address: `http://127.0.0.1:${x.redirect_port}/callback` })}>
    <input type="text" inputmode="numeric" value={String(x.redirect_port)}
      oninput={(e) => (x.redirect_port = Number(e.currentTarget.value) || 0)} />
  </Field>
  <details class="steps" data-tour="x.steps">
    <summary>{t('How to make the app')}</summary>
    <ol>
      <li>{t('Open the X developer portal and make a project and an app.')}</li>
      <li>
        <Tr text={t('Under User authentication settings choose {access}, and the type {type}.')}>
          {#snippet access()}<span class="mono">Read and write</span>{/snippet}
          {#snippet type()}<span class="mono">Native App</span>{/snippet}
        </Tr>
      </li>
      <li><Tr text={t('As Callback URI write {address}, and any website address. Save.')}>{#snippet address()}<span class="mono">{`http://127.0.0.1:${x.redirect_port}/callback`}</span>{/snippet}</Tr></li>
      <li>{t('Copy the OAuth 2.0 Client ID and paste it above. There is no secret to copy.')}</li>
      <li>{t('Save this connection and press Sign in.')}</li>
    </ol>
  </details>
{:else if value.kind === 'github'}
  {@const g = value}
  <Field label={t('Repository')} hint={t('Where issues go, as owner/name, like octo-org/app. A layout can pick another one.')} tour="github.repository">
    <input type="text" bind:value={g.repository} placeholder="octo-org/app" />
  </Field>
  <Field label={t('Pictures and files')} hint={t('GitHub cannot attach a file to an issue, so CargoFrame puts them in a branch of the repository and shows them in the issue.')} tour="github.files">
    <select bind:value={g.files}>
      <option value="branch">{t('Put them in a branch of the repository')}</option>
      <option value="none">{t('Do not send them (text only)')}</option>
    </select>
  </Field>
  {#if g.files === 'branch'}
    <Field label={t('Branch for pictures')} hint={t('Made from the main branch the first time.')}>
      <input type="text" bind:value={g.branch} placeholder="cargoframe-attachments" />
    </Field>
  {/if}
  <p class="quiet">
    {t('After saving, paste a fine-grained token below, for this repository only: Issues read and write, and Contents read and write for the pictures.')}
  </p>
  <details class="steps">
    <summary>{t('My company has its own GitHub')}</summary>
    <Field label={t('API address')} hint={t('GitHub Enterprise Server: https://github.example.com/api/v3. Everyone else leaves https://api.github.com.')}>
      <input type="text" bind:value={g.api_url} />
    </Field>
  </details>
{:else if value.kind === 'bluesky'}
  {@const b = value}
  <Field label={t('Handle')} hint={t('Your Bluesky name, like jane.bsky.social.')} tour="bluesky.handle">
    <input type="text" bind:value={b.handle} placeholder="jane.bsky.social" />
  </Field>
  <p class="quiet">
    {t('After saving, paste an app password below: in Bluesky open Settings, Privacy and security, App passwords, and add one called CargoFrame. Never use your real password.')}
  </p>
  <details class="steps">
    <summary>{t('My account is not on bsky.social')}</summary>
    <Field label={t('Server')} hint={t('Only for an account on its own server. Most people leave https://bsky.social.')}>
      <input type="text" bind:value={b.service} />
    </Field>
  </details>
{:else if value.kind === 'instagram'}
  {@const ig = value}
  <p class="quiet">
    {t('For a professional (business or creator) Instagram account. Instagram only takes a picture from a public web address, so CargoFrame first puts the picture on a server you or your company own, then asks Instagram to post it.')}
  </p>
  <Field label={t('Upload address')} hint={t('CargoFrame sends the picture here with a PUT. Write {name} where the file name goes.')} tour="instagram.upload">
    <input type="text" bind:value={ig.upload_url} placeholder={'https://storage.example.com/shots/{name}'} />
  </Field>
  <Field label={t('Public address')} hint={t('Where the same file can be opened by anyone. Also with {name}.')} tour="instagram.public">
    <input type="text" bind:value={ig.public_url} placeholder={'https://cdn.example.com/shots/{name}'} />
  </Field>
  <Field label={t('Header for the server\'s key')} hint={t('The header that carries the upload key, like AccessKey or Authorization. Leave empty when the upload address needs no key.')}>
    <input type="text" bind:value={ig.host_header} placeholder="AccessKey" />
  </Field>
  <details class="steps" data-tour="instagram.steps">
    <summary>{t('How to get the access token')}</summary>
    <ol>
      <li>{t('Open your Meta apps and make an app with the Instagram product (Instagram API with Instagram login).')}</li>
      <li>{t('Under API setup, add your Instagram professional account and press Generate token.')}</li>
      <li>{t('Save this connection, then paste the token below. It is renewed for 60 days each time it is used.')}</li>
    </ol>
  </details>
{:else if value.kind === 'custom'}
  <CustomForm bind:value />
{/if}

<Toggle label={t('Enabled')} bind:checked={value.enabled} />

<style>
  .with-link { display: flex; gap: 8px; align-items: center; }
  .with-link input { flex: 1; min-width: 0; }
  .steps { border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 12px; }
  .steps summary { cursor: pointer; font-weight: 600; font-size: 13px; }
  .steps ol { margin: 8px 0 2px; padding-left: 20px; display: grid; gap: 6px; font-size: 13px; }
</style>
