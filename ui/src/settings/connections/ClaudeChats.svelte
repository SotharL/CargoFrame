<script lang="ts">
  import { onMount } from 'svelte';
  import Chip from '../../lib/components/Chip.svelte';
  import { getApi } from '../../lib/ipc';
  import type { ChatView } from '../../lib/types';

  let chats = $state<ChatView[]>([]);
  let link = $state('');
  let title = $state('');
  let error = $state('');

  async function reload() {
    try {
      chats = await (await getApi()).listChats();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(reload);

  async function act(f: (api: Awaited<ReturnType<typeof getApi>>) => Promise<void>) {
    error = '';
    try {
      await f(await getApi());
      await reload();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="chats">
  <h3>Claude chats</h3>
  <p class="quiet">
    Claude cannot list your chats, so add the ones you share to often: open the chat, copy its
    address, and paste it here. They appear in the chat dropdown.
  </p>
  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      void act(async (api) => {
        await api.pinChat(link, title);
        link = '';
        title = '';
      });
    }}
  >
    <input type="text" aria-label="Chat address" placeholder="https://claude.ai/chat/…" bind:value={link} />
    <input type="text" aria-label="Chat name" placeholder="A name for it" bind:value={title} />
    <button class="primary" type="submit" disabled={!link.trim()}>Pin chat</button>
  </form>
  {#if error}<Chip kind="err" label={error} />{/if}
  {#if chats.length === 0}
    <p class="quiet">No chats yet. “New chat” is always offered.</p>
  {/if}
  <ul>
    {#each chats as c (c.id)}
      <li>
        <span class="name">{c.title}</span>
        <span class="quiet">{c.pinned ? 'Pinned' : 'Recent'}</span>
        <button onclick={() => act((api) => api.setChatPinned(c.id, !c.pinned))}>{c.pinned ? 'Unpin' : 'Pin'}</button>
        <button onclick={() => act((api) => api.removeChat(c.id))}>Remove</button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .chats { display: grid; gap: 8px; }
  .add { display: grid; grid-template-columns: 2fr 1fr auto; gap: 6px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  li { display: flex; gap: 8px; align-items: center; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
