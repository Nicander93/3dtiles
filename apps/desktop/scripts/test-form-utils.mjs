import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

async function loadTypeScript(relativePath) {
  const filename = fileURLToPath(new URL(relativePath, import.meta.url));
  const source = await readFile(filename, 'utf8');
  const { outputText, diagnostics } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
    reportDiagnostics: true,
  });
  assert.equal(diagnostics?.length ?? 0, 0, `${relativePath} should transpile`);
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
}

const errors = await loadTypeScript('../src/api/errorUtils.ts');
assert.equal(errors.friendlyError('xxx'), 'xxx');
assert.equal(errors.friendlyError(new Error('xxx')), 'xxx');
assert.equal(errors.friendlyError({ error: 'structured failure' }), 'structured failure');
assert.equal(errors.friendlyError({ detail: { message: 'nested' } }), 'nested');
assert.equal(errors.friendlyError({}), '发生未知错误，请查看任务日志。');

const forms = await loadTypeScript('../src/lib/formUtilsCore.ts');
const merge = await loadTypeScript('../src/lib/mergeTilesValidation.ts');
const clip = await loadTypeScript('../src/lib/clipTilesValidation.ts');
const drawing = await loadTypeScript('../src/lib/previewClipRegion.ts');
assert.deepEqual(drawing.drawnClipRegion('rectangle', [[0.001, 0.001], [0, 0]]), { type: 'rectangle', bounds: [0, 0, 0.001, 0.001] });
const drawnTriangle = [[0, 0], [0.001, 0], [0, 0.001]];
assert.deepEqual(drawing.drawnClipRegion('polygon', drawnTriangle), { type: 'Polygon', coordinates: [[...drawnTriangle, drawnTriangle[0]]] });
assert.equal(drawing.drawnClipRegion('polygon', drawnTriangle.toReversed()).type, 'Polygon');
assert.equal(drawing.drawnClipRegion('polygon', [[0, 0], [0.001, 0], [0.0002, 0.0002], [0, 0.001]]).type, 'Polygon');
assert.throws(() => drawing.drawnClipRegion('polygon', [[0,0],[0.001,0.001],[0,0.001],[0.001,0],[0,0.0005]]), /自交|面积/);
assert.throws(() => drawing.drawnClipRegion('polygon', [[0,0],[0.001,0],[0.001,0.001],[0.001,0],[0,0.001]]), /重复|自交/);
assert.throws(() => drawing.drawnClipRegion('polygon', [[0, 0], [0.001, 0.001], [0, 0.001], [0.001, 0]]));
assert.throws(() => drawing.drawnClipRegion('polygon', [...drawnTriangle, drawnTriangle[0]]), /重复/);
assert.throws(() => drawing.drawnClipRegion('rectangle', [[0, 0], [1, 1]]), /10 公里/);
assert.throws(() => drawing.drawnClipRegion('rectangle', [[179.999, 0], [-179.999, 0.001]]));
assert.throws(() => drawing.drawnClipRegion('rectangle', [[0, 81], [0.001, 81.001]]), /纬度/);
assert.throws(() => drawing.drawnClipRegion('rectangle', [[0, 0], [0, 0.001]]), /面积/);
assert.throws(() => drawing.drawnClipRegion('polygon', [[0, NaN], ...drawnTriangle]));
assert.throws(() => drawing.drawnClipRegion('polygon', Array(257).fill([0, 0])));
assert.equal(drawing.readDrawingMessage({ type: 'geoforge-region', mode: 'polygon', points: [[0, '0']], complete: true, active: true }), null);
assert.equal(drawing.readDrawingMessage({ type: 'geoforge-region', mode: 'polygon', points: [[0, Infinity]], complete: true, active: true }), null);
assert.equal(drawing.readDrawingMessage({ type: 'geoforge-region', mode: 'polygon', points: [], complete: 'yes', active: true }), null);
assert.equal(drawing.readDrawingMessage({ type: 'geoforge-region', mode: 'polygon', points: drawnTriangle, complete: true, active: 'yes' }), null);
assert.deepEqual(drawing.readDrawingMessage({ type: 'geoforge-region', mode: 'polygon', points: drawnTriangle, complete: true, active: true }).points, drawnTriangle);
assert.equal(drawing.readDrawingMessage({ type: 'geoforge-region', mode: 'polygon', points: drawnTriangle, complete: true, active: true, heights: [10] }), null);
assert.equal(drawing.readFlattenMessage({ type: 'geoforge-flatten-plane', regionKey: 'r', heightMeters: Infinity, initialHeightMeters: 0, hasUndo: false }), null);
assert.equal(drawing.readFlattenMessage({ type: 'geoforge-flatten-plane', regionKey: 'r', heightMeters: 10001, initialHeightMeters: 0, hasUndo: false }), null);
assert.equal(drawing.readFlattenMessage({ type: 'geoforge-flatten-plane', regionKey: 'r', heightMeters: '10', initialHeightMeters: 0, hasUndo: false }), null);
assert.equal(drawing.readFlattenMessage({ type: 'geoforge-flatten-plane', regionKey: 'r', heightMeters: -10, initialHeightMeters: 0, hasUndo: true }).heightMeters, -10);
const clipForm = { input: 'C:/data/a/tileset.json', output: 'D:/out/crop', name: '裁剪', mode: 'rectangle', bounds: ['-0.001', '-0.001', '0.001', '0.001'], geojson: '' };
assert.deepEqual(clip.clipTaskRequest(clipForm).options, { clip: { region: { type: 'rectangle', bounds: [-0.001, -0.001, 0.001, 0.001] } } });
assert.throws(() => clip.clipTaskRequest({ ...clipForm, output: 'C:/data/a/nested' }), /重叠/);
assert.throws(() => clip.clippingRegion({ ...clipForm, bounds: ['', '0', '1', '1'] }), /完整/);
assert.throws(() => clip.clippingRegion({ ...clipForm, bounds: ['1', '0', '0', '1'] }), /西经度/);
assert.throws(() => clip.clippingRegion({ ...clipForm, mode: 'polygon', geojson: '{' }));
const polygon = { type: 'Polygon', coordinates: [[[0, 0], [0.001, 0], [0, 0.001], [0, 0]]] };
assert.deepEqual(clip.clippingRegion({ ...clipForm, mode: 'polygon', geojson: JSON.stringify(polygon) }), polygon);
assert.deepEqual(clip.restoreClipRegion(clip.clippingRegion(clipForm)).bounds, clipForm.bounds);
assert.equal(clip.restoreClipRegion(polygon).mode, 'polygon');
const mergeForm = { inputs: ['C:/data/a/tileset.json', 'C:/data/b'], output: 'D:/out/merged', name: ' 合并 ' };
assert.equal(merge.validateMergeForm(mergeForm), null);
assert.equal(merge.validateMergeForm({ ...mergeForm, inputs: ['C:/data/a', 'c:\\data\\a\\tileset.json'] }), '输入列表包含重复的 Tileset。');
assert.equal(merge.validateMergeForm({ ...mergeForm, output: 'C:/data/b/nested/out' }), '成果目录不能与任何输入目录重叠。');
assert.equal(merge.validateMergeForm({ ...mergeForm, inputs: ['C:/data/a', ''] }), '请填写每份 Tileset 的路径。');
assert.equal(merge.validateMergeForm({ ...mergeForm, inputs: Array(65).fill('a') }), '请选择 2 到 64 份 Tileset。');
assert.deepEqual(merge.mergeTaskRequest(mergeForm), { operation: 'merge-tilesets', input: { path: 'C:/data/a/tileset.json' }, output: { path: 'D:/out/merged' }, taskName: '合并', options: { merge: { additionalInputs: ['C:/data/b'] } } });
assert.equal(forms.suggestOutputPath('C:\\data\\scene.osgb', '', '_tiles'), 'C:\\data\\scene.osgb_tiles');
assert.equal(forms.suggestOutputPath('/data/scene_tiles', '', '_tiles'), '/data/scene_tiles');
assert.equal(forms.suggestOutputPath('scene.osgb', 'D:\\out\\', '_process'), 'D:\\out\\scene.osgb_process');
assert.equal(forms.pathsEqual('C:\\DATA\\scene\\', 'c:/data/scene'), true);
assert.equal(forms.realProgressPercent({ completed: 1, total: 4 }), 25);
assert.equal(forms.realProgressPercent({ completed: 0, total: 0 }), null);

