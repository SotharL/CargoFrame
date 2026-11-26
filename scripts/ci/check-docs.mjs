// The Markdown files, as GitHub shows them:
// - every relative link and picture points at a file that exists (a moved or deleted file breaks none),
// - every `#section` of a link names a heading of that page, written as GitHub writes it,
// - no Mermaid diagram has a bare `;`, which Mermaid reads as the end of a line: GitHub then shows
//   "Unable to render rich display" instead of the diagram (put the text in quotes, or use a comma).
//
//   node scripts/ci/check-docs.mjs
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
// written by cargo-about, quoting each library's own license
const skip = new Set(['docs/third-party-licenses.md']);
const files = execFileSync('git', ['ls-files', '*.md'], { cwd: root, encoding: 'utf8' })
  .split('\n')
  .filter((f) => f && !skip.has(f) && !f.startsWith('ui/node_modules/') && existsSync(join(root, f)));

const problems = [];
const headingsOf = new Map();

/** The anchors GitHub gives a page's headings: lower case, signs left out, spaces as `-`, repeats numbered. */
function headings(file) {
  if (headingsOf.has(file)) return headingsOf.get(file);
  const seen = new Map();
  const found = new Set();
  let fenced = false;
  for (const line of readFileSync(join(root, file), 'utf8').split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced;
    const m = !fenced && /^#{1,6}\s+(.*?)\s*#*\s*$/.exec(line);
    if (!m) continue;
    const text = m[1]
      .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1') // a link keeps its words
      .replace(/<[^>]+>/g, '')
      .replace(/[`*_]/g, (c) => (c === '_' ? '_' : ''));
    const slug = text
      .toLowerCase()
      .replace(/[^\p{L}\p{N}\s_-]/gu, '')
      .replace(/\s/g, '-');
    const n = seen.get(slug) ?? 0;
    seen.set(slug, n + 1);
    found.add(n ? `${slug}-${n}` : slug);
  }
  headingsOf.set(file, found);
  return found;
}

function checkTarget(file, lineNo, target) {
  if (!target || /^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('//')) return; // https:, mailto:…
  const [path, anchor] = target.split('#');
  let dest = file;
  if (path) {
    dest = join(dirname(file), decodeURIComponent(path)).replace(/\\/g, '/');
    if (!existsSync(join(root, dest))) {
      problems.push(`${file}:${lineNo}: ${target}: no such file`);
      return;
    }
  }
  if (anchor && dest.endsWith('.md') && statSync(join(root, dest)).isFile() && !headings(dest).has(anchor.toLowerCase())) {
    problems.push(`${file}:${lineNo}: ${target}: no heading "#${anchor}" in ${dest}`);
  }
}

for (const file of files) {
  const lines = readFileSync(join(root, file), 'utf8').split('\n');
  let fence = null; // the language of the code block the line is in
  lines.forEach((line, i) => {
    const lineNo = i + 1;
    const open = /^\s*(```|~~~)\s*([\w-]*)/.exec(line);
    if (open) {
      fence = fence === null ? open[2] || 'text' : null;
      return;
    }
    if (fence === 'mermaid') {
      const outsideQuotes = line.replace(/"[^"]*"/g, '').replace(/^\s*%%.*$/, '');
      if (outsideQuotes.includes(';')) {
        problems.push(`${file}:${lineNo}: a ";" in a Mermaid diagram ends the line there; use a comma or quotes`);
      }
      return;
    }
    if (fence !== null) return;
    const code = line.replace(/`[^`]*`/g, ''); // a path written as code is not a link
    for (const m of code.matchAll(/!?\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g)) checkTarget(file, lineNo, m[1]);
    for (const m of code.matchAll(/\b(?:src|href|srcset)="([^"]+)"/g)) checkTarget(file, lineNo, m[1].split(/\s/)[0]);
  });
}

if (problems.length) {
  console.error(problems.join('\n'));
  console.error(`\n${problems.length} problem(s) in the docs`);
  process.exit(1);
}
console.log(`docs: ${files.length} Markdown files, every link, picture and diagram fine`);
