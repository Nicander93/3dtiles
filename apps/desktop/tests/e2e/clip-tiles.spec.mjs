import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createMergeFixture } from '../../scripts/create-merge-fixtures.mjs';

const execute = promisify(execFile);
const processor = process.env.GEOFORGE_E2E_PROCESSOR || resolve('../../target/debug', process.platform === 'win32' ? 'processor.exe' : 'processor');
let root, input, tasks, submissions;
test.beforeEach(async ({ page }) => {
  root = await mkdtemp(join(tmpdir(), 'geoforge-clip-e2e-'));
  input = await createMergeFixture(root, '地理模型', 0, true);
  tasks = []; submissions = [];
  await page.route((url) => url.pathname.startsWith('/api/'), async (route) => {
    const path = new URL(route.request().url()).pathname;
    const reply = (body, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
    if (path === '/api/tasks' && route.request().method() === 'POST') {
      const payload = route.request().postDataJSON(); submissions.push(payload);
      const id = `e2e-clip-${submissions.length}`;
      const taskFile = join(root, `${id}.json`);
      await writeFile(taskFile, JSON.stringify({ schemaVersion: 1, taskId: id, ...payload }));
      let events;
      try {
        const result = await execute(processor, ['run', '--task', taskFile], { timeout: 15_000 });
        events = result.stdout.trim().split(/\r?\n/).map((line) => JSON.parse(line));
      } catch (error) {
        const errors = String(error.stdout || '').trim().split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
        return reply({ message: errors.find((e) => e.type === 'error')?.message || error.message }, 400);
      }
      tasks.push({ id, ...payload, status: 'succeeded', stage: 'done', createdAt: Date.now(), log: events.map((e) => JSON.stringify(e)).join('\n') });
      return reply({ ok: true, id, task: tasks.at(-1) });
    }
    if (path === '/api/tasks') return reply({ tasks });
    if (path.endsWith('/logs')) return reply({ log: tasks[0]?.log || '', lines: [] });
    if (path.startsWith('/api/tasks/')) return reply({ task: tasks[0] });
    if (path === '/api/artifacts') return reply({ artifacts: [] });
    if (path === '/api/health') return reply({ ok: true });
    if (path === '/api/capabilities') return reply({ ok: true, textureModes: [] });
    return reply({});
  });
  await page.goto('/tiles/clip');
});
test.afterEach(async () => { await rm(root, { recursive: true, force: true }); });
async function fill(page, output = join(root, 'cropped')) {
  await page.getByLabel('Tileset 路径', { exact: true }).fill(input);
  await page.getByLabel('成果目录', { exact: true }).fill(output);
  for (const [label, value] of [['西经度（度）', '0'], ['南纬度（度）', '0'], ['东经度（度）', '0.0003'], ['北纬度（度）', '0.0003']]) await page.getByLabel(label, { exact: true }).fill(value);
}
function positions(bytes) {
  const jsonLength = bytes.readUInt32LE(12);
  const doc = JSON.parse(bytes.subarray(20, 20 + jsonLength).toString());
  const bin = bytes.subarray(28 + jsonLength);
  const accessor = doc.accessors[doc.meshes[0].primitives[0].attributes.POSITION];
  const view = doc.bufferViews[accessor.bufferView];
  return Array.from({ length: accessor.count }, (_, i) => Array.from({ length: 3 }, (_, c) => bin.readFloatLE(view.byteOffset + (i * 3 + c) * 4)));
}
test('workspace rectangle clips actual output geometry and restores task region', async ({ page }) => {
  await page.goto('/'); await page.getByRole('link', { name: /范围裁剪/ }).click();
  await fill(page);
  await page.getByRole('button', { name: '高级设置', exact: true }).click();
  await page.getByLabel('任务名', { exact: true }).fill('裁剪验收');
  await page.getByRole('button', { name: '完成', exact: true }).click();
  await page.getByRole('button', { name: '开始裁剪' }).click();
  await expect(page).toHaveURL(/\/processing\?task=e2e-clip-1$/);
  await expect(page.getByRole('heading', { name: '裁剪验收', exact: true })).toBeVisible();
  expect(submissions[0].operation).toBe('clip-tileset');
  const output = join(root, 'cropped'); const tileset = JSON.parse(await readFile(join(output, 'tileset.json'), 'utf8'));
  const vertices = positions(await readFile(join(output, tileset.root.content.uri)));
  expect(vertices.length).toBeGreaterThan(3);
  expect(vertices.every(([x, , z]) => x >= -0.00001 && x <= 0.335 && z <= 0.00001 && z >= -0.333)).toBe(true);
  const report = JSON.parse(await readFile(join(output, 'clip-report.json'), 'utf8'));
  expect(report.trianglesBefore).toBe(1); expect(report.trianglesAfter).toBe(vertices.length / 3);
  await expect(page.getByText('裁剪几何', { exact: true })).toBeVisible();
  await page.locator('aside.split-drawer__panel').getByRole('button', { name: '重新处理', exact: true }).click();
  await expect(page.getByLabel('东经度（度）', { exact: true })).toHaveValue('0.0003');
  await expect(page.getByLabel('成果目录', { exact: true })).not.toHaveValue(output);
});
test('invalid rectangle is blocked and imported GeoJSON persists and executes', async ({ page }) => {
  await fill(page); await page.getByLabel('东经度（度）', { exact: true }).fill('-1');
  await page.getByRole('button', { name: '开始裁剪' }).click();
  await expect(page.getByText('范围应满足西经度 < 东经度，南纬度 < 北纬度。')).toBeVisible(); expect(submissions).toHaveLength(0);
  await page.getByLabel('区域输入方式').selectOption('polygon');
  const region = { type: 'Feature', properties: {}, geometry: { type: 'Polygon', coordinates: [[[0,0],[0,0.0003],[0.0003,0.0003],[0.0003,0],[0,0]]] } };
  await page.getByLabel('导入 GeoJSON 文件').setInputFiles({ name: 'region.geojson', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(region)) });
  await expect(page.getByLabel('GeoJSON 区域')).toHaveValue(JSON.stringify(region)); await page.reload();
  await expect(page.getByLabel('GeoJSON 区域')).toHaveValue(JSON.stringify(region));
  await page.getByRole('button', { name: '开始裁剪' }).click();
  await expect(page).toHaveURL(/\/processing\?task=e2e-clip-1$/);
  await expect(page.locator('aside.split-drawer__panel')).toBeVisible();
  expect(submissions[0].options.clip.region).toEqual(region);
});
test('existing output and empty region fail without a successful task or overwritten files', async ({ page }) => {
  const existing = join(root, 'existing'); await mkdir(existing); await writeFile(join(existing, 'sentinel'), 'keep');
  await fill(page, existing); await page.getByRole('button', { name: '开始裁剪' }).click();
  await expect(page.getByText(/output already exists/)).toBeVisible(); expect(await readFile(join(existing, 'sentinel'), 'utf8')).toBe('keep');
  await page.getByLabel('成果目录', { exact: true }).fill(join(root, 'empty'));
  for (const [label, value] of [['西经度（度）', '0.005'], ['南纬度（度）', '0.005'], ['东经度（度）', '0.006'], ['北纬度（度）', '0.006']]) await page.getByLabel(label, { exact: true }).fill(value);
  await page.getByRole('button', { name: '开始裁剪' }).click(); await expect(page.getByText(/clip removed all geometry/)).toBeVisible();
  await expect(page.getByRole('link', { name: '查看任务', exact: true })).toHaveCount(0);
});
