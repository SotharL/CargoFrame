<script lang="ts">
  // A translated sentence with marked-up parts: each {name} in `text` is drawn by the snippet of
  // that name (a link, a code word), so a translation can move them.
  //   <Tr text={t('Open {site} and choose {what}.')}>
  //     {#snippet site()}<a href="https://api.slack.com/apps">api.slack.com/apps</a>{/snippet}
  //     {#snippet what()}<b>Create New App</b>{/snippet}
  //   </Tr>
  import type { Snippet } from 'svelte';

  let { text, ...parts }: { text: string; [name: string]: Snippet | string } = $props();

  const pieces = $derived(text.split(/(\{[A-Za-z0-9_]+\})/).filter((p) => p !== ''));
  const snippetOf = (piece: string): Snippet | null => {
    const m = /^\{([A-Za-z0-9_]+)\}$/.exec(piece);
    const s = m ? parts[m[1]!] : undefined;
    return typeof s === 'function' ? (s as Snippet) : null;
  };
</script>

{#each pieces as piece, i (i)}{@const s = snippetOf(piece)}{#if s}{@render s()}{:else}{piece}{/if}{/each}
