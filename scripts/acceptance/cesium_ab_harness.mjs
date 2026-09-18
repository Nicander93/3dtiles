#!/usr/bin/env node
/**
 * Phase 17 — Cesium automated A/B harness (plan §15).
 *
 * Compares baseline (convert-only) vs candidate (TopRebuild) under fixed cameras.
 * Uses system Chrome + puppeteer-core; Cesium 1.125 from apps/desktop/public/vendor/cesium.
 *
 * Usage:
 *   node scripts/acceptance/cesium_ab_harness.mjs \
 *     --baseline /path/to/baseline \
 *     --candidate /path/to/rebuild \
 *     --out acceptance-results/phase17_ab_smoke
 *
 * Single-arm (wiring smoke): omit --candidate.
 */
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import puppeteer from 'puppeteer-core';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(__dirname, '../..');
const CESIUM_ROOT = path.join(REPO, 'apps/desktop/public/vendor/cesium');
const PROBE = path.join(__dirname, 'cesium_ab_probe.html');
const CHROME = process.env.CHROME_PATH || '/usr/bin/google-chrome';

const CAMERAS = [
  { name: 'far-overview', range: 40 },
  { name: 'mid', range: 10 },
  { name: 'near', range: 2 },
];

function parseArgs(argv) {
  const out = {
    baseline: null,
    candidate: null,
    outDir: path.join(REPO, 'acceptance-results', 'phase17_ab'),
    sse: 16,
    settleMs: 10000,
    port: 0,
  };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    const next = argv[i + 1];
    if (a === '--baseline') { out.baseline = path.resolve(next); i++; }
    else if (a === '--candidate') { out.candidate = path.resolve(next); i++; }
    else if (a === '--out') { out.outDir = path.resolve(next); i++; }
    else if (a === '--sse') { out.sse = Number(next); i++; }
    else if (a === '--settle-ms') { out.settleMs = Number(next); i++; }
    else if (a === '--help' || a === '-h') { out.help = true; }
  }
  return out;
}

function contentType(p) {
  if (p.endsWith('.html')) return 'text/html; charset=utf-8';
  if (p.endsWith('.js')) return 'text/javascript; charset=utf-8';
  if (p.endsWith('.css')) return 'text/css; charset=utf-8';
  if (p.endsWith('.json')) return 'application/json';
  if (p.endsWith('.wasm')) return 'application/wasm';
  if (p.endsWith('.png')) return 'image/png';
  if (p.endsWith('.jpg') || p.endsWith('.jpeg')) return 'image/jpeg';
  if (p.endsWith('.ktx2')) return 'application/octet-stream';
  if (p.endsWith('.b3dm') || p.endsWith('.glb') || p.endsWith('.bin')) return 'application/octet-stream';
  return 'application/octet-stream';
}

function startServer(roots) {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      try {
        const u = new URL(req.url || '/', 'http://127.0.0.1');
        let rel = decodeURIComponent(u.pathname);
        if (rel === '/' || rel === '') rel = '/probe.html';
        let filePath = null;
        if (rel === '/probe.html') filePath = PROBE;
        else if (rel.startsWith('/cesium/')) {
          filePath = path.join(CESIUM_ROOT, rel.slice('/cesium/'.length));
        } else {
          for (const [prefix, root] of roots) {
            if (rel.startsWith(prefix)) {
              filePath = path.join(root, rel.slice(prefix.length));
              break;
            }
          }
        }
        if (!filePath || !fs.existsSync(filePath) || fs.statSync(filePath).isDirectory()) {
          res.writeHead(404); res.end('not found ' + rel); return;
        }
        res.writeHead(200, {
          'Content-Type': contentType(filePath),
          'Access-Control-Allow-Origin': '*',
          'Cache-Control': 'no-store',
        });
        fs.createReadStream(filePath).pipe(res);
      } catch (e) {
        res.writeHead(500); res.end(String(e));
      }
    });
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address();
      resolve({ server, port: addr.port });
    });
    server.on('error', reject);
  });
}

