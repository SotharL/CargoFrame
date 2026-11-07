// Windows and macOS ignore capital letters in file names; Linux does not. An import such as
// './guide.svelte' then finds Guide.svelte on Windows instead of guide.svelte.ts, and the build
// breaks only there. This fails when two files in a folder share a module name apart from case.
//
//   node scripts/ci/check-names.mjs [folder ...]      (default: ui/src and core, app)
import { readdirSync, statSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const folders = process.argv.length > 2 ? process.argv.slice(2) : ['ui/src', 'core', 'app/src'].map((d) => join(root, d));
const skip = new Set(['node_modules', 'target', 'dist', '.git']);

let clashes = 0;
function walk(dir) {
  const byModule = new Map();
  for (const name of readdirSync(dir)) {
    if (skip.has(name)) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      walk(path);
      continue;
    }
    // the name an import would use: guide.svelte.ts is imported as './guide.svelte'
    const module = name.replace(/\.(ts|js|rs)$/, '').toLowerCase();
    const list = byModule.get(module) ?? [];
    list.push(name);
    byModule.set(module, list);
  }
  for (const names of byModule.values()) {
    const distinct = new Set(names);
    // 'a.svelte' with 'a.svelte.ts' is fine to import by full name; only a case difference clashes
    if (distinct.size > 1 && new Set(names.map((n) => n.replace(/\.(ts|js|rs)$/, ''))).size > 1) {
      console.error(`Names that differ only by case in ${dir}: ${[...distinct].join(', ')}`);
      clashes++;
    }
  }
}
for (const f of folders) walk(f);
if (clashes) process.exit(1);
console.log('No file names that clash on Windows.');