const modelScan = await loadTypeScript('../src/lib/modelScanGate.ts');
const readyScan = {
  scannedPath: 'C:\\Models\\building.obj',
  currentPath: 'c:/models/building.obj',
  debouncedPath: 'C:/models/building.obj',
  scannedTextureRoots: ['C:/Models/Textures'],
  textureRoots: ['c:\\models\\textures'],
  scanPending: false,
  scanValid: true,
  scannedFormat: 'obj',
  selectedFormat: 'obj',
};
assert.equal(modelScan.modelScanMatchesCurrentInput(readyScan), true);
assert.equal(modelScan.canSubmitModelScan(readyScan), true);
assert.equal(modelScan.modelScanMatchesCurrentInput({ ...readyScan, currentPath: 'C:/Models/changed.obj' }), false);
assert.equal(modelScan.modelScanMatchesCurrentInput({ ...readyScan, debouncedPath: 'C:/Models/old.obj' }), false);
assert.equal(modelScan.modelScanMatchesCurrentInput({ ...readyScan, textureRoots: [] }), false);
assert.equal(modelScan.canSubmitModelScan({ ...readyScan, scanPending: true }), false);
assert.equal(modelScan.canSubmitModelScan({ ...readyScan, scanValid: false }), false);
assert.equal(modelScan.canSubmitModelScan({ ...readyScan, selectedFormat: 'fbx' }), false);

