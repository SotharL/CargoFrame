<script lang="ts">
  // An item the organisation set (its managed folder): shown, never changed or removed here.
  import Chip from './Chip.svelte';
  import { t } from '../i18n/index.svelte';
</script>

<span class="managed" title={t('Your organisation set this for everyone. Ask your IT department to change it.')}>
  <Chip kind="dbg" label={t('Set by your organisation')} />
</span>
