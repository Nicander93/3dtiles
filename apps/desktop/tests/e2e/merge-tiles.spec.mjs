import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createMergeFixture } from '../../scripts/create-merge-fixtures.mjs';

const execute = promisify(execFile);
const processor = process.env.GEOFORGE_E2E_PROCESSOR || resolve('../../target/debug', process.platform === 'win32' ? 'processor.exe' : 'processor');
let root;
let inputs;
let tasks;
let submissions;

async function fixture(name, offset) {
  return createMergeFixture(root, name, offset);
}

test.beforeEach(async ({ page }) => {
  root = await mkdtemp(join(tmpdir(), 'geoforge-merge-e2e-'));
  inputs = [await fixture('输入 A', 100), await fixture('input B', -100)];
  tasks = []; submissions = [];
  // Replace only the desktop transport. Each submitted task runs the real processor binary.
  await page.route((url) => url.pathname.startsWith('/api/'), async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const reply = (body, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
    if (path === '/api/tasks' && request.method() === 'POST') {
      const payload = request.postDataJSON();
      submissions.push(payload);
      const id = `e2e-merge-${submissions.length}`;
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
      tasks.push({ id, ...payload, status: 'succeeded', stage: 'done', createdAt: Date.now(), progress: { completed: 2, total: 2 }, log: events.map((e) => JSON.stringify(e)).join('\n') });
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
  await page.goto('/tiles/merge');
});

test.afterEach(async () => { await rm(root, { recursive: true, force: true }); });

async function fillInputs(page, output = join(root, 'merged')) {
  await page.getByLabel('Tileset 1', { exact: true }).fill(inputs[0]);
  await page.getByLabel('Tileset 2', { exact: true }).fill(inputs[1]);
  await page.getByLabel('成果目录', { exact: true }).fill(output);
}

test('workspace entry submits a real merge and shows completed stages and reusable parameters', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: /3D Tiles 合并/ }).click();
  await fillInputs(page);
  await page.getByRole('button', { name: '高级设置', exact: true }).click();
  await page.getByLabel('任务名', { exact: true }).fill('合并验收');
  await page.getByRole('button', { name: '完成', exact: true }).click();
  await page.getByRole('button', { name: '开始合并' }).click();
  await expect(page).toHaveURL(/\/processing\?task=e2e-merge-1$/);
  expect(submissions[0].operation).toBe('merge-tilesets');
  expect(submissions[0].options.merge.additionalInputs).toEqual([inputs[1]]);
  const output = join(root, 'merged');
  const result = JSON.parse(await readFile(join(output, 'tileset.json'), 'utf8'));
  expect(result.root.children).toHaveLength(2);
  expect(result.root.children[0].boundingVolume.box[0]).toBe(100);
  expect(await readFile(join(output, 'sources/source-001/tileset.json'), 'utf8')).toBe(await readFile(join(inputs[0], 'tileset.json'), 'utf8'));
  await expect(page.getByRole('heading', { name: '合并验收' })).toBeVisible();
  await expect(page.getByText('检查输入', { exact: true })).toBeVisible();
  await expect(page.getByText('检查成果', { exact: true })).toBeVisible();
  await page.locator('aside.split-drawer__panel').getByRole('button', { name: '重新处理', exact: true }).click();
  await expect(page.getByLabel('Tileset 2', { exact: true })).toHaveValue(inputs[1]);
  await expect(page.getByLabel('成果目录', { exact: true })).not.toHaveValue(output);
});

test('duplicate and overlapping inputs are blocked before submission; added inputs persist', async ({ page }) => {
  await fillInputs(page);
  await page.getByLabel('Tileset 2', { exact: true }).fill(join(inputs[0], 'tileset.json'));
  await page.getByRole('button', { name: '开始合并' }).click();
  await expect(page.getByText('输入列表包含重复的 Tileset。')).toBeVisible();
  await fillInputs(page, join(inputs[1], 'nested/output'));
  await page.getByRole('button', { name: '开始合并' }).click();
  await expect(page.getByText('成果目录不能与任何输入目录重叠。')).toBeVisible();
  expect(submissions).toHaveLength(0);
  await page.getByRole('button', { name: '添加 Tileset' }).click();
  await page.getByLabel('Tileset 3', { exact: true }).fill('C:/additional');
  await page.reload();
  await expect(page.getByLabel('Tileset 3', { exact: true })).toHaveValue('C:/additional');
  await page.getByRole('button', { name: '移除 Tileset 3', exact: true }).click();
  await expect(page.getByLabel('Tileset 3', { exact: true })).toHaveCount(0);
});

test('real processor reports missing resources and preserves an existing output', async ({ page }) => {
  const output = join(root, 'existing');
  await mkdir(output); await writeFile(join(output, 'sentinel'), 'keep');
  await fillInputs(page, output);
  await page.getByRole('button', { name: '开始合并' }).click();
  await expect(page.getByText(/output already exists/)).toBeVisible();
  expect(await readFile(join(output, 'sentinel'), 'utf8')).toBe('keep');
  await rm(join(inputs[1], 'tile.glb'));
  await page.getByLabel('成果目录', { exact: true }).fill(join(root, 'failed'));
  await page.getByRole('button', { name: '开始合并' }).click();
  await expect(page.getByText(/merge resource missing/)).toBeVisible();
  await expect(page.getByRole('link', { name: '查看任务', exact: true })).toHaveCount(0);
});
