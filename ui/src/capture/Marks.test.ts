import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import Marks from './Marks.svelte';
import type { BlockMarks } from './willSend';

const none: BlockMarks = { marks: [], notSent: false, notSentWhy: null };

describe('Marks', () => {
  afterEach(cleanup);

  it('shows nothing when there is nothing to say', () => {
    render(Marks, { marks: none });
    expect(screen.queryByRole('list')).toBeNull();
  });

  it('lists the buttons that send the block, in their destination colours', () => {
    render(Marks, {
      marks: {
        marks: [
          { buttonId: 'a', label: 'Create ticket', kind: 'jira', state: 'sent', why: null },
          { buttonId: 'b', label: 'Ask Claude', kind: 'claude', state: 'sent', why: null },
        ],
        notSent: false, notSentWhy: null,
      },
    });
    const items = screen.getAllByRole('listitem');
    expect(items.map((i) => i.textContent!.replace(/\s+/g, ' ').trim())).toEqual(['✓ Create ticket', '✓ Ask Claude']);
    expect(items[0]!.getAttribute('style')).toContain('--kind-jira');
    expect(items[1]!.getAttribute('style')).toContain('--kind-claude');
  });

  it('says why a button cannot send the block', () => {
    render(Marks, {
      marks: {
        marks: [{ buttonId: 'a', label: 'Post', kind: 'reddit', state: 'cannot_send', why: 'Reddit cannot take files on a post' }],
        notSent: true, notSentWhy: 'Post cannot send it: Reddit cannot take files on a post',
      },
    });
    expect(screen.getByText(/Post/).textContent).toContain('✕');
    expect(screen.getByText(/Reddit cannot take files on a post/)).toBeTruthy();
    expect(screen.getByText('Not sent', { exact: false })).toBeTruthy();
  });

  it('gives one plain "Not sent" when no button sends the block', () => {
    render(Marks, { marks: { marks: [], notSent: true, notSentWhy: 'Share has no field for it' } });
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(1);
    expect(items[0]!.textContent).toContain('Not sent');
    expect(items[0]!.title).toBe('Share has no field for it');
  });
});
