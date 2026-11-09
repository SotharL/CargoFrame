// Stand-in for the backend's capability table and reach, used only by the in-memory interface
// (tests and the browser demo). The real answers come from the Rust code through `button_reach`,
// so this does not need to be complete, only plausible and consistent with the types.
import type { Block, Capabilities, Capability, Layout, Reach, ServiceKind, BlockReach, SendRules } from './types';
import { DEFAULT_SEND_RULES } from './types';

const yes = (note: string): Capability => ({ support: 'yes', note });
const limited = (note: string): Capability => ({ support: 'limited', note });
const no = (note: string): Capability => ({ support: 'no', note });

export function demoCapabilities(kind: ServiceKind): Capabilities {
  switch (kind) {
    case 'jira':
      return {
        text: yes('Becomes the issue summary and description'),
        dropdown: yes('Priority, labels, components and more come from your Jira'),
        screenshot: yes('Attached to the issue'),
        video: limited("Attached to the issue, within your Jira's attachment size limit"),
        files: yes('Attached to the issue as one zip'),
        link: yes('Links use Jira wiki style: [word|https://…]'),
      };
    case 'reddit':
      return {
        text: yes('Becomes the post title and text'),
        dropdown: yes('The subreddit and flair come from your Reddit'),
        screenshot: yes('Posted as an image post'),
        video: no('Reddit video posts are not supported yet'),
        files: no('Reddit cannot take files on a post'),
        link: yes('The post text understands Markdown links'),
      };
    case 'google':
      return {
        text: limited('Goes into the spreadsheet columns you map, as plain text'),
        dropdown: limited('Only the Drive folder has a list to pick from'),
        screenshot: yes('Uploaded to Drive, with a link in the row'),
        video: yes('Uploaded to Drive, with a link in the row'),
        files: yes('Uploaded to Drive as one zip, with a link in the row'),
        link: limited('Addresses stay plain text in a cell'),
      };
    case 'claude':
      return {
        text: yes('The prompt and title are typed into the chat'),
        dropdown: limited('Only the chat to open can be picked'),
        screenshot: yes('Pasted into the chat as a picture'),
        video: no('Claude cannot watch a video'),
        files: yes('Pasted into the chat as files (20 at most, 30 MB each), or as text'),
        link: yes('Addresses are typed into the message as written'),
      };
    case 'slack':
      return {
        text: yes('Posted as the message (or the comment on the files)'),
        dropdown: limited('Only the channel has a list to pick from'),
        screenshot: yes('Uploaded to the channel as a picture'),
        video: yes('Uploaded to the channel as a video'),
        files: yes('Uploaded to the channel as one zip'),
        link: yes('Addresses turn into links by themselves; <, > and & are shown as typed'),
      };
    case 'discord':
      return {
        text: yes("Posted as the message, up to 2000 characters (a forum post's title is the name)"),
        dropdown: limited('The channel (or forum) and an existing thread can be picked'),
        screenshot: yes('Attached to the message'),
        video: limited("Attached to the message, within the server's upload limit (often 10 MB)"),
        files: limited("Attached as one zip, within the server's upload limit"),
        link: yes('Addresses and [word](https://…) links are shown as links'),
      };
    case 'teams':
      return {
        text: yes('Posted as the message (a new channel post uses the title as its headline)'),
        dropdown: limited('Your chats and channels can be picked; a thread is chosen by its link'),
        screenshot: yes('Shown inside the message'),
        video: no('Teams needs files in OneDrive, which CargoFrame does not use yet'),
        files: no('Teams needs files in OneDrive, which CargoFrame does not use yet'),
        link: yes('Addresses are shown as typed and Teams links them by itself; markup is shown as text'),
      };
    case 'github':
      return {
        text: yes("The issue's title and description, in GitHub's Markdown"),
        dropdown: yes('The repository, labels, people, milestone and an open issue come from GitHub'),
        screenshot: yes("Put in the repository's attachments branch and shown in the issue"),
        video: limited('Put in the attachments branch and linked from the issue, up to 100 MB'),
        files: limited('One zip in the attachments branch, linked from the issue, up to 100 MB'),
        link: yes('Addresses and [word](https://…) links are shown as links'),
      };
    case 'x':
    case 'bluesky':
    case 'instagram': {
      const name = { x: 'X', bluesky: 'Bluesky', instagram: 'Instagram' }[kind];
      return {
        text: limited('Posted as plain text, within the length the service allows'),
        dropdown: no(`${name} has no lists to pick from`),
        screenshot: yes('Attached to the post, made smaller when it is too big'),
        video: no(`Videos are not sent to ${name} yet; the screenshot is`),
        files: no(`${name} cannot take files`),
        link: kind === 'instagram' ? limited('Addresses are shown as text: Instagram captions have no links') : yes('Addresses become links'),
      };
    }
    default:
      return {
        text: yes('Placed in the request where you write {{prompt}} or {{field}}'),
        dropdown: no('This connection has no lists to pick from'),
        screenshot: no('The request has no multipart field for the screenshot'),
        video: no('A Custom API connection cannot send a video yet'),
        files: no('The request has no multipart field for files'),
        link: limited('Sent as written; the service decides what it shows'),
      };
  }
}

