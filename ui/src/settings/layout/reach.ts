import type { Block, BlockReach, Capability, Support } from '../../lib/types';
import { t, msg, translated } from '../../lib/i18n/index.svelte';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** One line about what a button does with one block, in words for the person. */
export interface Line {
  state: BlockReach['state'];
  /** ✓ sent, ✕ cannot be sent, – not sent. Colour is never the only signal. */
  icon: '✓' | '✕' | '–';
  text: string;
}

export function blockName(b: Block): string {
  if (b.type === 'screenshot') return t('Screenshot');
  if (b.type === 'video') return t('Video');
  if (b.type === 'tabs') return t('Tabs');
  return b.type === 'sharing_button' ? b.label || t('Share') : b.title || b.id;
}

/** What the button does with the block `r` describes. */
export function lineFor(r: BlockReach, button: SharingBlock, kind: string | null): Line {
  switch (r.state) {
    case 'sent':
      if (r.role === 'screenshot') return { state: 'sent', icon: '✓', text: t('Sent as the screenshot') };
      if (r.role === 'video') return { state: 'sent', icon: '✓', text: t('Sent as the video') };
      if (r.role === 'prompt') return { state: 'sent', icon: '✓', text: t('Sent as the prompt') };
      if (r.role === 'zip_name') return { state: 'sent', icon: '✓', text: t('Names the saved zip') };
      if (r.role === 'files') {
        const asText = kind === 'claude' && button.send?.files === 'inline';
        return { state: 'sent', icon: '✓', text: asText ? t('Text files go inside the message, the rest are attached') : t('Sent as files') };
      }
      if (kind === 'claude' && r.field === 'chat') return { state: 'sent', icon: '✓', text: t('Picks the chat to open') };
      return { state: 'sent', icon: '✓', text: t('Sent in the “{field}” field', { field: r.field }) };
    case 'cannot_send':
      return { state: 'cannot_send', icon: '✕', text: t('Cannot be sent: {why}', { why: r.why }) };
    case 'switched_off':
      return { state: 'switched_off', icon: '–', text: t('Left out by the send rules below') };
    default:
      return { state: 'not_used', icon: '–', text: t('Not used by this button') };
  }
}

export const SUPPORT_ICON: Record<Support, string> = { yes: '✓', limited: '◐', no: '✕' };
export const SUPPORT_WORD: Record<Support, string> = translated({ yes: msg('Yes'), limited: msg('With a catch'), no: msg('No') }, 'all');

export const CAPABILITY_ROWS: { key: 'text' | 'dropdown' | 'screenshot' | 'video' | 'files' | 'link'; label: string }[] = translated([
  { key: 'text', label: msg('Text') },
  { key: 'dropdown', label: msg('Choices from lists') },
  { key: 'screenshot', label: msg('Screenshot') },
  { key: 'video', label: msg('Video') },
  { key: 'files', label: msg('Files') },
  { key: 'link', label: msg('Links in text') },
]);

export type CapabilityOf = Capability;
