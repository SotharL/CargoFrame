// Fill fields: a field of a service and the steps that fill it on each share
// ("If {gpu} is found → add gpu-{gpu} to Labels"). Mirror of core/config/src/infotype.rs.
import type { FillStep, FillTest, FillWhen, InfoType } from '../lib/types';
import { fullName, token } from '../lib/valueNames';
import { t, msg, translated } from '../lib/i18n/index.svelte';

/** What a step looks at, as the capture names it: `{gpu}`, `{{rule.gpu}}` and `gpu` are `rule.gpu`. */
const subjectName = (s: string) => {
  const n = s.trim().replace(/^\{+|\}+$/g, '').trim();
  return n ? fullName(n) : '';
};

export const TESTS: { value: FillTest; label: string; needsText: boolean }[] = translated([
  { value: 'found', label: msg('is found'), needsText: false },
  { value: 'missing', label: msg('is not found'), needsText: false },
  { value: 'is', label: msg('is'), needsText: true },
  { value: 'is_not', label: msg('is not'), needsText: true },
  { value: 'contains', label: msg('contains'), needsText: true },
]);
export const needsText = (test: FillTest) => TESTS.find((x) => x.value === test)?.needsText ?? false;

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
export function cleaned(it: InfoType, steps: FillStep[]): InfoType {
  const kept = steps
    .map((s) => ({
      ...(s.when ? { when: { subject: subjectName(s.when.subject), test: s.when.test, text: s.when.text } } : {}),
      then: s.then.filter((a) => a.text.trim() !== '' || a.do === 'set'),
    }))
    .filter((s) => s.then.length > 0);
  const { default_template: _old, ...rest } = it;
  return { ...rest, steps: kept };
}

const quote = (s: string) => `“${s.trim()}”`;

/** "If {gpu} is found" / "If this field is empty" / "Always". */
function whenText(w: FillWhen | null | undefined): string {
  if (!w) return t('Always');
  const subject = w.subject.trim() ? token(w.subject.trim().replace(/^\{+|\}+$/g, '')) : t('this field');
  if (!w.subject.trim() && w.test === 'missing') return t('If this field is empty');
  if (!w.subject.trim() && w.test === 'found') return t('If this field has a value');
  const values = { value: subject, text: quote(w.text) };
  switch (w.test) {
    case 'found':
      return t('If {value} is found', values);
    case 'missing':
      return t('If {value} is not found', values);
    case 'is':
      return t('If {value} is {text}', values);
    case 'is_not':
      return t('If {value} is not {text}', values);
    default:
      return t('If {value} contains {text}', values);
  }
}

/** "add “crash” and add “gpu”". */
function thenText(s: FillStep): string {
  return s.then
    .map((a) => {
      const text = a.text.trim() ? quote(a.text) : t('(nothing)');
      return a.do === 'set' ? t('set it to {text}', { text }) : t('add {text}', { text });
    })
    .join(` ${t('and')} `);
}

/** One line per step for the card. */
export function stepLines(it: InfoType): string[] {
  return stepsOf(it).map((s) => `${whenText(s.when)}: ${thenText(s)}`);
}
