<script lang="ts">
  import Field from '../../lib/components/Field.svelte';
  import PairsEditor from './PairsEditor.svelte';
  import type { Connection, CustomAuth, CustomBody } from '../../lib/types';

  type Custom = Extract<Connection, { kind: 'custom' }>;
  let { value = $bindable() }: { value: Custom } = $props();

  const AUTHS: { value: CustomAuth['type']; label: string }[] = [
    { value: 'none', label: 'No sign-in' },
    { value: 'bearer', label: 'Bearer token (Authorization header)' },
    { value: 'header', label: 'A header with the key' },
    { value: 'basic', label: 'User name and password (basic)' },
    { value: 'query', label: 'A query parameter with the key' },
  ];
  const BODIES: { value: CustomBody['type']; label: string }[] = [
    { value: 'none', label: 'No body' },
    { value: 'json', label: 'JSON' },
    { value: 'form', label: 'Form fields' },
    { value: 'multipart', label: 'Form with the screenshot and files' },
  ];

  function setAuth(type: CustomAuth['type']) {
    value.auth =
      type === 'header' ? { type, name: 'X-Api-Key' }
      : type === 'basic' ? { type, user: '' }
      : type === 'query' ? { type, param: 'key' }
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

<Field label="Base address" hint="Every request goes under this address. It must start with https://.">
  <input type="text" bind:value={value.base_url} placeholder="https://api.example.com/v1" />
</Field>

<Field label="Sign-in">
  <select value={value.auth.type} onchange={(e) => setAuth(e.currentTarget.value as CustomAuth['type'])}>
    {#each AUTHS as a (a.value)}<option value={a.value}>{a.label}</option>{/each}
  </select>
</Field>
{#if value.auth.type === 'header'}
  <Field label="Header name"><input type="text" bind:value={value.auth.name} /></Field>
{:else if value.auth.type === 'basic'}
  <Field label="User name"><input type="text" bind:value={value.auth.user} /></Field>
{:else if value.auth.type === 'query'}
  <Field label="Query parameter"><input type="text" bind:value={value.auth.param} /></Field>
{/if}

<fieldset>
  <legend>What a share sends</legend>
  <div class="line">
    <Field label="Method">
      <select bind:value={value.send.method}>
        <option>POST</option><option>PUT</option><option>PATCH</option>
      </select>
    </Field>
    <Field label="Path" hint="May use {'{{field}}'}, {'{{prompt}}'} or {'{{title}}'}.">
      <input type="text" bind:value={value.send.path} placeholder="/messages" />
    </Field>
  </div>
  <PairsEditor bind:pairs={value.send.headers} label="Extra headers" nameLabel="Header" addLabel="Add a header" />
  <Field label="Body">
    <select value={value.send.body.type} onchange={(e) => setBody(e.currentTarget.value as CustomBody['type'])}>
      {#each BODIES as b (b.value)}<option value={b.value}>{b.label}</option>{/each}
    </select>
  </Field>
  {#if value.send.body.type === 'json'}
    <Field label="JSON" hint="Placeholders are filled with the values of the sharing button's fields, safely escaped.">
      <textarea rows="6" class="mono" bind:value={value.send.body.template} spellcheck="false"></textarea>
    </Field>
  {:else if value.send.body.type === 'form'}
    <PairsEditor bind:pairs={value.send.body.fields} label="Form fields" addLabel="Add a field" />
  {:else if value.send.body.type === 'multipart'}
    <PairsEditor bind:pairs={value.send.body.fields} label="Form fields" addLabel="Add a field" />
    <div class="line">
      <Field label="Screenshot field"><input type="text" bind:value={value.send.body.image_field} /></Field>
      <Field label="Files (zip) field" hint="Leave empty to leave the files out.">
        <input type="text" bind:value={value.send.body.files_field} />
      </Field>
    </div>
  {/if}
  <Field label="Address of the result" hint="Where the answer names what was created, for example data.url. It is shown as a link.">
    <input type="text" bind:value={value.result_url_path} placeholder="data.url" />
  </Field>
</fieldset>

<Field label="Test address" hint="A page CargoFrame can GET to check the sign-in. Leave empty to skip the test.">
  <input type="text" bind:value={value.test_path} placeholder="/me" />
</Field>

<fieldset>
  <legend>Lists for dropdowns</legend>
  <p class="quiet">A list is read with a GET. Point at the array in the answer and at each item's value and label.</p>
  {#each value.lists as l, i (i)}
    <div class="list">
      <input type="text" aria-label="List {i + 1} name" placeholder="name (channels)" bind:value={l.id} />
      <input type="text" aria-label="List {i + 1} title" placeholder="title" bind:value={l.label} />
      <input type="text" aria-label="List {i + 1} path" placeholder="/channels" bind:value={l.path} />
      <input type="text" aria-label="List {i + 1} array" placeholder="data.items" bind:value={l.items_path} />
      <input type="text" aria-label="List {i + 1} value" placeholder="id" bind:value={l.value_path} />
      <input type="text" aria-label="List {i + 1} label" placeholder="name" bind:value={l.label_path} />
      <button type="button" aria-label="Remove list {i + 1}" onclick={() => value.lists.splice(i, 1)}>Remove</button>
    </div>
  {/each}
  <div>
    <button
      type="button"
      onclick={() => value.lists.push({ id: '', label: '', path: '', items_path: '', value_path: '', label_path: '' })}
      >Add a list</button
    >
  </div>
</fieldset>

<style>
  fieldset { border: 1px solid var(--border); border-radius: var(--radius); padding: 12px; display: grid; gap: 12px; margin: 0; }
  legend { font-weight: 600; font-size: 13px; padding: 0 6px; }
  .line { display: grid; grid-template-columns: 1fr 2fr; gap: 10px; }
  .list { display: grid; grid-template-columns: repeat(3, 1fr) auto; gap: 6px; }
  .list input:nth-child(n + 4):nth-child(-n + 6) { grid-row: 2; }
</style>
