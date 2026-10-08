import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import Cartesian3 from '../node_modules/@cesium/engine/Source/Core/Cartesian3.js';
import Matrix4 from '../node_modules/@cesium/engine/Source/Core/Matrix4.js';
import Quaternion from '../node_modules/@cesium/engine/Source/Core/Quaternion.js';
import Transforms from '../node_modules/@cesium/engine/Source/Core/Transforms.js';
import HeadingPitchRoll from '../node_modules/@cesium/engine/Source/Core/HeadingPitchRoll.js';

const execute=promisify(execFile);
const processor=resolve(process.env.GEOFORGE_AUDIT_PROCESSOR || `../../target/debug/processor${process.platform === 'win32' ? '.exe' : ''}`);
const converter=resolve(process.env.GEOFORGE_3DTILE || '../../dist/runtime/converter/_3dtile.exe');
const root=await mkdtemp(join(tmpdir(),'geoforge-coordinates-'));
const results=[];
const identity=()=>Matrix4.clone(Matrix4.IDENTITY);
const multiply=(a,b)=>Matrix4.multiply(a,b,new Matrix4());
const translation=(p)=>Matrix4.fromTranslation(Cartesian3.fromArray(p));
const up=Matrix4.fromArray([1,0,0,0,0,0,1,0,0,-1,0,0,0,0,0,1]);
function nodeMatrix(node) {
  return node.matrix ? Matrix4.fromArray(node.matrix) : Matrix4.fromTranslationQuaternionRotationScale(
    Cartesian3.fromArray(node.translation || [0,0,0]),Quaternion.unpack(node.rotation || [0,0,0,1]),Cartesian3.fromArray(node.scale || [1,1,1]));
}
async function worldVertices(path,parent=identity()) {
  const top=JSON.parse(await readFile(path,'utf8'));
  const points=[];
  async function tile(node,previous) {
    const world=multiply(previous,Matrix4.fromArray(node.transform || Matrix4.IDENTITY));
    const uri=node.content?.uri || node.content?.url;
    if (uri?.endsWith('.json')) points.push(...await worldVertices(resolve(dirname(path),uri),world));
    else if (uri) {
      const bytes=await readFile(resolve(dirname(path),uri)); let offset=0,rtc=[0,0,0];
      if (bytes.toString('ascii',0,4) === 'b3dm') {
        const lengths=[12,16,20,24].map((p)=>bytes.readUInt32LE(p));
        const ft=JSON.parse(bytes.toString('utf8',28,28+lengths[0]).trim());rtc=ft.RTC_CENTER || rtc;
        offset=28+lengths.reduce((a,b)=>a+b,0);
      }
      assert.equal(bytes.toString('ascii',offset,offset+4),'glTF');
      const size=bytes.readUInt32LE(offset+12), gltf=JSON.parse(bytes.toString('utf8',offset+20,offset+20+size));
      const binary=offset+28+size;
      const base=multiply(multiply(world,translation(rtc)),top.asset.gltfUpAxis === 'Z' ? identity() : up);
      async function walk(id,previous) {
        const node=gltf.nodes[id], matrix=multiply(previous,nodeMatrix(node));
        if (node.mesh !== undefined) for (const primitive of gltf.meshes[node.mesh].primitives) {
          const accessor=gltf.accessors[primitive.attributes.POSITION], view=gltf.bufferViews[accessor.bufferView];
          assert.equal(accessor.componentType,5126);
          for (let i=0;i<accessor.count;i++) {
            const start=binary+(view.byteOffset || 0)+(accessor.byteOffset || 0)+i*(view.byteStride || 12);
            const p=Cartesian3.fromArray([0,1,2].map((j)=>bytes.readFloatLE(start+j*4)));
            points.push(Matrix4.multiplyByPoint(matrix,p,new Cartesian3()));
          }
        }
        for (const child of node.children || []) await walk(child,matrix);
      }
      for (const node of gltf.scenes[gltf.scene || 0].nodes) await walk(node,base);
    }
    for (const child of node.children || []) await tile(child,world);
  }
  await tile(top.root,parent);return points;
}
async function check(name,vertices,model,georeference,expected,tolerance=0.02) {
  const dir=join(root,name);await mkdir(dir);const input=join(dir,'source.obj'),output=join(root,`${name}-output`);
  await writeFile(input,vertices.map((p)=>`v ${p.join(' ')}`).join('\n')+'\nf 1 2 3\n');
  const config={schemaVersion:1,taskId:name,operation:'convert-model',input:{path:input},output:{path:output},options:{model:{format:'obj',...model},georeference}};
  const task=join(root,`${name}.json`);await writeFile(task,JSON.stringify(config));
  await execute(processor,['run','--task',task],{env:{...process.env,GEOFORGE_3DTILE:converter},timeout:60000,maxBuffer:2*1024*1024});
  const actual=await worldVertices(join(output,'tileset.json'));
  assert.ok(actual.length >= 3,`${name}: no vertices`);
  const errors=expected.map((p)=>Math.min(...actual.map((q)=>Cartesian3.distance(q,p))));
  const reverse=actual.map((p)=>Math.min(...expected.map((q)=>Cartesian3.distance(q,p))));
  const error=Math.max(...errors,...reverse);assert.ok(error<tolerance,`${name}: ${error} m coordinate error`);
  results.push({name,maxErrorMeters:error});
}
try {
  const vertices=[[10,20,30],[12,20,30],[10,23,34]];
  for (const [unit,scale] of [['meters',1],['centimeters',0.01],['millimeters',0.001],['feet',0.3048]]) {
    for (const axes of ['zUpRightHanded','yUpRightHanded']) {
      const enu=vertices.map(([x,y,z])=>axes === 'zUpRightHanded' ? [x*scale,y*scale,z*scale] : [x*scale,-z*scale,y*scale]);
      await check(`${unit}-${axes}`,vertices,{unit,axes},{mode:'local'},enu.map((p)=>Cartesian3.fromArray(p)));
    }
  }
  for (const [lon,lat,h] of [[117,35,123],[0,0,0],[-73,-35,-20]]) {
    const frame=Transforms.eastNorthUpToFixedFrame(Cartesian3.fromDegrees(lon,lat,h));
    await check(`anchor-${lon}-${lat}`,vertices,{unit:'meters',axes:'zUpRightHanded'},
      {mode:'anchor',longitudeDeg:lon,latitudeDeg:lat,ellipsoidHeightM:h,pivot:'original'},
      vertices.map((p)=>Matrix4.multiplyByPoint(frame,Cartesian3.fromArray(p),new Cartesian3())));
  }
  for (const pivot of ['original','bottomCenter','boundingBoxCenter']) {
    const offset=pivot === 'original' ? [0,0,0] : [11,21.5,pivot === 'bottomCenter' ? 30 : 32];
    const hpr=new HeadingPitchRoll(40*Math.PI/180,-15*Math.PI/180,20*Math.PI/180);
    const frame=Transforms.headingPitchRollToFixedFrame(Cartesian3.fromDegrees(117,35,123),hpr);
    await check(`rotated-${pivot}`,vertices,{unit:'meters',axes:'zUpRightHanded'},
      {mode:'anchor',longitudeDeg:117,latitudeDeg:35,ellipsoidHeightM:123,pivot,headingDeg:40,pitchDeg:-15,rollDeg:20},
      vertices.map((p)=>Matrix4.multiplyByPoint(frame,Cartesian3.fromArray(p.map((v,i)=>v-offset[i])),new Cartesian3())));
  }
  const a=6378137, x=a*117*Math.PI/180,y=a*Math.log(Math.tan(Math.PI/4+35*Math.PI/360));
  for (const axisMapping of ['eastNorthHeight','northEastHeight']) {
    const points=[[0,0,0],[10,0,0],[0,10,0]];
    const offsets=axisMapping === 'eastNorthHeight' ? [x,y,123] : [y,x,123];
    const expected=points.map(([p,q,z])=>{if(axisMapping === 'northEastHeight') [p,q]=[q,p];
      return Cartesian3.fromDegrees((x+p)/a*180/Math.PI,(2*Math.atan(Math.exp((y+q)/a))-Math.PI/2)*180/Math.PI,123+z);});
    await check(`projected-${axisMapping}`,points,{unit:'meters',axes:'zUpRightHanded'},
      {mode:'projected',sourceCrs:'EPSG:3857',axisMapping,originOffset:offsets},expected);
  }
  const report=JSON.stringify({converter,results},null,2);
  if (process.env.GEOFORGE_AUDIT_REPORT) await writeFile(resolve(process.env.GEOFORGE_AUDIT_REPORT),report);
  console.log(report);
} finally { await rm(root,{recursive:true,force:true}); }
