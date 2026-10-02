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
