import { cp, mkdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const source = new URL('../node_modules/cesium/Build/Cesium/', import.meta.url);
const destination = new URL('../public/vendor/cesium/', import.meta.url);

// Copy the complete runtime: workers and assets resolve relative to CESIUM_BASE_URL.
await readFile(new URL('Cesium.js', source));
await mkdir(destination, { recursive: true });
await cp(source, destination, { recursive: true });
await cp(new URL('../node_modules/cesium/LICENSE.md', import.meta.url),
  new URL('LICENSE.md', destination));
console.log(`Cesium runtime ready: ${fileURLToPath(destination)}`);
