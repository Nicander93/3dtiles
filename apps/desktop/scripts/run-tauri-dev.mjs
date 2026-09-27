import { spawnSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoDir = resolve(appDir, '..', '..');
const extension = process.platform === 'win32' ? '.exe' : '';
const processor = resolve(repoDir, 'target', 'debug', `processor${extension}`);
const topRebuild = resolve(repoDir, 'target', 'debug', `top_rebuild${extension}`);

run('cargo', ['build', '-p', 'processor'], repoDir);
run('cargo', ['build', '-p', 'top_rebuild', '--bin', 'top_rebuild'], repoDir);

const env = {
  ...process.env,
  GEOFORGE_PROCESSOR: processor,
  GEOFORGE_TOP_REBUILD: topRebuild,
};

if (process.platform === 'win32') {
  const override = process.env.GEOFORGE_3DTILE?.trim();
  if (override) {
    const converter = resolve(override);
    if (!existsSync(converter)) {
      throw new Error(`GEOFORGE_3DTILE does not exist: ${converter}`);
    }
    const runtimeDir = resolve(repoDir, '.cache', 'dev-runtime', 'converter');
    const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') ?? 'PATH';
    env[pathKey] = `${dirname(converter)};${runtimeDir};${env[pathKey] ?? ''}`;
    env.GEOFORGE_3DTILE = converter;
    console.log('Dev converter source: GEOFORGE_3DTILE override');
  } else {
    env.GEOFORGE_3DTILE = prepareWindowsConverter();
    console.log('Dev converter source: pinned release');
  }
}

console.log(`Dev processor: ${env.GEOFORGE_PROCESSOR}`);
if (env.GEOFORGE_3DTILE) console.log(`Dev converter: ${env.GEOFORGE_3DTILE}`);

if (process.argv.includes('--prepare-only')) process.exit(0);

const tauri = resolve(appDir, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
if (!existsSync(tauri)) throw new Error('Tauri CLI is missing; run npm install in apps/desktop.');
const child = spawn(process.execPath, [tauri, 'dev', ...process.argv.slice(2)], {
  cwd: appDir,
  env,
  stdio: 'inherit',
});
child.on('error', error => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code, signal) => {
  process.exitCode = code ?? (signal ? 1 : 0);
});

function prepareWindowsConverter() {
  const pin = JSON.parse(readFileSync(resolve(repoDir, 'third_party', '3dtiles-converter.json'), 'utf8'));
  const version = pin.version;
  const sha256 = pin.windowsX64?.sha256?.toLowerCase();
  if (!version || !/^[a-f0-9]{64}$/.test(sha256 ?? '')) {
    throw new Error('Converter manifest must pin a Windows release and SHA256.');
  }

  const cacheDir = resolve(repoDir, '.cache', 'dev-runtime', 'converter');
  const converter = resolve(cacheDir, '_3dtile.exe');
  const stampPath = resolve(cacheDir, '.geoforge-dev-pin.json');
  let stamp;
  try {
    stamp = JSON.parse(readFileSync(stampPath, 'utf8'));
  } catch {
    stamp = null;
  }

  const cached = stamp?.version === version
    && stamp?.sha256 === sha256
    && existsSync(converter)
    && stamp?.converterSha256 === fileSha256(converter);
  if (!cached) {
    const script = resolve(appDir, 'scripts', 'prepare-converter.ps1');
    run('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', script, '-OutDir', cacheDir], appDir);
    writeFileSync(stampPath, `${JSON.stringify({ version, sha256, converterSha256: fileSha256(converter) })}\n`);
  }

  return converter;
}

function fileSha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function run(command, args, cwd) {
  const executable = process.platform === 'win32' && command === 'cargo' ? 'cargo.exe' : command;
  const result = spawnSync(executable, args, { cwd, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${executable} failed with exit code ${result.status ?? 'unknown'}`);
}
