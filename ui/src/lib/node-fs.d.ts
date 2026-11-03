// The one piece of Node the tests use: reading a source file next to the page. The page itself
// never runs under Node, so the real Node types are not pulled in for the whole project.
declare module 'node:fs' {
  export function readFileSync(path: URL, encoding: 'utf8'): string;
}
