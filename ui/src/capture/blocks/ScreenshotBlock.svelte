<script lang="ts">
  import Editor from '../editor/Editor.svelte';
  import { afterPaint } from '../../lib/paint';
  import type { TextReading } from '../../lib/types';

  let {
    original,
    size,
    edited = $bindable(false),
    onpainted,
    reader,
  }: {
    original: Blob;
    size: { width: number; height: number };
    edited?: boolean;
    /** Called once the picture is on screen. */
    onpainted?: () => void;
    /** Reads the text in a part of the picture (the "Read text" tool). */
    reader?: (png: Blob, attempt: number) => Promise<TextReading>;
  } = $props();

  let bitmap = $state<ImageBitmap | null>(null);
  let failed = $state<string | null>(null);
  let editor = $state<ReturnType<typeof Editor>>();

  $effect(() => {
    let cancelled = false;
    bitmap = null;
    failed = null;
    createImageBitmap(original).then(
      (b) => { if (!cancelled) bitmap = b; else b.close(); },
      (e) => { if (!cancelled) failed = String(e); },
    );
    return () => { cancelled = true; };
  });

  $effect(() => {
    if (bitmap) afterPaint(() => onpainted?.());
  });

  /** The picture to send, with the person's edits. */
  export function exportImage(): Promise<Blob> {
    return editor ? editor.exportImage() : Promise.resolve(original);
  }
</script>

<div class="shot" data-content>
  {#if bitmap}
    <Editor bind:this={editor} image={bitmap} {original} {size} bind:edited {reader} />
  {:else if failed}
    <p class="quiet">The screenshot could not be shown: {failed}</p>
  {:else}
    <p class="quiet">Loading the screenshot…</p>
  {/if}
</div>

<style>
  .shot { height: 100%; min-height: 0; }
</style>
