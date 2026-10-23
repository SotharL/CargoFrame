<script lang="ts">
  import Chip from '../../lib/components/Chip.svelte';
  let { title }: { title: string } = $props();
</script>

<div class="empty">
  <Chip kind="dbg" label="Not built yet" />
  <p>The <strong>{title}</strong> page arrives in a later phase of the plan.</p>
</div>

<style>
  .empty {
    display: grid; gap: 10px; justify-items: start; padding: 20px;
    border: 1px dashed var(--border-strong); border-radius: var(--radius);
  }
</style>
