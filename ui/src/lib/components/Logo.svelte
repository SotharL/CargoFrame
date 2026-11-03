<script lang="ts">
  // The CargoFrame mark: three frames moving right, the lead one carrying a forward arrow.
  // The frames follow the theme's text colour and the arrow the brand accent, so the mark reads
  // in both themes. Decorative: the name next to it says what it is.
  let { size = 24 }: { size?: number } = $props();
</script>

<svg viewBox="0 0 64 64" width={size} height={size} aria-hidden="true" focusable="false">
  <defs>
    <mask id="cf-mask-a" maskUnits="userSpaceOnUse" x="-8" y="-8" width="80" height="80">
      <rect x="-8" y="-8" width="80" height="80" fill="#fff" />
      <rect x="13" y="15" width="34" height="34" rx="10" />
      <rect x="27" y="15" width="34" height="34" rx="10" />
    </mask>
    <mask id="cf-mask-b" maskUnits="userSpaceOnUse" x="-8" y="-8" width="80" height="80">
      <rect x="-8" y="-8" width="80" height="80" fill="#fff" />
      <rect x="27" y="15" width="34" height="34" rx="10" />
    </mask>
  </defs>
  <g class="frames">
    <rect x="3" y="17" width="30" height="30" rx="8" stroke-opacity=".28" mask="url(#cf-mask-a)" />
    <rect x="15" y="17" width="30" height="30" rx="8" stroke-opacity=".6" mask="url(#cf-mask-b)" />
    <rect x="29" y="17" width="30" height="30" rx="8" />
  </g>
  <path class="arrow" d="M41 26l7 6-7 6" />
</svg>

<style>
  svg { display: block; flex: none; }
  .frames { fill: none; stroke: var(--text); stroke-width: 4; stroke-linejoin: round; }
  .arrow {
    fill: none;
    stroke: var(--brand-accent);
    stroke-width: 4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
