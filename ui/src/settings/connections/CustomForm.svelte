<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte';
  import Field from '../../lib/components/Field.svelte';
  import PairsEditor from './PairsEditor.svelte';
  import type { Connection, CustomAuth, CustomBody } from '../../lib/types';

  type Custom = Extract<Connection, { kind: 'custom' }>;
  let { value = $bindable() }: { value: Custom } = $props();

  const AUTHS: { value: CustomAuth['type']; label: string }[] = [
    { value: 'none', label: t('No sign-in') },
    { value: 'bearer', label: t('Bearer token (Authorization header)') },
    { value: 'header', label: t('A header with the key') },
    { value: 'basic', label: t('User name and password (basic)') },
    { value: 'query', label: t('A query parameter with the key') },
    { value: 'oauth2', label: t('OAuth 2.0: sign in in the browser (company sign-in)') },
    { value: 'client_credentials', label: t('OAuth 2.0: client credentials (a service account)') },
  ];
  const BODIES: { value: CustomBody['type']; label: string }[] = [
    { value: 'none', label: t('No body') },
    { value: 'json', label: 'JSON' },
    { value: 'form', label: t('Form fields') },
    { value: 'multipart', label: t('Form with the screenshot and files') },
  ];

  function setAuth(type: CustomAuth['type']) {
    value.auth =
      type === 'header' ? { type, name: 'X-Api-Key' }
      : type === 'basic' ? { type, user: '' }
      : type === 'query' ? { type, param: 'key' }
      : type === 'oauth2' ? { type, auth_url: 'https://', token_url: 'https://', client_id: '', scopes: '', redirect_port: 53684 }
      : type === 'client_credentials' ? { type, token_url: 'https://', client_id: '', scopes: '' }
      : { type };
  }
  function setBody(type: CustomBody['type']) {
    value.send.body =
      type === 'json' ? { type, template: '{\n  "text": "{{prompt}}"\n}' }
      : type === 'form' ? { type, fields: [{ name: 'text', value: '{{prompt}}' }] }
      : type === 'multipart' ? { type, fields: [{ name: 'text', value: '{{prompt}}' }], image_field: 'screenshot', files_field: '' }
      : { type };
  }
</script>

<Field label={t('Base address')} hint={t('Every request goes under this address. It must start with https://.')} tour="custom.url">
  <input type="text" bind:value={value.base_url} placeholder="https://api.example.com/v1" />
</Field>