async function measureArm(browser, baseUrl, armPrefix, tilesetPath, label, opts) {
  const results = [];
  for (const cam of CAMERAS) {
    const page = await browser.newPage();
    await page.setViewport({ width: 1280, height: 720, deviceScaleFactor: 1 });
    const tilesetUrl = `${baseUrl}${armPrefix}/tileset.json`;
    const url =
      `${baseUrl}/probe.html?tileset=${encodeURIComponent(tilesetUrl)}` +
      `&label=${encodeURIComponent(label + ':' + cam.name)}` +
      `&range=${cam.range}&sse=${opts.sse}&settleMs=${opts.settleMs}`;
    const client = await page.createCDPSession();
    await client.send('Network.enable');
    let netBytes = 0;
    let netRequests = 0;
    let failed = 0;
    let contentReqs = 0;
    client.on('Network.loadingFinished', (e) => {
      netRequests += 1;
      netBytes += e.encodedDataLength || 0;
    });
    client.on('Network.loadingFailed', () => { failed += 1; });
    client.on('Network.responseReceived', (e) => {
      const u = e.response?.url || '';
      if (/\.(b3dm|i3dm|pnts|cmpt|glb|gltf|json|ktx2)(\?|$)/i.test(u)) contentReqs += 1;
    });

    await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 120000 });
    const metrics = await page.waitForFunction(
      () => window.__GEOFORGE_AB__ && window.__GEOFORGE_AB__.ready === true,
      { timeout: opts.settleMs + 60000 }
    ).then(() => page.evaluate(() => window.__GEOFORGE_AB__));

    const shot = path.join(opts.outDir, 'screenshots', `${label}_${cam.name}.png`);
    fs.mkdirSync(path.dirname(shot), { recursive: true });
    await page.screenshot({ path: shot, type: 'png' });

    const row = {
      arm: label,
      camera: cam.name,
      rangeMul: cam.range,
      tilesetPath,
      screenshot: path.relative(opts.outDir, shot),
      probe: metrics,
      network: {
        requestCount: netRequests,
        contentRequestCount: contentReqs,
        bytesTransferred: netBytes,
        failedRequestCount: failed,
      },
    };
    results.push(row);
    await page.close();
  }
  return results;
}

function pctImprove(base, cand) {
  if (base == null || cand == null || base === 0) return null;
  return ((base - cand) / base) * 100;
}

