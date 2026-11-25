#!/usr/bin/env node
// The translation packs follow the code: every text the code marks for translation must be in
// every pack, with the same {names}, and a pack keeps no text the code no longer has.
//
//   node scripts/i18n/check.mjs              check (exit 1 when a pack is behind)
//   node scripts/i18n/check.mjs --missing    the texts each pack lacks, as JSON to translate
//   node scripts/i18n/check.mjs --merge f    add the translations of f ({area: {lang: {en: tr}}})
//   node scripts/i18n/check.mjs --prune      drop the texts the code no longer has
//   node scripts/i18n/check.mjs --where      every text and a file it is in, to see where it is shown
//
// Areas: "ui" (ui/src: t(), tn(), msg()) and "app" (Rust: t(), tf(), msg!()). See docs/i18n.md.
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = new URL('../../', import.meta.url).pathname;
const LANGS = ['fr', 'es', 'it', 'de'];

const AREAS = {
  ui: {
    dirs: ['ui/src'],
    files: (f) => /\.(svelte|ts)$/.test(f) && !/\.test\.ts$|\.harness\.svelte$/.test(f) && !f.includes('/i18n/'),
    packs: 'ui/src/lib/i18n/packs',
    find: findUi,
  },
  app: {
    dirs: ['app/src', 'core'],
    // the i18n crate's own tests are not texts of the program
    files: (f) => f.endsWith('.rs') && !f.includes('/target/') && !f.includes('core/i18n/'),
    packs: 'core/i18n/packs',
    find: findRust,
  },
};

function walk(dir, keep, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'target' || name.startsWith('.')) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, keep, out);
    else if (keep(p)) out.push(p);
  }
  return out;
}

/** A JS string literal's text. */
function jsText(lit) {
  const quote = lit[0];
  const body = lit.slice(1, -1);
  if (quote === '`' && body.includes('${')) return null;
  return body.replace(/\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|.)/gs, (_, e) => {
    if (e[0] === 'u') return String.fromCodePoint(parseInt(e.replace(/[u{}]/g, ''), 16));
    if (e[0] === 'x') return String.fromCharCode(parseInt(e.slice(1), 16));
    return { n: '\n', t: '\t', r: '\r', '\n': '' }[e] ?? e;
  });
}

/** A Rust string literal's text (a backslash at a line's end joins the next line). */
function rustText(body) {
  return body
    .replace(/\\\r?\n\s*/g, '')
    .replace(/\\(u\{[0-9a-fA-F]+\}|.)/g, (_, e) =>
      e[0] === 'u' ? String.fromCodePoint(parseInt(e.slice(2, -1), 16)) : ({ n: '\n', t: '\t', r: '\r', 0: '\0' }[e] ?? e),
    );
}

const JS_LIT = String.raw`('(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|` + '`(?:[^`\\\\]|\\\\.)*`' + ')';
const UI_ONE = new RegExp(String.raw`(?<![\w.$])(?:t|tx|tr|msg)\(\s*` + JS_LIT, 'g');
// tn(one, other, n): the plural is often written t('…') (found by UI_ONE), so the singular is
// looked for on its own, with or without a plain plural after it
const UI_TWO = new RegExp(String.raw`(?<![\w.$])tn\(\s*` + JS_LIT + String.raw`(?:\s*,\s*` + JS_LIT + ')?', 'g');

function findUi(src, file, add) {
  src = uncommented(src);
  for (const m of src.matchAll(UI_ONE)) add(jsText(m[1]), file);
  for (const m of src.matchAll(UI_TWO)) {
    add(jsText(m[1]), file);
    if (m[2]) add(jsText(m[2]), file);
  }
}

const RS_LIT = String.raw`"((?:[^"\\]|\\.|\\\r?\n)*)"`;
// t("…") alone (also split over lines by rustfmt, which adds a comma: `t(\n "…",\n)`), tf("…", values), msg!("…")
const RS = new RegExp(String.raw`(?<![\w.])(?:(?:cargoframe_i18n::)?t\(\s*` + RS_LIT + String.raw`\s*,?\s*\)|(?:cargoframe_i18n::)?tf\(\s*` + RS_LIT + String.raw`\s*,|(?:cargoframe_i18n::)?msg!\(\s*` + RS_LIT + ')', 'g');

