import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import Toggle from './Toggle.svelte';

afterEach(cleanup);

describe('Toggle', () => {
  it('reports the new value when clicked', async () => {
    let seen: boolean | undefined;
    render(Toggle, { label: 'Run at sign-in', checked: false, onchange: (v: boolean) => (seen = v) });
    await fireEvent.click(screen.getByLabelText('Run at sign-in'));
    expect(seen).toBe(true);
  });
});
