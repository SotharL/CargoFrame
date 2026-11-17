// The language of the windows. English is written in the code; each other language is a pack,
// `packs/<code>.json`, mapping the English text to its translation (docs/i18n.md).
//
//   t('Save')                                  the text in the language in use
//   t('Save {key}', { key: '{map}' })          `{names}` filled in after translating
//   tn('{n} file', '{n} files', n)             one or several (`n` is filled in)
//   msg('Layouts')                             only marks a text kept in data (nav, guides);
//                                              translate it with t() where it is shown
//
// scripts/i18n/check.mjs reads these calls: every text must be in every pack. The program's own
// texts (connectors, checks, templates) come translated from the Rust side (core/i18n).

/** The languages CargoFrame speaks: code and name in that language. English first. */
export const LANGUAGES: { code: string; name: string }[] = [
  { code: 'en', name: 'English' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'it', name: 'Italiano' },
  { code: 'de', name: 'Deutsch' },
];

type Pack = Record<string, string>;
const loaders = import.meta.glob<Pack>('./packs/*.json', { import: 'default' });

const state = $state<{ code: string; pack: Pack }>({ code: 'en', pack: {} });

/** The language in use ('en', 'fr'…), for dates and numbers too. */
export const lang = () => state.code;

/** The language a setting asks for: 'system' follows `system` ('fr-FR'), English when none matches. */
export function resolve(setting: string | undefined, system: string | undefined): string {
  const wanted = (setting ?? '').trim().toLowerCase();
  const from = !wanted || wanted === 'system' ? (system ?? 'en').toLowerCase() : wanted;
  const code = /^[a-z]+/.exec(from)?.[0] ?? 'en';
  return LANGUAGES.some((l) => l.code === code) ? code : 'en';
}

/** Speak `code` from now on; its pack is read first. */
export async function setLanguage(code: string): Promise<void> {
  const load = loaders[`./packs/${code}.json`];
  const pack = code === 'en' || !load ? {} : await load().catch(() => ({}));
  state.pack = pack;
  state.code = load || code === 'en' ? code : 'en';
  if (typeof document !== 'undefined') document.documentElement.lang = state.code;
}

/** Fill `{name}` with its value; an unknown `{name}` stays as written. */
export function fill(text: string, values?: Record<string, string | number>): string {
  if (!values) return text;
  return text.replace(/\{([A-Za-z0-9_]+)\}/g, (all, name: string) => (name in values ? String(values[name]) : all));
}

/** `text` in the language in use (English when the pack has none), `{names}` filled in. */
export function t(text: string, values?: Record<string, string | number>): string {
  return fill(state.pack[text] || text, values);
}

/** One or several: `one` when the language says `n` is one (French: 0 and 1), else `other`; `{n}` is filled in. */
export function tn(one: string, other: string, n: number, values?: Record<string, string | number>): string {
  const single = new Intl.PluralRules(state.code).select(n) === 'one';
  return t(single ? one : other, { n, ...values });
}

/** Marks a text kept in data for the packs; translate it with t() where it is shown. */
export const msg = <T extends string>(text: T): T => text;

/** The properties of data that are texts to read (labels, hints, guide sentences…). */
const TEXT_KEYS = new Set(['label', 'hint', 'blurb', 'title', 'say', 'text', 'description', 'example', 'summary']);
const proxies = new WeakMap<object, object>();

/**
 * Data kept in a module (the sidebar, the blocks, the guides) whose texts are read in the
 * language in use: each text property (`keys`, or every string with `'all'`) goes through t()
 * when it is read. Mark the English texts with msg() so the packs have them.
 */
export function translated<T extends object>(data: T, keys: Set<string> | 'all' = TEXT_KEYS): T {
  const known = proxies.get(data);
  if (known && keys === TEXT_KEYS) return known as T;
  const proxy = new Proxy(data, {
    get(target, prop, receiver) {
      const v = Reflect.get(target, prop, receiver);
      if (typeof v === 'string' && typeof prop === 'string' && !Array.isArray(target) && (keys === 'all' || keys.has(prop))) return t(v);
      if (v && typeof v === 'object') return translated(v as object, keys);
      return v;
    },
  });
  if (keys === TEXT_KEYS) proxies.set(data, proxy);
  return proxy as T;
}
