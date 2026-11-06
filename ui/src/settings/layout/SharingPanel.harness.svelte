<script lang="ts">
  // Test-only: holds the button as state the way the layout page does, so edits show up.
  import SharingPanel from './SharingPanel.svelte';
  import type { Block, Capabilities, Reach, ServiceKind } from '../../lib/types';

  type Button = Extract<Block, { type: 'sharing_button' }>;
  let {
    initial,
    blocks,
    kind,
    reach,
    fallback = null,
    fields = [],
    onchange,
  }: {
    initial: Button;
    blocks: Block[];
    kind: ServiceKind | null;
    reach: Reach | undefined;
    fallback?: Capabilities | null;
    fields?: { key: string; label: string }[];
    onchange?: (b: Button) => void;
  } = $props();
  let block = $state<Button>(untrackedCopy());
  function untrackedCopy(): Button {
    return structuredClone($state.snapshot(initial)) as Button;
  }
  $effect(() => onchange?.($state.snapshot(block) as Button));
</script>

<SharingPanel {block} {blocks} {kind} {reach} {fallback} {fields} />
