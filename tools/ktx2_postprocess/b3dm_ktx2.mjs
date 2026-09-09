#!/usr/bin/env node
/**
 * Post-process 3D Tiles B3DM/GLB textures -> KTX2 ETC1S (KHR_texture_basisu)
 * via gltf-transform etc1s + KTX-Software toktx. No newer _3dtile required.
 *
 * Old fanvanzh keep-convert embeds KHR_techniques_webgl as required; we demote
 * it to optional so gltf-transform can rewrite textures.
 */
import fs from 'fs';
import path from 'path';
import os from 'os';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const DEFAULT_GLTF_TRANSFORM =
  '/workspace/tools/gltf-transform/node_modules/.bin/gltf-transform';
const DEFAULT_TOKTX_DIR =
  '/workspace/tools/ktx/KTX-Software-4.4.2-Linux-x86_64/bin';

function envPath() {
  const toktxDir = process.env.GEOFORGE_TOKTX_DIR || DEFAULT_TOKTX_DIR;
  const parts = [toktxDir, '/usr/bin', '/bin', '/usr/sbin', '/sbin'];
  for (const p of (process.env.PATH || '').split(':')) {
    if (p && !p.toLowerCase().includes('conda') && !parts.includes(p)) parts.push(p);
  }
  const env = { ...process.env, PATH: parts.join(':') };
  delete env.CONDA_PREFIX;
  delete env.CONDA_DEFAULT_ENV;
  delete env.PYTHONPATH;
  return env;
}

function resolveGltfTransform() {
  const cand = process.env.GEOFORGE_GLTF_TRANSFORM || DEFAULT_GLTF_TRANSFORM;
  if (fs.existsSync(cand)) return cand;
  throw new Error(`gltf-transform not found at ${cand}`);
}

function demoteTechniquesWebgl(glbBuf) {
  if (glbBuf.toString('ascii', 0, 4) !== 'glTF') throw new Error('not glb');
  const jsonLen = glbBuf.readUInt32LE(12);
  const jsonStart = 20;
  const jsonRaw = glbBuf.toString('utf8', jsonStart, jsonStart + jsonLen).replace(/\0+$/, '');
  const json = JSON.parse(jsonRaw);
  const req = json.extensionsRequired || [];
  if (!req.includes('KHR_techniques_webgl')) return glbBuf;
  json.extensionsRequired = req.filter((x) => x !== 'KHR_techniques_webgl');
  if (!(json.extensionsUsed || []).includes('KHR_techniques_webgl')) {
    json.extensionsUsed = [...(json.extensionsUsed || []), 'KHR_techniques_webgl'];
  }
  const jsonBuf = Buffer.from(JSON.stringify(json));
  const pad = (4 - (jsonBuf.length % 4)) % 4;
  const jsonChunk = Buffer.concat([jsonBuf, Buffer.alloc(pad, 0x20)]);
  const binStart = jsonStart + jsonLen;
  const rest = glbBuf.subarray(binStart);
  const newTotal = 12 + 8 + jsonChunk.length + rest.length;
  const out = Buffer.alloc(newTotal);
  glbBuf.copy(out, 0, 0, 12);
  out.writeUInt32LE(newTotal, 8);
  out.writeUInt32LE(jsonChunk.length, 12);
  out.writeUInt32LE(0x4e4f534a, 16);
  jsonChunk.copy(out, 20);
  rest.copy(out, 20 + jsonChunk.length);
  return out;
}

function parseB3dm(buf) {
  if (buf.length < 28 || buf.toString('ascii', 0, 4) !== 'b3dm') {
    throw new Error('not a b3dm');
  }
  const version = buf.readUInt32LE(4);
  const ftJson = buf.readUInt32LE(12);
  const ftBin = buf.readUInt32LE(16);
  const btJson = buf.readUInt32LE(20);
  const btBin = buf.readUInt32LE(24);
  if (version !== 1) throw new Error(`unsupported b3dm version ${version}`);
  let offset = 28 + ftJson + ftBin + btJson + btBin;
  let headerAndTables = buf.subarray(0, offset);
  let glb = buf.subarray(offset);
  if (glb.toString('ascii', 0, 4) !== 'glTF') {
    const idx = glb.indexOf(Buffer.from('glTF'));
    if (idx < 0) throw new Error('GLB not found in b3dm');
    headerAndTables = buf.subarray(0, offset + idx);
    glb = glb.subarray(idx);
  }
  return { headerAndTables, glb };
}

function packB3dm(headerAndTables, glb) {
  const hdr = Buffer.from(headerAndTables);
  hdr.writeUInt32LE(hdr.length + glb.length, 8);
  return Buffer.concat([hdr, glb]);
}

function runEtc1s(gltfBin, src, dst, jobs = 1, quality = 128) {
  const r = spawnSync(
    gltfBin,
    ['etc1s', src, dst, '--jobs', String(jobs), '--quality', String(quality)],
    { encoding: 'utf8', env: envPath(), maxBuffer: 20 * 1024 * 1024 }
  );
  if (r.status !== 0) {
    throw new Error(
      `gltf-transform etc1s failed rc=${r.status}\nstdout=${(r.stdout || '').slice(-2000)}\nstderr=${(r.stderr || '').slice(-2000)}`
    );
  }
}

