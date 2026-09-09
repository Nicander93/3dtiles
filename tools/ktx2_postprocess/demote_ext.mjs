import fs from 'fs';
const p = process.argv[2];
const buf = fs.readFileSync(p);
if (buf.toString('ascii',0,4) !== 'glTF') throw new Error('not glb');
const jsonLen = buf.readUInt32LE(12);
const jsonStart = 20;
const json = JSON.parse(buf.toString('utf8', jsonStart, jsonStart + jsonLen).replace(/\0+$/,''));
const req = json.extensionsRequired || [];
json.extensionsRequired = req.filter(x => x !== 'KHR_techniques_webgl');
if (!(json.extensionsUsed||[]).includes('KHR_techniques_webgl') && req.includes('KHR_techniques_webgl')) {
  json.extensionsUsed = [...(json.extensionsUsed||[]), 'KHR_techniques_webgl'];
}
const jsonBuf = Buffer.from(JSON.stringify(json));
const pad = (4 - (jsonBuf.length % 4)) % 4;
const jsonChunk = Buffer.concat([jsonBuf, Buffer.alloc(pad, 0x20)]);
const binStart = jsonStart + jsonLen;
const rest = buf.subarray(binStart); // includes BIN chunk header+data and any trailing
const newJsonChunkTotal = 8 + jsonChunk.length;
const newTotal = 12 + newJsonChunkTotal + (rest.length);
const out = Buffer.alloc(newTotal);
buf.copy(out, 0, 0, 12);
out.writeUInt32LE(newTotal, 8);
out.writeUInt32LE(jsonChunk.length, 12);
out.writeUInt32LE(0x4E4F534A, 16); // JSON
jsonChunk.copy(out, 20);
rest.copy(out, 20 + jsonChunk.length);
fs.writeFileSync(p, out);
console.log(JSON.stringify({ok:true, removedRequired:'KHR_techniques_webgl', newSize:newTotal}));
