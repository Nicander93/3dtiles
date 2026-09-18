import { access, constants, copyFile, mkdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoDir = resolve(appDir, '..', '..');
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
const extension = process.platform === 'win32' ? '.exe' : '';

runCargo(['build', '--release', '-p', 'processor']);
runCargo(['build', '--release', '-p', 'top_rebuild', '--bin', 'top_rebuild']);
await mkdir(resolve(appDir, 'src-tauri', 'binaries'), { recursive: true });
for (const name of ['processor', 'top_rebuild']) {
  const source = resolve(repoDir, 'target', 'release', `${name}${extension}`);
  const destination = resolve(appDir, 'src-tauri', 'binaries', `${name}-${triple}${extension}`);
  await copyFile(source, destination);
  console.log(`Sidecar ready: ${destination}`);
}

// Optional basisu sidecar (tauri.conf externalBin). Prefer prebuilt paths; do not fail package if absent
// (Windows package-windows.ps1 also stages basisu under resources/runtime/texture/).
{
  const candidates = [
    resolve(appDir, 'src-tauri', 'binaries', `basisu${extension}`),
    resolve(repoDir, 'vcpkg_installed', process.platform === 'win32' ? 'x64-windows' : 'x64-linux', 'tools', 'basisu', `basisu${extension}`),
    process.env.GEOFORGE_BASISU || '',
  ].filter(Boolean);
  let copied = false;
  for (const source of candidates) {
    try {
      await access(source, constants.R_OK);
      const destination = resolve(appDir, 'src-tauri', 'binaries', `basisu-${triple}${extension}`);
      await copyFile(source, destination);
      console.log(`Sidecar ready: ${destination}`);
      copied = true;
      break;
    } catch {
      // try next
    }
  }
  if (!copied) {
    console.warn('basisu sidecar not found — KTX2 externalBin may be missing at tauri build; Windows texture/ path still used when packaged');
  }
}

function runCargo(args) {
  const command = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
  const result = spawnSync(command, args, { cwd: repoDir, stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function hostTriple() {
  const arch = process.arch === 'arm64' ? 'aarch64' : 'x86_64';
  if (process.platform === 'win32') return `${arch}-pc-windows-msvc`;
  if (process.platform === 'darwin') return `${arch}-apple-darwin`;
  return `${arch}-unknown-linux-gnu`;
}