const modelValidation = await loadTypeScript('../src/lib/modelConvertValidation.ts');
assert.equal(modelValidation.buildModelOutputPath('C:\\models\\building.fbx', 'C:\\Users\\PC\\Desktop\\', 'test-id'), 'C:\\Users\\PC\\Desktop\\building_tiles_test-id');
assert.equal(modelValidation.buildModelOutputPath('/models/city.obj', '/output', 'test-id'), '/output/city_tiles_test-id');
assert.equal(modelValidation.buildModelOutputPath('C:\\models\\building.fbx', '', 'test-id'), '');
const selectedExistingParent = modelValidation.buildModelOutputPath('D:\\models\\building.fbx', 'C:\\Users\\PC\\Desktop\\新建文件夹', 'test-id');
assert.equal(selectedExistingParent, 'C:\\Users\\PC\\Desktop\\新建文件夹\\building_tiles_test-id');
assert.equal(modelValidation.modelOutputPathError('D:\\models\\building.fbx', selectedExistingParent), null);
assert.match(modelValidation.modelOutputPathError('D:\\models\\building.fbx', 'D:\\models\\building_tiles_test-id'), /不能位于模型文件所在目录内/);
const validModelForm = {
  input: 'C:/models/building.fbx',
  output: 'C:/results/building_tiles_test-id',
  format: 'fbx',
  unit: 'fromMetadata',
  axes: 'fromMetadata',
  georeferenceMode: 'local',
  longitude: '',
  latitude: '',
  height: '',
  sourceCrs: '',
  originX: '0',
  originY: '0',
  originZ: '0',
  projectedGeoreferenceSupported: true,
};
assert.equal(modelValidation.modelConvertValidationError(validModelForm), null);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, output: 'c:\\MODELS\\building.fbx\\' }), /不能与模型文件相同/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, output: 'C:/models/tiles' }), /不能位于模型文件所在目录内/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, output: 'C:/models' }), /不能位于模型文件所在目录内/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, output: 'C:/' }), /不能包含模型文件所在目录/);
assert.equal(modelValidation.modelConvertValidationError({ ...validModelForm, output: 'C:/models-other/tiles' }), null);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, format: 'obj' }), /明确选择/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, georeferenceMode: 'anchor', longitude: '181', latitude: '0', height: '0' }), /经度范围/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, georeferenceMode: 'anchor', longitude: 'NaN', latitude: '0', height: '0' }), /有效的经度/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, georeferenceMode: 'projected' }), /源 CRS/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, georeferenceMode: 'projected', sourceCrs: 'EPSG:4547', projectedGeoreferenceSupported: false }), /不支持/);
assert.match(modelValidation.modelConvertValidationError({ ...validModelForm, georeferenceMode: 'projected', sourceCrs: 'EPSG:4547', originX: 'Infinity' }), /有限数值/);

const textureCaps = await loadTypeScript('../src/lib/textureCaps.ts');
const unavailableCaps = {
  textureModes: [
    { mode: 'ktx2-etc1s', supported: false },
    { mode: 'ktx2-uastc', supported: false },
  ],
  postprocessBasisu: { available: false },
};
const availableCaps = {
  textureModes: [
    { mode: 'ktx2-etc1s', supported: true },
    { mode: 'ktx2-uastc', supported: true, processTileset: { supported: true } },
  ],
  postprocessBasisu: { available: false },
};
assert.equal(textureCaps.textureModeEnabled(null, 'keep'), true);
assert.equal(textureCaps.textureModeEnabled(null, 'ktx2-etc1s'), false);
assert.equal(textureCaps.textureModeEnabled(unavailableCaps, 'ktx2-uastc'), false);
assert.equal(textureCaps.textureModeEnabled(availableCaps, 'ktx2-etc1s'), true);
assert.equal(textureCaps.textureModeEnabled(availableCaps, 'ktx2-uastc', true), true);

console.log('desktop pure-function tests passed');