function evaluateFarGates(baselineRows, candidateRows) {
  const b = baselineRows.find((r) => r.camera === 'far-overview');
  const c = candidateRows.find((r) => r.camera === 'far-overview');
  if (!b || !c) return { applicable: false, reason: 'missing far-overview arm' };
  const baseReq = b.network.contentRequestCount || b.probe?.contentRequestCount || 0;
  const candReq = c.network.contentRequestCount || c.probe?.contentRequestCount || 0;
  const baseBytes = b.network.bytesTransferred || b.probe?.bytesTransferred || 0;
  const candBytes = c.network.bytesTransferred || c.probe?.bytesTransferred || 0;
  const baseTime = b.probe?.timeToOverviewReadyMs;
  const candTime = c.probe?.timeToOverviewReadyMs;
  const improvements = {
    contentRequestPct: pctImprove(baseReq, candReq),
    bytesPct: pctImprove(baseBytes, candBytes),
    timeToOverviewPct: pctImprove(baseTime, candTime),
  };
  const hits = [
    improvements.contentRequestPct != null && improvements.contentRequestPct >= 50,
    improvements.bytesPct != null && improvements.bytesPct >= 40,
    improvements.timeToOverviewPct != null && improvements.timeToOverviewPct >= 25,
  ].filter(Boolean).length;
  return {
    applicable: true,
    planRef: '04-v1-production-readiness-plan.md §15.4',
    baseline: { contentRequestCount: baseReq, bytesTransferred: baseBytes, timeToOverviewReadyMs: baseTime },
    candidate: { contentRequestCount: candReq, bytesTransferred: candBytes, timeToOverviewReadyMs: candTime },
    improvements,
    hitsOfThree: hits,
    pass: hits >= 2,
    note: hits >= 2
      ? 'Far-view benefit gate PASS (>=2 of 3 thresholds)'
      : 'Far-view benefit gate FAIL — report honestly; do not invent benefit',
  };
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help || !args.baseline) {
    console.log(`Usage: node scripts/acceptance/cesium_ab_harness.mjs --baseline <dir> [--candidate <dir>] [--out <dir>] [--sse 16]`);
    process.exit(args.help ? 0 : 2);
  }
  if (!fs.existsSync(path.join(args.baseline, 'tileset.json'))) {
    console.error('baseline tileset.json missing:', args.baseline);
    process.exit(2);
  }
  if (args.candidate && !fs.existsSync(path.join(args.candidate, 'tileset.json'))) {
    console.error('candidate tileset.json missing:', args.candidate);
    process.exit(2);
  }
  if (!fs.existsSync(path.join(CESIUM_ROOT, 'Cesium.js'))) {
    console.error('Cesium vendor missing. Run: cd apps/desktop && npm run prepare:cesium');
    process.exit(2);
  }
  if (!fs.existsSync(CHROME)) {
    console.error('Chrome not found at', CHROME);
    process.exit(2);
  }

  fs.mkdirSync(args.outDir, { recursive: true });
  fs.mkdirSync(path.join(args.outDir, 'screenshots'), { recursive: true });

  const roots = [
    ['/baseline/', args.baseline],
  ];
  if (args.candidate) roots.push(['/candidate/', args.candidate]);

  const { server, port } = await startServer(roots);
  const baseUrl = `http://127.0.0.1:${port}`;
  console.log('[ab] server', baseUrl);
  console.log('[ab] chrome', CHROME);

  const browser = await puppeteer.launch({
    executablePath: CHROME,
    headless: 'new',
    args: [
      '--no-sandbox',
      '--disable-gpu',
      '--use-angle=swiftshader',
      '--enable-webgl',
      '--ignore-gpu-blocklist',
      '--window-size=1280,720',
    ],
  });

  const report = {
    schema: 'geoforge.acceptance.cesium_ab.v1',
    plan: 'Phase 17 / production-readiness §15',
    capturedAt: new Date().toISOString(),
    cesiumVendor: 'apps/desktop/public/vendor/cesium (1.125)',
    chrome: CHROME,
    sse: args.sse,
    settleMs: args.settleMs,
    baselinePath: args.baseline,
    candidatePath: args.candidate,
    arms: {},
    farViewGate: null,
  };

  try {
    report.arms.baseline = await measureArm(
      browser, baseUrl, '/baseline', args.baseline, 'baseline', args
    );
    fs.writeFileSync(
      path.join(args.outDir, 'cesium-baseline.json'),
      JSON.stringify(report.arms.baseline, null, 2)
    );

    if (args.candidate) {
      report.arms.candidate = await measureArm(
        browser, baseUrl, '/candidate', args.candidate, 'rebuild', args
      );
      fs.writeFileSync(
        path.join(args.outDir, 'cesium-rebuild.json'),
        JSON.stringify(report.arms.candidate, null, 2)
      );
      report.farViewGate = evaluateFarGates(report.arms.baseline, report.arms.candidate);
    } else {
      report.farViewGate = {
        applicable: false,
        reason: 'single-arm run (no --candidate); wiring/smoke only',
      };
    }
  } finally {
    await browser.close();
    server.close();
  }

  fs.writeFileSync(path.join(args.outDir, 'result.json'), JSON.stringify(report, null, 2));

  const md = [];
  md.push('# Cesium A/B result');
  md.push('');
  md.push(`Captured: ${report.capturedAt}`);
  md.push(`Baseline: \`${args.baseline}\``);
  md.push(`Candidate: \`${args.candidate || '(none — single-arm)'}\``);
  md.push(`SSE: ${args.sse}`);
  md.push('');
  if (report.farViewGate?.applicable) {
    md.push('## Far-view gate (§15.4)');
    md.push('');
    md.push(`- hits: **${report.farViewGate.hitsOfThree}/3**`);
    md.push(`- pass: **${report.farViewGate.pass}**`);
    md.push(`- request improve %: ${report.farViewGate.improvements.contentRequestPct?.toFixed?.(1)}`);
    md.push(`- bytes improve %: ${report.farViewGate.improvements.bytesPct?.toFixed?.(1)}`);
    md.push(`- time improve %: ${report.farViewGate.improvements.timeToOverviewPct?.toFixed?.(1)}`);
    md.push('');
    md.push(report.farViewGate.note);
  } else {
    md.push(`## Far-view gate`);
    md.push('');
    md.push(report.farViewGate?.reason || 'n/a');
  }
  md.push('');
  md.push('## Arms');
  for (const [arm, rows] of Object.entries(report.arms)) {
    md.push(`### ${arm}`);
    for (const r of rows) {
      md.push(
        `- ${r.camera}: tilesLoaded=${r.probe?.tilesLoaded} ` +
        `tOverview=${r.probe?.timeToOverviewReadyMs?.toFixed?.(0)}ms ` +
        `netContent=${r.network.contentRequestCount} bytes=${r.network.bytesTransferred} ` +
        `shot=${r.screenshot}`
      );
    }
  }
  fs.writeFileSync(path.join(args.outDir, 'result.md'), md.join('\n') + '\n');
  console.log(md.join('\n'));
  console.log('[ab] wrote', path.join(args.outDir, 'result.json'));
  if (report.farViewGate?.applicable && !report.farViewGate.pass) process.exitCode = 1;
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
