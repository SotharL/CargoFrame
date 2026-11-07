<script lang="ts">
  // A small turning ring for "this is being looked at". It stands still for people who ask their
  // computer for less motion, and says what it is waiting for to a screen reader.
  let { label = 'Working' }: { label?: string } = $props();
</script>

<span class="spinner" role="img" aria-label={label}></span>

<style>
  .spinner {
    display: inline-block; width: 16px; height: 16px; box-sizing: border-box; flex: none;
    border: 2px solid color-mix(in srgb, var(--quiet) 35%, transparent); border-top-color: var(--accent);
    border-radius: 50%; animation: turn 0.8s linear infinite;
  }
  @keyframes turn { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; border-top-color: var(--quiet); }
  }
</style>
