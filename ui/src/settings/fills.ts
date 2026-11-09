// Fill fields: a field of a service and the steps that fill it on each share
// ("If {{rule.gpu}} is found → add gpu-{{rule.gpu}} to Labels"). Mirror of core/config/src/infotype.rs.
import type { FillStep, FillTest, FillWhen, InfoType } from '../lib/types';

export const TESTS: { value: FillTest; label: string; needsText: boolean }[] = [
  { value: 'found', label: 'is found', needsText: false },
  { value: 'missing', label: 'is not found', needsText: false },
  { value: 'is', label: 'is', needsText: true },
  { value: 'is_not', label: 'is not', needsText: true },
  { value: 'contains', label: 'contains', needsText: true },
];
export const needsText = (t: FillTest) => TESTS.find((x) => x.value === t)?.needsText ?? false;

/** A fill with nothing in it. */
export function blankFill(): InfoType {
  return { name: '', connection_id: '', field: '', kind: 'text', options: { type: 'none' }, required: false, steps: [newStep()], layouts: [] };
}

export const newStep = (): FillStep => ({ when: null, then: [{ do: 'add', text: '' }] });
export const newWhen = (): FillWhen => ({ subject: '', test: 'found', text: '' });

/** The steps to edit: a starting value from before 0.3.0 becomes the first ("if empty, set"). */
export function stepsOf(t: InfoType): FillStep[] {
  const out: FillStep[] = [];
  if (t.default_template?.trim()) {
    out.push({ when: { subject: '', test: 'missing', text: '' }, then: [{ do: 'set', text: t.default_template }] });
  }
  return [...out, ...(t.steps ?? [])];
}

/** A fill as it is saved: steps only, empty actions dropped, no starting value. */
export function cleaned(t: InfoType, steps: FillStep[]): InfoType {
  const kept = steps
    .map((s) => ({
      ...(s.when ? { when: { subject: s.when.subject.trim().replace(/^\{\{|\}\}$/g, ''), test: s.when.test, text: s.when.text } } : {}),
      then: s.then.filter((a) => a.text.trim() !== '' || a.do === 'set'),
    }))
    .filter((s) => s.then.length > 0);
  const { default_template: _old, ...rest } = t;
  return { ...rest, steps: kept };
}

const quote = (s: string) => `“${s.trim()}”`;

/** "If {{rule.gpu}} is found" / "If this field is empty" / "Always". */
export function whenText(w: FillWhen | null | undefined): string {
  if (!w) return 'Always';
  const subject = w.subject.trim() ? `{{${w.subject.trim()}}}` : 'this field';
  if (!w.subject.trim() && w.test === 'missing') return 'If this field is empty';
  if (!w.subject.trim() && w.test === 'found') return 'If this field has a value';
  const t = TESTS.find((x) => x.value === w.test);
  return `If ${subject} ${t?.label ?? w.test}${t?.needsText ? ` ${quote(w.text)}` : ''}`;
}

/** "add “crash” and add “gpu”". */
export function thenText(s: FillStep): string {
  return s.then.map((a) => `${a.do === 'set' ? 'set it to' : 'add'} ${a.text.trim() ? quote(a.text) : '(nothing)'}`).join(' and ');
}

/** One line per step for the card. */
export function stepLines(t: InfoType): string[] {
  return stepsOf(t).map((s) => `${whenText(s.when)}: ${thenText(s)}`);
}
