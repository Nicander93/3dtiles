import { mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** A renderable triangle dataset used by browser e2e and manual acceptance. */
export async function createMergeFixture(root, name, offset, geographic = false) {
  const dir = join(root, name);
  await mkdir(dir);
  const gltf = { asset: { version: '2.0' }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, material: 0 }] }],
    materials: [{ doubleSided: true, pbrMetallicRoughness: { baseColorFactor: offset >= 0 ? [0.9,0.3,0.1,1] : [0.1,0.4,0.9,1], metallicFactor: 0, roughnessFactor: 1 } }],
    buffers: [{ byteLength: 36 }], bufferViews: [{ buffer: 0, byteLength: 36 }],
    accessors: [{ bufferView: 0, componentType: 5126, count: 3, type: 'VEC3', min: [0,0,-1], max: [1,0,0] }] };
  const json = JSON.stringify(gltf);
  const text = Buffer.from(json.padEnd(Math.ceil(Buffer.byteLength(json) / 4) * 4, ' '));
  const header = Buffer.alloc(20);
  header.write('glTF'); header.writeUInt32LE(2, 4); header.writeUInt32LE(28 + text.length + 36, 8);
  header.writeUInt32LE(text.length, 12); header.write('JSON', 16);
  const binHeader = Buffer.alloc(8); binHeader.writeUInt32LE(36); binHeader.writeUInt32LE(0x004e4942, 4);
  const vertices = Buffer.alloc(36);
  [0,0,0,1,0,0,0,0,-1].forEach((n, i) => vertices.writeFloatLE(n, i * 4));
  await writeFile(join(dir, 'tile.glb'), Buffer.concat([header, text, binHeader, vertices]));
  const transform = geographic
    ? [0,100,0,0,0,0,100,0,100,0,0,0,6378147,offset,0,1]
    : [1,0,0,0,0,1,0,0,0,0,1,0,offset,0,0,1];
  await writeFile(join(dir, 'tileset.json'), JSON.stringify({ asset: { version: '1.0' }, geometricError: 8,
    root: { boundingVolume: { box: [0,0,0,1,0,0,0,1,0,0,0,1] }, geometricError: 0,
      refine: 'REPLACE', transform, content: { uri: 'tile.glb' } } }, null, 2));
  return dir;
}

if (resolve(process.argv[1] || '') === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] || '../../.cache/merge-manual-fixtures');
  await mkdir(root, { recursive: true });
  const inputs = [await createMergeFixture(root, 'input-A', 100, true), await createMergeFixture(root, 'input-B', -100, true)];
  console.log(JSON.stringify({ inputs, output: join(root, 'merged') }, null, 2));
}