export const LOCAL_CAPABILITIES: Capabilities = {
  text: yes('Kept in share.json inside the zip'),
  dropdown: yes('The choices are kept in share.json inside the zip'),
  screenshot: yes('Kept as screenshot.png inside the zip'),
  video: yes('Kept as recording.mp4 inside the zip'),
  files: yes("Kept in the zip's files folder"),
  link: yes('Kept as written'),
};

/** What `button` does with each other block, given what its destination can take. */
export function demoReach(layout: Layout, button: Extract<Block, { type: 'sharing_button' }>, caps: Capabilities, kind: ServiceKind | null): Reach {
  const rules: SendRules = { ...DEFAULT_SEND_RULES, ...button.send };
  const mappings = [...button.mappings];
  if (kind === 'claude' && !mappings.some((m) => m.field === 'chat')) {
    const chats = layout.blocks.find((b) => b.type === 'dropdown' && b.source.type === 'claude_chats');
    if (chats) mappings.push({ field: 'chat', source: { kind: 'block', value: chats.id } });
  }
  // What the demo destinations cannot work without: an AI chat's prompt, a Jira issue's summary.
  const needs = (role: 'prompt' | 'field', field?: string) => (kind === 'claude' && role === 'prompt') || (kind === 'jira' && field === 'summary');
  const fieldOf = (id: string) => mappings.find((m) => m.source.kind === 'block' && m.source.value === id)?.field;
  const blocks: BlockReach[] = [];
  for (const b of layout.blocks) {
    if (b.type === 'sharing_button') continue;
    if (b.type === 'screenshot') {
      blocks.push(!rules.screenshot ? { block_id: b.id, state: 'switched_off' } : caps.screenshot.support === 'no' ? { block_id: b.id, state: 'cannot_send', why: caps.screenshot.note } : { block_id: b.id, state: 'sent', role: 'screenshot' });
    } else if (b.type === 'video') {
      blocks.push(!rules.screenshot ? { block_id: b.id, state: 'switched_off' } : caps.video.support === 'no' ? { block_id: b.id, state: 'cannot_send', why: caps.video.note } : { block_id: b.id, state: 'sent', role: 'video' });
    } else if (b.type === 'files') {
      blocks.push(rules.files === 'none' ? { block_id: b.id, state: 'switched_off' } : caps.files.support === 'no' ? { block_id: b.id, state: 'cannot_send', why: caps.files.note } : { block_id: b.id, state: 'sent', role: 'files' });
    } else if (b.type === 'text') {
      const field = fieldOf(b.id);
      const sentAs = button.prompt_block_id === b.id ? ({ role: 'prompt' } as const) : field ? ({ role: 'field', field } as const) : null;
      blocks.push(!sentAs ? { block_id: b.id, state: 'not_used' } : caps.text.support === 'no' ? { block_id: b.id, state: 'cannot_send', why: caps.text.note } : { block_id: b.id, state: 'sent', ...sentAs, required: needs(sentAs.role, field) });
    } else {
      const field = fieldOf(b.id);
      blocks.push(!field ? { block_id: b.id, state: 'not_used' } : caps.dropdown.support === 'no' ? { block_id: b.id, state: 'cannot_send', why: caps.dropdown.note } : { block_id: b.id, state: 'sent', role: 'field', field });
    }
  }
  return { capabilities: caps, blocks };
}
