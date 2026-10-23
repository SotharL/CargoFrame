import { describe, expect, it } from 'vitest';
import { memoryApi } from './ipc';

describe('in-memory backend', () => {
  it('stores settings between calls', async () => {
    const api = memoryApi();
    expect((await api.getGeneral()).theme).toBe('system');
    await api.setGeneral({ theme: 'dark', memory_mode: 'low_memory', check_updates: false });
    expect(await api.getGeneral()).toEqual({
      theme: 'dark',
      memory_mode: 'low_memory',
      check_updates: false,
    });
  });

  it('returns copies, so callers cannot change stored state by accident', async () => {
    const api = memoryApi();
    const g = await api.getGeneral();
    g.theme = 'dark';
    expect((await api.getGeneral()).theme).toBe('system');
  });

  it('remembers the startup choice', async () => {
    const api = memoryApi();
    await api.setAutostart(false);
    expect(await api.getAutostart()).toBe(false);
  });
});
