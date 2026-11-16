import { describe, expect, it } from 'vitest';
import { choose, describePick, hits, labelBefore, linesOf, search, signAfter, snap, suggest, valueOf, type Pick } from './pick';

// the same log as core/rules/src/find.rs's tests: both sides must find the same values
const GAME_LOG = linesOf(
  'Log file open, 11/16/26 13:58:01\n' +
    '[2026.11.16-13.58.09:877][  0]LogLoad: LoadMap: /Game/Maps/MainMenu?listen\n' +
    '[2026.11.16-13.59.12:440][  0]LogLoad: LoadMap: /Game/Maps/Harbor_P?listen\n' +
    '[2026.11.16-13.59.19:251][  0]LogLoad: Took 6.81 seconds to LoadMap(/Game/Maps/Harbor_P)\n' +
    '[2026.11.16-13.59.28:300][  0]LogAI: Spawned 4 gulls on Harbor_P\n' +
    '[2026.11.16-13.59.31:090][  0]LogAI: Spawned 12 crew on Harbor_P\n',
);
const pick = (lines_with: string, from_line_start: boolean, stop: string | null): Pick => ({ type: 'pick', lines_with, from_line_start, stop });
const run = (p: Pick, which: 'first' | 'last' | 'all', lines = GAME_LOG) => valueOf(choose(hits(lines, p), which));

describe('a value picked in a file', () => {
  it('is found again the way core/rules finds it', () => {
    expect(hits(GAME_LOG, pick('Maps/', false, '?')).map((h) => h.value)).toEqual(['MainMenu', 'Harbor_P']);
    expect(run(pick('Maps/', false, '?'), 'last')).toBe('Harbor_P');
    expect(run(pick('Maps/', false, '?'), 'first')).toBe('MainMenu');
    expect(run(pick('LogAI:', false, null), 'last')).toBe('Spawned 12 crew on Harbor_P');
    expect(run(pick('', true, '['), 'last')).toBe('[2026.11.16-13.59.31:090]');
    expect(run(pick('LogAI:', true, null), 'first')).toBe('[2026.11.16-13.59.28:300][  0]LogAI: Spawned 4 gulls on Harbor_P');
    expect(run(pick('Carte:', false, '?'), 'last', ['Carte: Hafen_é?x'])).toBe('Hafen_é');
    expect(run(pick('Nowhere:', false, null), 'last')).toBe('');
    expect(run(pick('Spawned', false, ' '), 'all')).toBe('4, 12');
  });

  it('comes from what was selected, a word cut at either end taken whole', () => {
    const line = GAME_LOG[5]!;
    // a click takes the word under it
    const click = snap(line, line.indexOf('rbor_P'), line.indexOf('rbor_P'));
    expect(line.slice(click!.s, click!.e)).toBe('Harbor_P');
    // a drag from inside a word to inside another takes both whole
    const drag = snap(line, line.indexOf('awned'), line.indexOf('rew') + 1);
    expect(line.slice(drag!.s, drag!.e)).toBe('Spawned 12 crew');
    // a time stamp with its signs, the way it was dragged over
    const stamp = '[2026.11.16-13.58.02:114][  0]LogInit: Build: ++Game+Main-CL-48213';
    const t = snap(stamp, 0, 25)!;
    expect(stamp.slice(t.s, t.e)).toBe('[2026.11.16-13.58.02:114]');
    expect(suggest(stamp, t.s, t.e)).toEqual(pick('', true, '['));
    // a click on a sign takes nothing, unless a sign is wanted
    expect(snap(stamp, 0, 0)).toBeNull();
    expect(snap(stamp, 0, 0, true)).toEqual({ s: 0, e: 1 });
  });

  it('suggests the label before it and what follows it', () => {
    const map = GAME_LOG[2]!;
    const s = map.indexOf('Harbor_P');
    expect(labelBefore(map, s)).toBe('Maps/');
    expect(signAfter(map, s + 8)).toBe('?');
    expect(suggest(map, s, s + 8)).toEqual(pick('Maps/', false, '?'));
    const ai = GAME_LOG[5]!;
    const from = ai.indexOf('Spawned');
    expect(suggest(ai, from, ai.length)).toEqual(pick('LogAI:', false, null));
    const twelve = ai.indexOf('12');
    expect(suggest(ai, twelve, twelve + 2)).toEqual(pick('Spawned', false, ' '));
    const json = '  "version": "1.4.2",';
    const v = json.indexOf('1.4.2');
    expect(suggest(json, v, v + 5)).toEqual(pick('version": "', false, '"'));
    expect(run(pick('version": "', false, '"'), 'first', [json])).toBe('1.4.2');
    expect(describePick(pick('LogAI:', false, null), 'last')).toBe('After “LogAI:”, to the end of the line, the most recent line');
    expect(describePick(pick('', true, '['), 'first')).toBe('Every line, from its start, up to “[”, the first line');
  });

  it('is searched for like Ctrl+F', () => {
    expect(search(GAME_LOG, 'harbor').map((h) => h.line)).toEqual([2, 3, 4, 5]);
    expect(search(GAME_LOG, '')).toEqual([]);
    expect(linesOf('a\r\nb\n')).toEqual(['a', 'b']);
  });
});
