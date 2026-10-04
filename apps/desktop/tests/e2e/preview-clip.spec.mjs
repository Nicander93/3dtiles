import { test, expect } from '@playwright/test';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createMergeFixture } from '../../scripts/create-merge-fixtures.mjs';

const execute = promisify(execFile);
// CI uses software WebGL: repeated GPU picking and drawing need a larger total budget.
// Individual locator assertions keep their short defaults so functional failures surface promptly.
test.describe.configure({ timeout: 120000 });
const processor = process.env.GEOFORGE_E2E_PROCESSOR || resolve('../../target/debug', process.platform === 'win32' ? 'processor.exe' : 'processor');
let root, input, artifacts, submissions, tasks;
test.beforeEach(async ({ page }) => {
  root = await mkdtemp(join(tmpdir(), 'geoforge-preview-clip-'));
  input = await createMergeFixture(root, 'source', 0, true);
  artifacts = [{ id: 'source', path: input, label: '原始模型', has_tileset: true }]; submissions = []; tasks = [];
  await page.route((url) => url.pathname.startsWith('/fixture/'), async (route) => {
    const [, , id, ...parts] = new URL(route.request().url()).pathname.split('/');
    const artifact = artifacts.find((a) => a.id === id);
    if (!artifact || parts.some((p) => p === '..')) return route.fulfill({ status: 404 });
    const file = join(artifact.path, ...parts);
    await route.fulfill({ body: await readFile(file), contentType: file.endsWith('.json') ? 'application/json' : 'model/gltf-binary' });
  });
  await page.route((url) => url.pathname.startsWith('/api/'), async (route) => {
    const path = new URL(route.request().url()).pathname;
    const reply = (body, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
    if (path.endsWith('/preview-url')) {
      const artifact = artifacts.find((a) => a.id === path.split('/')[3]);
      return reply({ ok: true, url: `/fixture/${artifact.id}/tileset.json`, path: join(artifact.path, 'tileset.json') });
    }
    if (path === '/api/tasks' && route.request().method() === 'POST') {
      const payload = route.request().postDataJSON(); submissions.push(payload);
      const id = `preview-clip-${submissions.length}`, file = join(root, `${id}.json`);
      await writeFile(file, JSON.stringify({ schemaVersion: 1, taskId: id, ...payload }));
      try { await execute(processor, ['run', '--task', file], { timeout: 15000 }); }
      catch (error) { return reply({ message: error.stdout || error.message }, 400); }
      tasks.push({ id, ...payload, status: 'succeeded', stage: 'done' });
      artifacts.push({ id, taskId: id, path: payload.output.path, label: '裁剪模型', has_tileset: true });
      return reply({ ok: true, id, task: tasks.at(-1) });
    }
    if (path === '/api/artifacts') return reply({ artifacts });
    if (path.startsWith('/api/tasks/')) return reply({ task: tasks.find((t) => t.id === path.split('/')[3]) });
    if (path === '/api/tasks') return reply({ tasks });
    if (path === '/api/health') return reply({ ok: true });
    return reply({});
  });
});
test.afterEach(async () => { await rm(root, { recursive: true, force: true }); });

async function openClip(page,operation = 'clip') {
  await page.goto('/preview/tiles?artifact=source');
  await expect(page.getByText('加载完成', { exact: true })).toBeVisible({ timeout: 20000 });
  await page.getByRole('button', { name: '模型操作' }).click();
  await page.getByRole('button', { name: operation === 'flatten' ? '区域压平并导出' : '范围裁剪并导出' }).click();
  const frame = page.frames().find((f) => f.url().includes('cesium-preview.html'));
  await frame.evaluate(() => {
    const { viewer } = window.__geoforgePreview;
    viewer.camera.setView({ destination: Cesium.Cartesian3.fromDegrees(0.00025, 0.00025, 250), orientation: { heading: 0, pitch: -Math.PI / 2, roll: 0 } });
  });
  return frame;
}
async function screen(page, frame, lon, lat) {
  const pixel = await frame.evaluate(([lon, lat]) => {
    const { viewer } = window.__geoforgePreview;
    const p = Cesium.SceneTransforms.worldToWindowCoordinates(viewer.scene, Cesium.Cartesian3.fromDegrees(lon, lat, 10));
    return { x: p.x, y: p.y };
  }, [lon, lat]);
  const box = await page.locator('iframe').boundingBox();
  return { x: box.x + pixel.x, y: box.y + pixel.y };
}
async function clickGeo(page, frame, lon, lat) { const p = await screen(page, frame, lon, lat); await page.mouse.click(p.x, p.y); }

test('draw rectangle on real Cesium tiles, export real geometry and open the new model', async ({ page }) => {
  const frame = await openClip(page);
  await page.getByRole('button', { name: '开始绘制', exact: true }).click();
  await expect(page.getByText('已绘制 0 个控制点', { exact: true })).toBeVisible();
  const a = await screen(page, frame, 0.00005, 0.00005), b = await screen(page, frame, 0.0003, 0.0003);
  await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move(b.x, b.y, { steps: 8 }); await page.mouse.up();
  await expect(page.getByText('区域已闭合 · 2 个控制点')).toBeVisible();
  await expect(page.getByRole('button', { name: '导出裁剪模型', exact: true })).toBeEnabled();
  await page.getByLabel('裁剪成果目录').fill(join(root, 'cropped'));
  await page.getByRole('button', { name: '导出裁剪模型', exact: true }).click();
  await expect(page.getByRole('button', { name: '打开裁剪后模型' })).toBeVisible();
  expect(submissions[0].operation).toBe('clip-tileset');
  const report = JSON.parse(await readFile(join(root, 'cropped', 'clip-report.json'), 'utf8'));
  expect(report.trianglesBefore).toBe(1); expect(report.trianglesAfter).toBeGreaterThan(1);
  expect(submissions[0].options.clip.region.bounds[0]).toBeCloseTo(0.00005, 5);
  await page.getByRole('button', { name: '打开裁剪后模型' }).click();
  await expect(page).toHaveURL(/artifact=preview-clip-1/);
  await expect(page.getByText('加载完成', { exact: true })).toBeVisible({ timeout: 20000 });
  await expect(page.getByRole('complementary', { name: '模型操作' })).toHaveCount(0);
  expect(JSON.parse(await readFile(join(input, 'tileset.json'), 'utf8')).asset.version).toBe('1.0');
});

test('polygon supports undo, vertex dragging, validation and clearing without accepting forged messages', async ({ page }) => {
  const frame = await openClip(page);
  await page.getByLabel('绘制方式').selectOption('polygon');
  await page.getByRole('button', { name: '开始绘制', exact: true }).click();
  await expect(page.getByText('已绘制 0 个控制点', { exact: true })).toBeVisible();
  for (const [i, p] of [[0.00005, 0.00005], [0.0004, 0.00005], [0.0002, 0.00012], [0.00005, 0.0004]].entries()) {
    await clickGeo(page, frame, ...p);
    await expect(page.getByText(`已绘制 ${i + 1} 个控制点`, { exact: true })).toBeVisible();
  }
  await page.getByRole('button', { name: '完成绘制', exact: true }).click();
  await expect(page.getByText(/首版只支持不自交的凸多边形/)).toBeVisible();
  await expect(page.getByRole('button', { name: '导出裁剪模型', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: '撤销顶点' }).click();
  await page.getByRole('button', { name: '撤销顶点' }).click();
  await clickGeo(page, frame, 0.00005, 0.0004);
  await page.getByRole('button', { name: '完成绘制', exact: true }).click();
  await expect(page.getByRole('button', { name: '导出裁剪模型', exact: true })).toBeEnabled();
  const a = await screen(page, frame, 0.0004, 0.00005), b = await screen(page, frame, 0.0003, 0.00005);
  await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move(b.x, b.y, { steps: 8 }); await page.mouse.up();
  await page.getByText('区域参数', { exact: true }).click();
  await expect(page.locator('.preview-region-json')).toContainText('Polygon');
  const region = JSON.parse(await page.locator('.preview-region-json').textContent());
  expect(region.coordinates[0][1][0]).toBeCloseTo(0.0003, 5);
  expect(await frame.evaluate(() => window.__geoforgePreview.viewer.scene.screenSpaceCameraController.enableInputs)).toBe(true);
  await page.getByRole('button', { name: '清空区域' }).click();
  await page.evaluate(() => window.postMessage({ type: 'geoforge-region', mode: 'rectangle', points: [[0,0],[0.0003,0.0003]], complete: true, active: true }, window.location.origin));
  await expect(page.getByRole('button', { name: '导出裁剪模型', exact: true })).toBeDisabled();
  expect(submissions).toHaveLength(0);
});

test('a preview-only ENU placement cannot submit geographic clipping', async ({ page }) => {
  const local = await createMergeFixture(root, 'local', 0, false);
  artifacts[0].path = local;
  await page.goto('/preview/tiles?artifact=source');
  await expect(page.getByText(/此模型使用预览临时定位/)).toBeVisible({ timeout: 20000 });
  await page.getByRole('button', { name: '模型操作' }).click();
  await expect(page.getByRole('button', { name: '范围裁剪并导出' })).toBeDisabled();
});

async function drawRectangle(page,frame) {
  await page.getByRole('button', { name: '开始绘制', exact: true }).click();
  await expect(page.getByText('已绘制 0 个控制点', { exact: true })).toBeVisible();
  const a=await screen(page,frame,0.00005,0.00005), b=await screen(page,frame,0.0003,0.0003);
  await page.mouse.move(a.x,a.y); await page.mouse.down(); await page.mouse.move(b.x,b.y,{steps:6}); await page.mouse.up();
  await expect(page.getByText('区域已闭合 · 2 个控制点')).toBeVisible();
}
async function dragPlane(page,frame,pixels) {
  const point=await frame.evaluate(async () => {
    const { viewer }=window.__geoforgePreview;
    // Camera lookAt updates projection matrices during rendering; pick the visible frame.
    await new Promise((resolve) => { const remove=viewer.scene.postRender.addEventListener(() => { remove(); resolve(); }); viewer.scene.requestRender(); });
    const pixel=Cesium.SceneTransforms.worldToWindowCoordinates(viewer.scene,window.__geoforgeFlatten.handlePosition);
    return {x:pixel.x,y:pixel.y};
  });
  const box=await page.locator('iframe').boundingBox();
  const x=box.x+point.x+1,y=box.y+point.y+1;
  await page.mouse.move(x,y); await page.mouse.down();
  expect(await frame.evaluate(() => window.__geoforgePreview.viewer.scene.screenSpaceCameraController.enableInputs)).toBe(false);
  await page.mouse.move(x,y+pixels,{steps:6}); await page.mouse.up();
  expect(await frame.evaluate(() => window.__geoforgePreview.viewer.scene.screenSpaceCameraController.enableInputs)).toBe(true);
  const height=await frame.evaluate(() => window.__geoforgeFlatten.heightMeters);
  await expect(page.getByLabel('目标平面高度')).toContainText(height.toFixed(2)); return height;
}

test('drag flatten plane in real Cesium, undo height and export real geometry with sidewalls', async ({ page }) => {
  const frame=await openClip(page,'flatten'); await drawRectangle(page,frame);
  await expect(page.getByRole('button',{name:'导出压平模型',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'显示压平拖拽工具'}).click();
  await expect(page.getByLabel('目标平面高度')).toBeVisible();
  const initial=await frame.evaluate(() => window.__geoforgeFlatten.heightMeters);
  const changed=await dragPlane(page,frame,35); expect(Math.abs(changed-initial)).toBeGreaterThan(0.5);
  await page.getByRole('button',{name:'撤销高度调整'}).click();
  await expect(page.getByLabel('目标平面高度')).toContainText(initial.toFixed(2));
  const height=await dragPlane(page,frame,25);
  const output=join(root,'flattened'); await page.getByLabel('压平成果目录').fill(output);
  await page.getByRole('button',{name:'导出压平模型',exact:true}).click();
  await expect(page.getByRole('button',{name:'打开压平后模型'})).toBeVisible();
  expect(submissions[0].operation).toBe('flatten-tileset'); expect(submissions[0].options.flatten.heightMeters).toBe(height);
  expect(submissions[0].options.preview.sourceArtifactId).toBe('source');
  const report=JSON.parse(await readFile(join(output,'flatten-report.json'),'utf8'));
  expect(report.trianglesFlattened).toBeGreaterThan(0); expect(report.wallTriangles).toBeGreaterThan(0);
  expect(report.heightMeters).toBeCloseTo(height,10); expect(report.contentsProcessed).toBe(1);
  await expect(page.getByText('An error occurred while rendering.')).toHaveCount(0);
  await page.getByRole('button',{name:'打开压平后模型'}).click();
  await expect(page).toHaveURL(/artifact=preview-clip-1/);
  await expect(page.getByText('加载完成',{exact:true})).toBeVisible({timeout:20000});
  expect(await page.frames().find((f)=>f.url().includes('cesium-preview.html')).evaluate(() => Boolean(window.__geoforgeFlatten))).toBe(false);
  await page.goto('/processing?task=preview-clip-1');
  await page.getByRole('button',{name:'重新处理',exact:true}).last().click();
  await expect(page).toHaveURL(/preview\/tiles\?operation=flatten&artifact=source/);
  await expect(page.getByRole('heading',{name:'区域压平',exact:true})).toBeVisible({timeout:20000});
  await expect(page.getByRole('button',{name:'导出压平模型',exact:true})).toBeDisabled();
});

test('closing flatten tool restores the source view and clearing the region invalidates height', async ({ page }) => {
  const frame=await openClip(page,'flatten'); await drawRectangle(page,frame);
  await page.getByRole('button',{name:'显示压平拖拽工具'}).click(); await expect(page.getByLabel('目标平面高度')).toBeVisible();
  await frame.evaluate(() => { const {viewer}=window.__geoforgePreview; viewer.camera.lookAt(window.__geoforgeFlatten.center,new Cesium.HeadingPitchRange(0.3,-0.6,250)); viewer.camera.lookAtTransform(Cesium.Matrix4.IDENTITY); });
  await dragPlane(page,frame,-20); await page.getByRole('button',{name:'恢复原高度'}).click();
  await page.getByRole('button',{name:'清空区域'}).click();
  await expect(page.getByLabel('目标平面高度')).toHaveCount(0); await expect(page.getByRole('button',{name:'导出压平模型',exact:true})).toBeDisabled();
  await expect.poll(()=>frame.evaluate(()=>Boolean(window.__geoforgePreview.tileset.customShader))).toBe(false);
  await drawRectangle(page,frame); await page.getByRole('button',{name:'显示压平拖拽工具'}).click(); await expect(page.getByLabel('目标平面高度')).toBeVisible();
  await page.getByRole('button',{name:'关闭操作'}).click();
  await expect.poll(()=>frame.evaluate(()=>Boolean(window.__geoforgePreview.tileset.customShader))).toBe(false);
  expect(await frame.evaluate(()=>window.__geoforgePreview.viewer.scene.screenSpaceCameraController.enableInputs)).toBe(true);
  expect(submissions).toHaveLength(0);
});
