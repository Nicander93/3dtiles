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
assert.equal(errors.friendlyError({ detail: { message: 'nested' } }), 'nested');

const forms = await loadTypeScript('../src/lib/formUtilsCore.ts');
assert.equal(forms.suggestOutputPath('C:\\data\\scene.osgb', '', '_tiles'), 'C:\\data\\scene.osgb_tiles');
assert.equal(forms.suggestOutputPath('/data/scene_tiles', '', '_tiles'), '/data/scene_tiles');
assert.equal(forms.suggestOutputPath('scene.osgb', 'D:\\out\\', '_process'), 'D:\\out\\scene.osgb_process');
assert.equal(forms.pathsEqual('C:\\DATA\\scene\\', 'c:/data/scene'), true);
assert.equal(forms.realProgressPercent({ completed: 1, total: 4 }), 25);
assert.equal(forms.realProgressPercent({ completed: 0, total: 0 }), null);

console.log('desktop pure-function tests passed');
