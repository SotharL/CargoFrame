import type { Block } from './types';

type Text = Extract<Block, { type: 'text' }>;

/** The small label a text block wears for its role; plain text wears none. */
export function roleTag(role: Text['role']): string {
  switch (role) {
    case 'prompt':
      return 'Prompt';
    case 'zip_name':
      return 'Zip name';
    default:
      return '';
  }
}

/** What the Role field in the builder says about each role. */
export function roleHint(role: Text['role']): string {
  switch (role) {
    case 'prompt':
      return 'The text sent to an AI chat. A sharing button to Claude needs one.';
    case 'zip_name':
      return 'The name of the zip saved on this computer (when a button saves locally, or a send fails and a copy is kept). Only the first one in the layout is used; empty keeps the dated name.';
    default:
      return 'Plain notes that are sent along with the capture.';
  }
}
