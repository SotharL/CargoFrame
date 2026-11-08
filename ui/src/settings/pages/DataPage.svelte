<script lang="ts">
  // Everything about what CargoFrame keeps: what is on this computer and a way to clear it, then a
  // backup of layouts and values that can go to another computer.
  import YourData from './YourData.svelte';
  import TransferSection from './TransferSection.svelte';
</script>

<div class="page">
  <YourData />
  <TransferSection />
</div>

<style>
  .page { display: grid; gap: 14px; }
</style>
