// How a value is written in a text: `{gpu}` (0.5.0) or `{{rule.gpu}}`, `{app.exe}` or
// `{{app.exe}}`. Mirror of core/context/src/template.rs: the same names read as values there and
// here, so what the builder shows as a value is what a capture fills in.

/** Names Jira's own formatting writes in single braces: never values. */
export const RESERVED = ['code', 'noformat', 'quote', 'color', 'panel', 'anchor', 'toc', 'section', 'column'];
const GROUPS = ['app.', 'project.', 'capture.', 'rule.', 'system.', 'cargoframe.'];

const validName = (n: string) => /^[A-Za-z0-9_.-]+$/.test(n);

/** Whether `{name}` is meant as a value. */
export function shortNameOk(name: string): boolean {
  return (
    validName(name) &&
    /^[A-Za-z0-9_]/.test(name) &&
    !/^\d+$/.test(name) &&
    !RESERVED.includes(name.toLowerCase()) &&
    (!name.includes('.') || GROUPS.some((g) => name.startsWith(g) && name.length > g.length))
  );
}

/** The key of a value read from files that `name` stands for: `rule.gpu` and `gpu` are `gpu`. */
export function ruleKey(name: string): string | null {
  if (name.startsWith('rule.')) return name.slice(5);
  return name.includes('.') ? null : name;
}

/** How a value is written, the short way: `rule.gpu` → `{gpu}`, `app.exe` → `{app.exe}`. */
export function token(name: string): string {
  const key = name.startsWith('rule.') ? name.slice(5) : name;
  return `{${key}}`;
}

/** The capture's name for what a text writes: `gpu` → `rule.gpu`, `app.exe` stays. */
export const fullName = (name: string) => (name.includes('.') ? name : `rule.${name}`);

/** Every value written in `text`, either way, as `{ name, start, end }` (end excluded). */
export function valuesIn(text: string): { name: string; start: number; end: number }[] {
  const out: { name: string; start: number; end: number }[] = [];
  let i = 0;
  while (i < text.length) {
    if (text.startsWith('\\{{', i)) {
      i += 3;
      continue;
    }
    if (text.startsWith('{{', i)) {
      const end = text.indexOf('}}', i + 2);
      const name = end < 0 ? '' : text.slice(i + 2, end).trim();
      if (end >= 0 && validName(name)) {
        out.push({ name, start: i, end: end + 2 });
        i = end + 2;
      } else {
        i += 2;
      }
      continue;
    }
    if (text[i] === '{') {
      const end = text.indexOf('}', i + 1);
      const name = end < 0 || end - i - 1 > 64 ? '' : text.slice(i + 1, end);
      if (shortNameOk(name)) {
        out.push({ name, start: i, end: end + 1 });
        i = end + 1;
        continue;
      }
    }
    i += 1;
  }
  return out;
}

/** Whether `text` uses the value read from files `key`, either way. */
export const usesKey = (text: string, key: string) => valuesIn(text).some((v) => ruleKey(v.name) === key);

/** `text` cut into plain parts and values filled from `values` (by full name), to show a preview. */
export function fillIn(text: string, values: Readonly<Record<string, string>>): { t: string; value: boolean; empty: boolean }[] {
  const out: { t: string; value: boolean; empty: boolean }[] = [];
  let at = 0;
  for (const v of valuesIn(text)) {
    if (v.start > at) out.push({ t: text.slice(at, v.start), value: false, empty: false });
    const got = values[v.name] ?? values[fullName(v.name)] ?? '';
    out.push({ t: got || token(v.name), value: true, empty: !got });
    at = v.end;
  }
  if (at < text.length) out.push({ t: text.slice(at), value: false, empty: false });
  return out;
}
