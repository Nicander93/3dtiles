import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve, sep } from 'node:path';
import { inflateSync } from 'node:zlib';

function foregroundFraction(png, background) {
  const width = png.readUInt32BE(16), height = png.readUInt32BE(20);
  const channels = png[25] === 6 ? 4 : 3;
  expect(png[24]).toBe(8); expect([2, 6]).toContain(png[25]);
  const chunks = [];
  for (let offset = 8; offset < png.length;) {
    const length = png.readUInt32BE(offset);
    if (png.toString('ascii', offset + 4, offset + 8) === 'IDAT') chunks.push(png.subarray(offset + 8, offset + 8 + length));
    offset += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(chunks)), stride = width * channels;
  let previous = Buffer.alloc(stride), foreground = 0;
  const paeth = (a, b, c) => { const p = a + b - c; return Math.abs(p - a) <= Math.abs(p - b) && Math.abs(p - a) <= Math.abs(p - c) ? a : Math.abs(p - b) <= Math.abs(p - c) ? b : c; };
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)], row = Buffer.alloc(stride);
    expect(filter).toBeLessThanOrEqual(4);
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? row[i - channels] : 0, b = previous[i], c = i >= channels ? previous[i - channels] : 0;
      row[i] = (raw[y * (stride + 1) + 1 + i] + [0, a, b, Math.floor((a + b) / 2), paeth(a, b, c)][filter]) & 255;
    }
    for (let x = 0; x < width; x++) if (background.some((v, i) => Math.abs(row[x * channels + i] - v) > 20)) foreground++;
    previous = row;
  }
  return foreground / (width * height);
}

// Optional data-bearing acceptance: never substitute a synthetic success when
// the real conversion audit outputs have not been supplied.
const output = process.env.GEOFORGE_AUDIT_OUTPUT;
test.describe.configure({ timeout: 120000 });
for (const name of ['osgb-native-keep', 'osgb-native-ktx2', 'process-rebuild-grid']) {
  test(`Cesium renders audited ${name} output`, async ({ page }) => {
    test.skip(!output, 'Run scripts/audit-existing-features.py and supply GEOFORGE_AUDIT_OUTPUT');
    const root = resolve(output, name);
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(message.text());
    });
    await page.route((url) => url.pathname.startsWith('/audited/'), async (route) => {
      const relative = decodeURIComponent(new URL(route.request().url()).pathname.slice('/audited/'.length));
      const file = resolve(root, relative);
      if (!file.startsWith(root + sep)) return route.fulfill({ status: 403 });
      try {
        await route.fulfill({ body: await readFile(file), contentType: file.endsWith('.json') ? 'application/json' : 'application/octet-stream' });
      } catch { await route.fulfill({ status: 404 }); }
    });
    const enu = name.startsWith('process-') ? '&enu=35,117,0' : '';
    await page.goto(`/cesium-preview.html?tileset=${encodeURIComponent('/audited/tileset.json')}${enu}`);
    await expect.poll(() => page.evaluate(() => {
      const tileset = window.__geoforgePreview?.tileset;
      return Boolean(tileset && !tileset.isDestroyed());
    }), { timeout: 60000 }).toBe(true);
    expect(errors).toEqual([]);
    const evidence = await page.evaluate(() => {
      const stats = window.__geoforgePreview.tileset._statistics;
      return { triangles: stats.numberOfTrianglesSelected, readyTiles: stats.numberOfTilesWithContentReady };
    });
    // Legacy B3DM content can report zero triangle statistics while rendering.
    // Inspect the central canvas pixels, excluding the status and attribution UI.
    const viewport = page.viewportSize();
    const screenshot = await page.screenshot({ clip: { x: viewport.width * 0.2, y: viewport.height * 0.2, width: Math.floor(viewport.width * 0.6), height: Math.floor(viewport.height * 0.6) } });
    const background = await page.evaluate(() => {
      const color = window.__geoforgePreview.viewer.scene.backgroundColor;
      return [color.red, color.green, color.blue].map((v) => Math.round(v * 255));
    });
    evidence.foregroundFraction = foregroundFraction(screenshot, background);
    expect(evidence.foregroundFraction).toBeGreaterThan(0.02);
    await test.info().attach('rendered-model', { body: screenshot, contentType: 'image/png' });
    await test.info().attach('render-evidence', { body: JSON.stringify(evidence), contentType: 'application/json' });
  });
}
