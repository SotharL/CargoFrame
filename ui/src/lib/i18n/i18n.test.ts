import { afterEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import { lang, resolve, setLanguage, t, tn } from './index.svelte';
import General from '../../settings/pages/Preferences.svelte';

afterEach(() => setLanguage('en'));

describe('Languages', () => {
  it('follows the setting, then the computer, and falls back to English', () => {
    expect(resolve('system', 'fr-FR')).toBe('fr');
    expect(resolve('de', 'fr-FR')).toBe('de');
    expect(resolve('system', 'pt-BR')).toBe('en');
    expect(resolve(undefined, undefined)).toBe('en');
  });

  it('speaks a pack once it is loaded, with the values filled after translating', async () => {
    await setLanguage('fr');
    expect(lang()).toBe('fr');
    expect(t('Save locally')).toBe('Enregistrer en local');
    expect(t('{n} of {total} selected', { n: 2, total: 5 })).toBe('2 sur 5 sélectionnés');
    expect(tn('{n} line', '{n} lines', 3)).toBe('3 lignes');
    expect(t('a text no pack has')).toBe('a text no pack has');
  });

  it('draws a page in the language chosen', async () => {
    await setLanguage('es');
    render(General);
    expect(await screen.findByRole('heading', { name: 'Idioma' })).toBeTruthy();
  });

  it('goes back to English for an unknown code', async () => {
    await setLanguage('klingon');
    expect(lang()).toBe('en');
    expect(t('Save locally')).toBe('Save locally');
  });
});
