// Draws the program's icon files from the logo drawings in assets/brand (run when the logo changes):
//
//   node scripts/dev/render-icons.mjs
//
// It needs Playwright with its Chromium (the same one the visual checks use). Nothing sits behind
// the mark: every picture is transparent. At 16 to 32 px the small drawing is used, from 48 px up
// the large one, as the brand rules say. The tray icon is made of two layers, the frames and the
// arrow, because the arrow takes the accent colour chosen in Settings (see app/src/tray.rs).
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Playwright is found the way `require` finds it (a global install is found through NODE_PATH).
const { chromium } = createRequire(import.meta.url)('playwright');

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const brand = (f) => readFileSync(join(root, 'assets/brand', f), 'utf8');
const icons = join(root, 'app/icons');

const large = brand('convoy-icon-static.svg');
const small = brand('convoy-icon-small-static.svg');
const frames = brand('tray-frames.svg');
const arrow = brand('tray-arrow.svg');

const browser = await chromium.launch();
const page = await browser.newPage({ deviceScaleFactor: 1 });

async function png(svg, size) {
  await page.setViewportSize({ width: size, height: size });
  const body = svg.replace('<svg ', `<svg width="${size}" height="${size}" style="display:block" `);
  await page.setContent(`<html><body style="margin:0;background:transparent">${body}</body></html>`);
  return page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
}

// the sizes the bundle lists, and the layers of the tray icon
writeFileSync(join(icons, '32x32.png'), await png(small, 32));
writeFileSync(join(icons, '128x128.png'), await png(large, 128));
writeFileSync(join(icons, '128x128@2x.png'), await png(large, 256));
writeFileSync(join(icons, 'icon.png'), await png(large, 512));
writeFileSync(join(icons, 'tray-frames.png'), await png(frames, 32));
writeFileSync(join(icons, 'tray-arrow.png'), await png(arrow, 32));

// icon.ico: PNG pictures of 16, 24, 32 (small drawing) and 48, 64, 256 (large drawing)
const entries = [];
for (const [size, svg] of [[16, small], [24, small], [32, small], [48, large], [64, large], [256, large]]) {
  entries.push({ size, data: await png(svg, size) });
}
await browser.close();

const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0); header.writeUInt16LE(1, 2); header.writeUInt16LE(entries.length, 4);
let offset = 6 + 16 * entries.length;
const dirs = entries.map((e) => {
  const d = Buffer.alloc(16);
  d.writeUInt8(e.size === 256 ? 0 : e.size, 0);
  d.writeUInt8(e.size === 256 ? 0 : e.size, 1);
  d.writeUInt16LE(1, 4); d.writeUInt16LE(32, 6);
  d.writeUInt32LE(e.data.length, 8); d.writeUInt32LE(offset, 12);
  offset += e.data.length;
  return d;
});
writeFileSync(join(icons, 'icon.ico'), Buffer.concat([header, ...dirs, ...entries.map((e) => e.data)]));
console.log('icons written to', icons);
