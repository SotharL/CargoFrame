import { describe, expect, it } from 'vitest';
import { fillIn, ruleKey, shortNameOk, token, usesKey, valuesIn } from './valueNames';

describe('how a value is written', () => {
  it('reads both ways, and leaves braces written for other reasons', () => {
    const text = 'On {map} with {{rule.gpu}} in {app.exe}; {code}x{code} {color:red} {"a": 1} {0} {a b} {file.txt} \\{{x}}';
    expect(valuesIn(text).map((v) => v.name)).toEqual(['map', 'rule.gpu', 'app.exe']);
    const [first] = valuesIn(text);
    expect(text.slice(first!.start, first!.end)).toBe('{map}');
  });

  it('names a value read from files the short way', () => {
    expect(token('rule.gpu')).toBe('{gpu}');
    expect(token('app.exe')).toBe('{app.exe}');
    expect(ruleKey('rule.gpu')).toBe('gpu');
    expect(ruleKey('gpu')).toBe('gpu');
    expect(ruleKey('app.exe')).toBeNull();
    expect(shortNameOk('Code')).toBe(false);
    expect(usesKey('a {{rule.gpu}} b', 'gpu') && usesKey('{gpu}', 'gpu') && usesKey('{{gpu}}', 'gpu')).toBe(true);
    expect(usesKey('{gpus}', 'gpu')).toBe(false);
  });

  it('fills a text in with the values of the last capture, for a preview', () => {
    const parts = fillIn('On {map} ({{rule.gpu}}), {crash}', { 'rule.map': 'Harbor_P', 'rule.gpu': 'RTX 4070' });
    expect(parts.map((p) => p.t).join('')).toBe('On Harbor_P (RTX 4070), {crash}');
    expect(parts.filter((p) => p.empty).map((p) => p.t)).toEqual(['{crash}']);
  });
});
