import { describe, expect, it } from 'vitest';
import { memoryApi } from './memoryApi';

describe('in-memory backend', () => {
  it('stores settings between calls', async () => {
    const api = memoryApi();
    expect((await api.getGeneral()).theme).toBe('system');
    const notifications = { routine: false, problems: true };
    const changed = { ...(await api.getGeneral()), theme: 'dark' as const, memory_mode: 'low_memory' as const, accent: '#e8590c', notifications };
    await api.setGeneral(changed);
    expect(await api.getGeneral()).toEqual(changed);
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
