// The files of the last capture in the in-memory stand-in (`npm run dev`, tests, screenshots): an
// Unreal-like game log and editor log, and a system report.
import type { Rule, ValuesPreview } from './types';
import { choose, hits, linesOf, valueOf } from '../settings/data/pick';
import { valuesOf } from '../settings/ruleValues';

const at = (t: string) => `[2026.11.16-${t}][  0]`;
const MIN = 60_000;
const BASE = Date.UTC(2026, 10, 16, 13, 0);

export const DEMO_SAMPLES: { path: string; text: string; modified_ms: number }[] = [
  {
    path: 'Saved/Logs/Editor.log',
    modified_ms: BASE + 63 * MIN,
    text: [
      `${at('14.01.10:004')}LogInit: Build: ++Game+Main-CL-48213`,
      `${at('14.01.22:511')}LogEditor: Opening /Game/Maps/Harbor_P`,
      `${at('14.02.40:090')}LogLoad: LoadMap: /Game/Maps/Harbor_Night_P?listen`,
      `${at('14.02.43:301')}LogRHI: GPU: NVIDIA GeForce RTX 4070`,
      `${at('14.02.51:410')}LogAI: Spawned 30 crew on Harbor_Night_P`,
      `${at('14.03.12:015')}LogPlayLevel: PIE: Session ended`,
    ].join('\n'),
  },
  {
    path: 'Saved/Logs/Game.log',
    modified_ms: BASE + 59 * MIN,
    text: [
      'Log file open, 11/16/26 13:58:01',
      `${at('13.58.02:114')}LogInit: Build: ++Game+Main-CL-48213`,
      `${at('13.58.02:131')}LogInit: OS: Windows 11 (23H2), CPU: AMD Ryzen 7 7800X3D, GPU: NVIDIA GeForce RTX 4070`,
      `${at('13.58.09:877')}LogLoad: LoadMap: /Game/Maps/MainMenu?listen`,
      `${at('13.59.12:440')}LogLoad: LoadMap: /Game/Maps/Harbor_P?listen`,
      `${at('13.59.19:251')}LogLoad: Took 6.81 seconds to LoadMap(/Game/Maps/Harbor_P)`,
      `${at('13.59.24:552')}Warning: LogPhysics: Overlap budget exceeded (14 ms)`,
      `${at('13.59.31:090')}LogAI: Spawned 12 crew on Harbor_P`,
      `${at('13.59.52:612')}Error: LogPhysics: Body instance is NaN on Crane_03`,
      `${at('13.59.58:120')}LogExit: Exiting.`,
    ].join('\n'),
  },
  {
    path: 'system_info.txt',
    modified_ms: BASE - 230 * MIN,
    text: ['Machine: DESKTOP-7Q2KJ4M', 'OS: Windows 11 Pro 23H2', 'GPU: NVIDIA GeForce RTX 4070', 'RAM: 32 GB', 'Driver: 566.14'].join('\n'),
  },
];

const baseName = (p: string) => p.split('/').pop() ?? p;

/** What the saved values give in the demo files: picks are run, other ways give "<name> demo". */
export function demoValues(rules: Rule[]): ValuesPreview {
  const out: ValuesPreview = { folder: '/home/me/dev/Harbor', values: {}, sources: {}, problems: {} };
  const best: Record<string, number> = {};
  rules.forEach((r, i) => {
    for (const v of valuesOf(`rule-${i}`, r)) {
      if (v.find.type === 'combine') continue;
      const file = DEMO_SAMPLES.find((f) => baseName(f.path) === r.file.trim() || f.path.includes(r.file.trim()));
      if (!file) {
        if (!(v.key in out.values)) out.problems[v.key] = `No file matches ${r.file}`;
        continue;
      }
      const got = v.find.type === 'pick' ? valueOf(choose(hits(linesOf(file.text), v.find), v.which)) : `${v.name || v.key} demo`;
      if (!got) {
        if (!(v.key in out.values)) out.problems[v.key] = `Not found in ${baseName(file.path)}`;
        continue;
      }
      const key = `rule.${v.key}`;
      if (v.across === 'all' && out.values[key] && out.values[key] !== got) {
        out.values[key] = `${out.values[key]}, ${got}`;
      } else if ((best[v.key] ?? -1) < file.modified_ms) {
        best[v.key] = file.modified_ms;
        out.values[key] = got;
        out.sources[v.key] = file.path;
      }
      delete out.problems[v.key];
    }
  });
  return out;
}