function hasKtx2Marker(filePath) {
  const st = fs.statSync(filePath);
  const fd = fs.openSync(filePath, 'r');
  const n = Math.min(st.size, 2_000_000);
  const buf = Buffer.alloc(n);
  fs.readSync(fd, buf, 0, n, 0);
  fs.closeSync(fd);
  return (
    buf.includes(Buffer.from('KHR_texture_basisu')) ||
    buf.includes(Buffer.from('image/ktx2')) ||
    buf.includes(Buffer.from('.ktx2'))
  );
}

function processGlb(gltfBin, filePath, jobs, quality) {
  const demoted = demoteTechniquesWebgl(fs.readFileSync(filePath));
  const tmpIn = filePath + '.demote.glb';
  const tmpOut = filePath + '.ktx2tmp.glb';
  try {
    fs.writeFileSync(tmpIn, demoted);
    runEtc1s(gltfBin, tmpIn, tmpOut, jobs, quality);
    fs.renameSync(tmpOut, filePath);
  } finally {
    for (const p of [tmpIn, tmpOut]) if (fs.existsSync(p)) fs.unlinkSync(p);
  }
}

function processB3dm(gltfBin, filePath, jobs, quality) {
  const data = fs.readFileSync(filePath);
  const { headerAndTables, glb } = parseB3dm(data);
  const td = fs.mkdtempSync(path.join(os.tmpdir(), 'b3dm_ktx2_'));
  try {
    const glbIn = path.join(td, 'in.glb');
    const glbOut = path.join(td, 'out.glb');
    fs.writeFileSync(glbIn, demoteTechniquesWebgl(glb));
    runEtc1s(gltfBin, glbIn, glbOut, jobs, quality);
    const newGlb = fs.readFileSync(glbOut);
    fs.writeFileSync(filePath, packB3dm(headerAndTables, newGlb));
  } finally {
    fs.rmSync(td, { recursive: true, force: true });
  }
}

function* walkTiles(root) {
  const stack = [root];
  while (stack.length) {
    const cur = stack.pop();
    let entries;
    try {
      entries = fs.readdirSync(cur, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const ent of entries) {
      const p = path.join(cur, ent.name);
      if (ent.isDirectory()) stack.push(p);
      else if (/\.(b3dm|glb|gltf)$/i.test(ent.name)) yield p;
    }
  }
}

function processTree(root, { jobs = 1, quality = 128, limit = null } = {}) {
  const gltfBin = resolveGltfTransform();
  const toktx = path.join(process.env.GEOFORGE_TOKTX_DIR || DEFAULT_TOKTX_DIR, 'toktx');
  if (!fs.existsSync(toktx)) throw new Error(`toktx missing: ${toktx}`);

  const results = {
    root,
    processed: 0,
    skipped: 0,
    failed: 0,
    errors: [],
    samples: [],
  };
  let count = 0;
  for (const filePath of [...walkTiles(root)].sort()) {
    if (limit != null && count >= limit) break;
    if (hasKtx2Marker(filePath)) {
      results.skipped += 1;
      continue;
    }
    try {
      if (filePath.toLowerCase().endsWith('.b3dm')) {
        processB3dm(gltfBin, filePath, jobs, quality);
      } else {
        processGlb(gltfBin, filePath, jobs, quality);
      }
      results.processed += 1;
      count += 1;
      if (results.samples.length < 8) results.samples.push(filePath);
      if (!hasKtx2Marker(filePath)) {
        results.errors.push(`${filePath}: processed but no KTX2 marker`);
      }
    } catch (e) {
      results.failed += 1;
      results.errors.push(`${filePath}: ${e.message || e}`);
      if (results.failed >= 5 && results.processed === 0) break;
    }
  }
  return results;
}

function main(argv) {
  const args = { input: null, jobs: 1, quality: 128, limit: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if ((a === '-i' || a === '--input') && argv[i + 1]) args.input = argv[++i];
    else if (a === '--jobs' && argv[i + 1]) args.jobs = Number(argv[++i]);
    else if (a === '--quality' && argv[i + 1]) args.quality = Number(argv[++i]);
    else if (a === '--limit' && argv[i + 1]) args.limit = Number(argv[++i]);
  }
  if (!args.input) {
    console.error('Usage: b3dm_ktx2.mjs -i <tileset-root|file> [--jobs 1] [--limit N]');
    process.exit(2);
  }
  const target = path.resolve(args.input);
  if (!fs.existsSync(target)) {
    console.error(`input not found: ${target}`);
    process.exit(2);
  }
  const gltfBin = resolveGltfTransform();
  if (fs.statSync(target).isFile()) {
    try {
      if (target.toLowerCase().endsWith('.b3dm')) processB3dm(gltfBin, target, args.jobs, args.quality);
      else processGlb(gltfBin, target, args.jobs, args.quality);
      console.log(
        JSON.stringify({ processed: 1, file: target, hasMarker: hasKtx2Marker(target) }, null, 2)
      );
      process.exit(hasKtx2Marker(target) ? 0 : 1);
    } catch (e) {
      console.log(JSON.stringify({ processed: 0, error: String(e.message || e) }, null, 2));
      process.exit(1);
    }
  }
  const summary = processTree(target, args);
  console.log(JSON.stringify(summary, null, 2));
  process.exit(summary.processed > 0 ? 0 : 1);
}

const isMain =
  process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) main(process.argv.slice(2));
