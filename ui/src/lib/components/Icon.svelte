<script lang="ts">
  import { ICONS, type IconName } from '../icons';

  /** A line icon from lib/icons.ts. It is decoration: the button around it carries the name. */
  let { name, size = 16 }: { name: IconName; size?: number } = $props();
</script>

<svg class="icon" viewBox="0 0 24 24" width={size} height={size} aria-hidden="true"><path d={ICONS[name]} /></svg>

<style>
  .icon { display: block; flex: none; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
</style>
