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
  <Field label="Open Claude with" hint="Nothing is sent to Claude by CargoFrame: the picture goes to your clipboard and the chat opens.">
    <select bind:value={value.open_with}>
      <option value="auto">The desktop app on Windows, the browser on Linux</option>
      <option value="app">The desktop app</option>
      <option value="browser">The browser</option>
    </select>
  </Field>
  <Toggle label="Put the prompt on the clipboard with the picture" bind:checked={value.image_and_text_together} />
  <p class="quiet">Leave this off unless a single paste brings both into Claude's message box.</p>
{:else if value.kind === 'custom'}
  <CustomForm bind:value />
{/if}

<Toggle label="Enabled" bind:checked={value.enabled} />
