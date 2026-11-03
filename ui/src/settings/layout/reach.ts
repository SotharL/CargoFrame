import type { Block, BlockReach, Capability, Support } from '../../lib/types';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** One line about what a button does with one block, in words for the person. */
export interface Line {
  state: BlockReach['state'];
  /** ✓ sent, ✕ cannot be sent, – not sent. Colour is never the only signal. */
  icon: '✓' | '✕' | '–';
  text: string;
}

export function blockName(b: Block): string {
  if (b.type === 'screenshot') return 'Screenshot';
  if (b.type === 'video') return 'Video';
  return b.type === 'sharing_button' ? b.label || 'Share' : b.title || b.id;
}

/** What the button does with the block `r` describes. */
export function lineFor(r: BlockReach, button: SharingBlock, kind: string | null): Line {
  switch (r.state) {
    case 'sent':
      if (r.role === 'screenshot') return { state: 'sent', icon: '✓', text: 'Sent as the screenshot' };
      if (r.role === 'video') return { state: 'sent', icon: '✓', text: 'Sent as the video' };
      if (r.role === 'prompt') return { state: 'sent', icon: '✓', text: 'Sent as the prompt' };
      if (r.role === 'zip_name') return { state: 'sent', icon: '✓', text: 'Names the saved zip' };
      if (r.role === 'files') {
        const asText = kind === 'claude' && button.send?.files === 'inline';
        return { state: 'sent', icon: '✓', text: asText ? 'Text files go inside the message, the rest are attached' : 'Sent as files' };
      }
      if (kind === 'claude' && r.field === 'chat') return { state: 'sent', icon: '✓', text: 'Picks the chat to open' };
      return { state: 'sent', icon: '✓', text: `Sent in the “${r.field}” field` };
    case 'cannot_send':
      return { state: 'cannot_send', icon: '✕', text: `Cannot be sent: ${r.why}` };
    case 'switched_off':
      return { state: 'switched_off', icon: '–', text: 'Left out by the send rules below' };
    default:
      return { state: 'not_used', icon: '–', text: 'Not used by this button' };
  }
}

export const SUPPORT_ICON: Record<Support, string> = { yes: '✓', limited: '◐', no: '✕' };
export const SUPPORT_WORD: Record<Support, string> = { yes: 'Yes', limited: 'With a catch', no: 'No' };

export const CAPABILITY_ROWS: { key: 'text' | 'dropdown' | 'screenshot' | 'video' | 'files' | 'link'; label: string }[] = [
  { key: 'text', label: 'Text' },
  { key: 'dropdown', label: 'Choices from lists' },
  { key: 'screenshot', label: 'Screenshot' },
  { key: 'video', label: 'Video' },
  { key: 'files', label: 'Files' },
  { key: 'link', label: 'Links in text' },
];

export type CapabilityOf = Capability;
