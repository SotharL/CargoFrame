import { afterEach, describe, expect, it } from 'vitest';
import { memoryApi, setApiForTests, type Api } from './ipc';
import { setDebug, snapshot, startDebugView } from './debugView';
import type { DebugView } from './types';

afterEach(() => {
  document.body.innerHTML = '';
  setDebug(false);
  setApiForTests(undefined);
});

const page = `
  <main><h1>Layouts</h1>
    <div role="tablist"><button role="tab" aria-selected="false">Layouts</button><button role="tab" aria-selected="true">Fill fields</button></div>
    <section><h2>Fill a field</h2><label>On <select id="svc"><option>Jira</option></select></label></section>
  </main>`;

describe('what a window says it shows, in debug mode', () => {
  it('is read from the page: its title, the tab open, the section and the control in focus', () => {
    document.body.innerHTML = page;
    (document.getElementById('svc') as HTMLSelectElement).focus();
    startDebugView('settings', false, () => ({ guide: 'Connect Jira (step 2 of 5)' }));
    expect(snapshot()).toEqual({
      window: 'settings', page: 'Layouts', tab: 'Fill fields', section: 'Fill a field', focus: 'On', guide: 'Connect Jira (step 2 of 5)',
    });
  });

  it('is sent only while debug mode is on', async () => {
    const reported: DebugView[] = [];
    const api: Api = memoryApi({ reported });
    setApiForTests(api);
    document.body.innerHTML = page;
    startDebugView('settings', false);
    document.body.click();
    await new Promise((r) => setTimeout(r, 300));
    expect(reported).toEqual([]);
    setDebug(true);
    await new Promise((r) => setTimeout(r, 300));
    expect(reported.at(-1)?.page).toBe('Layouts');
  });
});
