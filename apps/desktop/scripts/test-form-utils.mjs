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
assert.equal(forms.suggestOutputPath('C:\\data\\scene.osgb', '', '_tiles'), 'C:\\data\\scene.osgb_tiles');
assert.equal(forms.suggestOutputPath('/data/scene_tiles', '', '_tiles'), '/data/scene_tiles');
assert.equal(forms.suggestOutputPath('scene.osgb', 'D:\\out\\', '_process'), 'D:\\out\\scene.osgb_process');
assert.equal(forms.pathsEqual('C:\\DATA\\scene\\', 'c:/data/scene'), true);
assert.equal(forms.realProgressPercent({ completed: 1, total: 4 }), 25);
assert.equal(forms.realProgressPercent({ completed: 0, total: 0 }), null);

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
