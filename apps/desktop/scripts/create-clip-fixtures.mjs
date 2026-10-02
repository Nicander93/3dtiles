import { mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createMergeFixture } from './create-merge-fixtures.mjs';

const root = resolve(process.argv[2] || '../../.cache/clip-manual-fixtures');
await mkdir(root, { recursive: true });
const input = await createMergeFixture(root, 'geographic-triangle', 0, true);
const region = { type: 'rectangle', bounds: [0, 0, 0.0003, 0.0003] };
const geojson = { type: 'Polygon', coordinates: [[[0,0],[0.0003,0],[0.0003,0.0003],[0,0.0003],[0,0]]] };
await writeFile(join(root, 'region.json'), JSON.stringify(region, null, 2));
await writeFile(join(root, 'region.geojson'), JSON.stringify(geojson, null, 2));
console.log(JSON.stringify({ input, output: join(root, 'cropped'), region, geojson: join(root, 'region.geojson') }, null, 2));
