import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import StylePicker from './StylePicker.svelte';

afterEach(cleanup);

const props = { colors: ['#e04f4f', '#4f8ee0', '#ffffff'], thicknesses: ['thin', 'medium', 'thick'] as const, color: '#e04f4f', thickness: 'medium' as const };

describe('StylePicker', () => {
  it('is one small button that shows the colour, with the choices hidden until it is opened', async () => {
    render(StylePicker, { ...props, thicknesses: [...props.thicknesses] });
    const button = screen.getByRole('button', { name: 'Colour and thickness' });
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect((button.querySelector('.chip') as HTMLElement).style.background).toBe('rgb(224, 79, 79)');
    expect(screen.queryByRole('button', { name: 'Colour #4f8ee0' })).toBeNull();
    await fireEvent.click(button);
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('button', { name: 'Colour #e04f4f' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('button', { name: 'medium' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('picking a colour changes the dot and closes the panel; a thickness keeps it open', async () => {
    render(StylePicker, { ...props, thicknesses: [...props.thicknesses] });
    const button = screen.getByRole('button', { name: 'Colour and thickness' });
    await fireEvent.click(button);
    await fireEvent.click(screen.getByRole('button', { name: 'thick' }));
    expect(screen.getByRole('button', { name: 'thick' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('group', { name: 'Colour' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Colour #4f8ee0' }));
    expect(screen.queryByRole('group', { name: 'Colour' })).toBeNull();
    expect((button.querySelector('.chip') as HTMLElement).style.background).toBe('rgb(79, 142, 224)');
  });

  it('closes on a click elsewhere, and on Escape without the window hearing it', async () => {
    const heard = vi.fn();
    window.addEventListener('keydown', heard);
    render(StylePicker, { ...props, thicknesses: [...props.thicknesses] });
    const button = screen.getByRole('button', { name: 'Colour and thickness' });
    await fireEvent.click(button);
    await fireEvent.click(document.body);
    expect(button.getAttribute('aria-expanded')).toBe('false');
    await fireEvent.click(button);
    await fireEvent.keyDown(button, { key: 'Escape' });
    expect(button.getAttribute('aria-expanded')).toBe('false');
    // so the capture window, which closes on Escape, stays open
    expect(heard).not.toHaveBeenCalled();
    window.removeEventListener('keydown', heard);
  });
});