<Field label={t('Sign-in')} tour="custom.auth">
  <select value={value.auth.type} onchange={(e) => setAuth(e.currentTarget.value as CustomAuth['type'])}>
    {#each AUTHS as a (a.value)}<option value={a.value}>{a.label}</option>{/each}
  </select>
</Field>
{#if value.auth.type === 'header'}
  <Field label={t('Header name')}><input type="text" bind:value={value.auth.name} /></Field>
{:else if value.auth.type === 'basic'}
  <Field label={t('User name')}><input type="text" bind:value={value.auth.user} /></Field>
{:else if value.auth.type === 'query'}
  <Field label={t('Query parameter')}><input type="text" bind:value={value.auth.param} /></Field>
{:else if value.auth.type === 'oauth2'}
  {@const a = value.auth}
  <Field label={t('Sign-in address')} hint={t('The authorization endpoint of your identity provider.')}>
    <input type="text" bind:value={a.auth_url} placeholder="https://login.example.com/oauth2/authorize" />
  </Field>
  <Field label={t('Token address')} hint={t('Its token endpoint.')}>
    <input type="text" bind:value={a.token_url} placeholder="https://login.example.com/oauth2/token" />
  </Field>
  <Field label={t('Client ID')} hint={t('Of the app registered for CargoFrame with your identity provider. A client secret, if it gave one, goes below once saved.')}>
    <input type="text" bind:value={a.client_id} />
  </Field>
  <Field label={t('Scopes')} hint={t('Separated by spaces, as your identity provider names them.')}>
    <input type="text" bind:value={a.scopes} placeholder="openid tickets.write" />
  </Field>
  <Field label={t('Redirect port')} hint={t('The app\'s redirect URL must be {address}', { address: `http://127.0.0.1:${a.redirect_port}/callback` })}>
    <input type="text" inputmode="numeric" value={String(a.redirect_port)}
      oninput={(e) => (a.redirect_port = Number(e.currentTarget.value) || 0)} />
  </Field>
{:else if value.auth.type === 'client_credentials'}
  {@const a = value.auth}
  <Field label={t('Token address')} hint={t('The token endpoint of your identity provider.')}>
    <input type="text" bind:value={a.token_url} placeholder="https://login.example.com/oauth2/token" />
  </Field>
  <Field label={t('Client ID')} hint={t('Of the service account. Its secret goes below once saved.')}>
    <input type="text" bind:value={a.client_id} />
  </Field>
  <Field label={t('Scopes')} hint={t('Separated by spaces. Empty: none asked for.')}>
    <input type="text" bind:value={a.scopes} placeholder="api://tickets/.default" />
  </Field>
{/if}

<fieldset>
  <legend>{t('What a share sends')}</legend>
  <div class="line">
    <Field label={t('Method')}>
      <select bind:value={value.send.method}>
        <option>POST</option><option>PUT</option><option>PATCH</option>
      </select>
    </Field>
    <Field label={t('Path')} hint={t('May use {{field}}, {{prompt}} or {{title}}.')}>
      <input type="text" bind:value={value.send.path} placeholder="/messages" />
    </Field>
  </div>
  <PairsEditor bind:pairs={value.send.headers} label={t('Extra headers')} nameLabel={t('Header')} addLabel={t('Add a header')} />
  <Field label={t('Body')}>
    <select value={value.send.body.type} onchange={(e) => setBody(e.currentTarget.value as CustomBody['type'])}>
      {#each BODIES as b (b.value)}<option value={b.value}>{b.label}</option>{/each}
    </select>
  </Field>
  {#if value.send.body.type === 'json'}
    <Field label="JSON" hint={t('Placeholders are filled with the values of the sharing button\'s fields, safely escaped.')}>
      <textarea rows="6" class="mono" bind:value={value.send.body.template} spellcheck="false"></textarea>
    </Field>
  {:else if value.send.body.type === 'form'}
    <PairsEditor bind:pairs={value.send.body.fields} label={t('Form fields')} addLabel={t('Add a field')} />
  {:else if value.send.body.type === 'multipart'}
    <PairsEditor bind:pairs={value.send.body.fields} label={t('Form fields')} addLabel={t('Add a field')} />
    <div class="line">
      <Field label={t('Screenshot field')}><input type="text" bind:value={value.send.body.image_field} /></Field>
      <Field label={t('Files (zip) field')} hint={t('Leave empty to leave the files out.')}>
        <input type="text" bind:value={value.send.body.files_field} />
      </Field>
    </div>
  {/if}
  <Field label={t('Address of the result')} hint={t('Where the answer names what was created, for example data.url. It is shown as a link.')}>
    <input type="text" bind:value={value.result_url_path} placeholder="data.url" />
  </Field>
</fieldset>

<Field label={t('Test address')} hint={t('A page CargoFrame can GET to check the sign-in. Leave empty to skip the test.')}>
  <input type="text" bind:value={value.test_path} placeholder="/me" />
</Field>

<fieldset>
  <legend>{t('Lists for dropdowns')}</legend>
  <p class="quiet">{t('A list is read with a GET. Point at the array in the answer and at each item\'s value and label.')}</p>
  {#each value.lists as l, i (i)}
    <div class="list">
      <input type="text" aria-label={t('List {n} name', { n: i + 1 })} placeholder={t('name (channels)')} bind:value={l.id} />
      <input type="text" aria-label={t('List {n} title', { n: i + 1 })} placeholder="title" bind:value={l.label} />
      <input type="text" aria-label={t('List {n} path', { n: i + 1 })} placeholder="/channels" bind:value={l.path} />
      <input type="text" aria-label={t('List {n} array', { n: i + 1 })} placeholder="data.items" bind:value={l.items_path} />
      <input type="text" aria-label={t('List {n} value', { n: i + 1 })} placeholder="id" bind:value={l.value_path} />
      <input type="text" aria-label={t('List {n} label', { n: i + 1 })} placeholder="name" bind:value={l.label_path} />
      <button type="button" aria-label={t('Remove list {n}', { n: i + 1 })} onclick={() => value.lists.splice(i, 1)}>{t('Remove')}</button>
    </div>
  {/each}
  <div>
    <button
      type="button"
      onclick={() => value.lists.push({ id: '', label: '', path: '', items_path: '', value_path: '', label_path: '' })}
      >{t('Add a list')}</button
    >
  </div>
</fieldset>

<style>
  fieldset { border: 1px solid var(--border); border-radius: var(--radius); padding: 12px; display: grid; gap: 12px; margin: 0; }
  legend { font-weight: 600; font-size: 13px; padding: 0 6px; }
  .line { display: grid; grid-template-columns: 1fr 2fr; gap: 10px; align-items: start; }
  .list { display: grid; grid-template-columns: repeat(3, 1fr) auto; gap: 6px; }
  .list input:nth-child(n + 4):nth-child(-n + 6) { grid-row: 2; }
</style>
