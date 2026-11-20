import { beforeEach, describe, expect, it } from 'vitest';
import { closeKept, forgetKept, keepOpened, withKept, type ViewFile } from './files';

const file = (id: string): ViewFile => ({ id, name: id.replace('dropped:', ''), modified_ms: 0, lines: [], cut: false, problem: null });

describe('files kept while the Settings window is open', () => {
  beforeEach(forgetKept);

  it('shows the files opened before first, then the capture’s', () => {
    keepOpened([file('dropped:a.log')]);
    expect(withKept([file('Saved/game.log')]).map((f) => f.id)).toEqual(['dropped:a.log', 'Saved/game.log']);
    // opened again: once, first
    keepOpened([file('dropped:b.log'), file('dropped:a.log')]);
    expect(withKept([]).map((f) => f.id)).toEqual(['dropped:b.log', 'dropped:a.log']);
  });

  it('closes a file for good: an opened one is let go, a capture’s is hidden', () => {
    keepOpened([file('dropped:a.log')]);
    closeKept('dropped:a.log');
    closeKept('Saved/game.log');
    expect(withKept([file('Saved/game.log'), file('Saved/other.log')]).map((f) => f.id)).toEqual(['Saved/other.log']);
  });

  it('forgets it all when the window opens again', () => {
    keepOpened([file('dropped:a.log')]);
    closeKept('Saved/game.log');
    forgetKept();
    expect(withKept([file('Saved/game.log')]).map((f) => f.id)).toEqual(['Saved/game.log']);
  });
});
