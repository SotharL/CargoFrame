// Size gate for the web UI: what each window loads up front, gzipped, against the budget in the
// performance plan (250 KB of JavaScript per window, plus 20% margin).
//
//   node scripts/ci/check-budgets.mjs [ui/dist]
//
// A window's cost is the module script of its HTML page, the chunks the page preloads, and its
// stylesheets. Chunks loaded later on demand are listed but not counted.
import { readFileSync, readdirSync, existsSync, appendFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const dist = resolve(process.argv[2] ?? join(root, 'ui/dist'));
const BUDGET_JS_KB = 250;
const MARGIN = 1.2;
const gate = BUDGET_JS_KB * MARGIN;

if (!existsSync(dist)) {
  console.error(`No build found at ${dist}. Run "npm run build" in ui first.`);
  process.exit(2);
}

const gz = (file) => gzipSync(readFileSync(file), { level: 9 }).length;
const kb = (n) => (n / 1024).toFixed(1);
const refs = (html, re) => [...html.matchAll(re)].map((m) => m[1]);

let failed = false;
const lines = ['### UI size per window (gzip)', '', '| Window | JS up front | CSS | Gate (JS) |', '|---|---|---|---|'];
const pages = readdirSync(dist).filter((f) => f.endsWith('.html')).sort();
if (pages.length === 0) {
  console.error('No HTML pages in the build.');
  process.exit(2);
}

for (const page of pages) {
  const html = readFileSync(join(dist, page), 'utf8');
  const js = new Set([
    ...refs(html, /<script[^>]*type="module"[^>]*src="([^"]+)"/g),
    ...refs(html, /<link[^>]*rel="modulepreload"[^>]*href="([^"]+)"/g),
  ]);
  const css = new Set(refs(html, /<link[^>]*rel="stylesheet"[^>]*href="([^"]+)"/g));
  const size = (set) =>
    [...set].reduce((sum, ref) => sum + gz(join(dist, ref.replace(/^\.\//, ''))), 0);
  const jsBytes = size(js);
  const cssBytes = size(css);
  const ok = jsBytes <= gate * 1024;
  if (!ok) failed = true;
  console.log(
    `${page}: ${kb(jsBytes)} KB JS in ${js.size} file(s), ${kb(cssBytes)} KB CSS ` +
      `(budget ${BUDGET_JS_KB} KB, gate ${gate} KB) ${ok ? 'ok' : 'OVER'}`,
  );
  lines.push(`| ${page} | ${kb(jsBytes)} KB | ${kb(cssBytes)} KB | ${gate} KB ${ok ? '' : '**over**'} |`);
}

if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, lines.join('\n') + '\n');
if (failed) {
  console.error('A window is over its JavaScript budget.');
  process.exit(1);
}