const uncommented = (src) => src.replace(/^\s*\/\/.*$/gm, '');

function findRust(src, file, add) {
  for (const m of uncommented(src).matchAll(RS)) add(rustText(m[1] ?? m[2] ?? m[3]), file);
}

/** The texts an area's code marks, with a file each was found in. */
function texts(area) {
  const found = new Map();
  for (const dir of area.dirs) {
    for (const file of walk(join(root, dir), area.files)) {
      area.find(readFileSync(file, 'utf8'), relative(root, file), (text, where) => {
        if (text && !found.has(text)) found.set(text, where);
      });
    }
  }
  return found;
}

const names = (s) => [...s.matchAll(/\{([A-Za-z0-9_.]+)\}/g)].map((m) => m[1]).sort().join(',');
const packPath = (area, lang) => join(root, area.packs, `${lang}.json`);
const readPack = (area, lang) => {
  try {
    return JSON.parse(readFileSync(packPath(area, lang), 'utf8'));
  } catch {
    return {};
  }
};
const writePack = (area, lang, pack) => {
  const sorted = Object.fromEntries(Object.entries(pack).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  writeFileSync(packPath(area, lang), JSON.stringify(sorted, null, 1) + '\n');
};

const mode = process.argv[2] ?? '--check';
let problems = 0;
const missingOut = {};

if (mode === '--merge') {
  const add = JSON.parse(readFileSync(process.argv[3], 'utf8'));
  for (const [name, byLang] of Object.entries(add)) {
    for (const [lang, entries] of Object.entries(byLang)) {
      const pack = readPack(AREAS[name], lang);
      Object.assign(pack, entries);
      writePack(AREAS[name], lang, pack);
    }
  }
  process.exit(0);
}

if (mode === '--where') {
  const all = {};
  for (const [name, area] of Object.entries(AREAS)) all[name] = Object.fromEntries(texts(area));
  console.log(JSON.stringify(all, null, 1));
  process.exit(0);
}

for (const [name, area] of Object.entries(AREAS)) {
  const code = texts(area);
  for (const lang of LANGS) {
    const pack = readPack(area, lang);
    const missing = [...code.keys()].filter((k) => !pack[k]);
    const stale = Object.keys(pack).filter((k) => !code.has(k));
    const wrong = Object.entries(pack).filter(([k, v]) => code.has(k) && v && names(k) !== names(v));
    if (mode === '--prune' && stale.length) {
      for (const k of stale) delete pack[k];
      writePack(area, lang, pack);
      console.log(`${name}/${lang}: dropped ${stale.length}`);
      continue;
    }
    if (mode === '--missing') {
      if (missing.length) ((missingOut[name] ??= {})[lang] = Object.fromEntries(missing.map((k) => [k, ''])));
      continue;
    }
    if (missing.length || stale.length || wrong.length) {
      problems += missing.length + stale.length + wrong.length;
      console.log(`${name}/${lang}: ${missing.length} missing, ${stale.length} no longer in the code, ${wrong.length} with other {names}`);
      for (const k of missing.slice(0, 5)) console.log(`  missing: ${JSON.stringify(k)} (${code.get(k)})`);
      for (const k of stale.slice(0, 5)) console.log(`  stale:   ${JSON.stringify(k)}`);
      for (const [k, v] of wrong.slice(0, 5)) console.log(`  names:   ${JSON.stringify(k)} → ${JSON.stringify(v)}`);
    }
  }
  if (mode === '--check') console.log(`${name}: ${code.size} texts`);
}

if (mode === '--missing') console.log(JSON.stringify(missingOut, null, 1));
if (mode === '--check' && problems) {
  console.log(`\nThe packs are behind the code. node scripts/i18n/check.mjs --missing lists what to translate (docs/i18n.md).`);
  process.exit(1);
}
