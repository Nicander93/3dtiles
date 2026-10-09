import { test, expect } from '@playwright/test';
import { mkdtemp, readFile, rm, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createMergeFixture } from '../../scripts/create-merge-fixtures.mjs';

test.beforeEach(async ({ page }) => {
  // Optional font for screenshots on Linux hosts without a CJK system font.
  if (!process.env.GEOFORGE_UI_FONT) return;
  const font = (await readFile(process.env.GEOFORGE_UI_FONT)).toString('base64');
  await page.addInitScript((font) => {
    document.addEventListener('DOMContentLoaded', () => {
      const bytes = Uint8Array.from(atob(font), (c) => c.charCodeAt(0));
      const face = new FontFace('Noto Sans SC', bytes);
      document.fonts.add(face);
      void face.load();
    });
  }, font);
});

async function installDesktop(
  page,
  { failSubmit = false, converterReady = true, savedSettings = {} } = {},
) {
  // Exercise the existing Tauri adapter and its exact command payload, not a
  // replacement page/API implementation. Real processor tests remain in e2e/.
  await page.addInitScript(
    ({ failSubmit, converterReady, savedSettings }) => {
      const caps = {
        convert: { exists: converterReady, native: converterReady },
        model: { ready: converterReady, projectedGeoreference: true },
        postprocessBasisu: { available: false },
      };
      // Without settingsVersion this mirrors a record saved when 1 was the default.
      let settings = {
        defaultOutputRoot: '',
        defaultConvertThreads: 1,
        defaultTextureCompress: false,
        execution: { resourceMode: 'auto' },
        ...savedSettings,
      };
      const tasks = [
        {
          id: 'running',
          taskName: '城区转换',
          operation: 'convert-osgb',
          input: '/survey/city',
          output: '/results/city',
          status: 'running',
          stage: 'convert',
          progress: { completed: 3, total: 10, percent: 30 },
          createdAt: 1791223200,
        },
        {
          id: 'failed',
          taskName: '失败任务',
          operation: 'merge-tilesets',
          input: '/models/a',
          output: '/results/merge',
          status: 'failed',
          stage: 'scan',
          options: { merge: { additionalInputs: ['/models/b'] } },
          error: '输入文件无法读取',
          createdAt: 1791223100,
        },
      ];
      const artifacts = [
        {
          id: 'source',
          label: '示例模型',
          path: '/models/' + 'very-long-project-name/'.repeat(12) + 'source',
          has_tileset: true,
          available: true,
          createdAt: 1791223000,
        },
        {
          id: 'missing',
          label: '已移动模型',
          path: '/missing/model',
          has_tileset: true,
          available: false,
          createdAt: 1791222900,
        },
      ];
      window.__uiCalls = [];
      window.__TAURI_INTERNALS__ = {
        invoke: async (command, args) => {
          window.__uiCalls.push({ command, args });
          if (command === 'capabilities') return caps;
          if (command === 'get_settings') return settings;
          if (command === 'update_settings') {
            settings = args.settings;
            return settings;
          }
          if (command === 'health') return { ok: true, status: 'ok', version: 'test' };
          if (command === 'get_resource_server_info')
            return { baseUrl: window.location.origin, dataDir: '/test' };
          if (command === 'list_tasks') return { tasks };
          if (command === 'get_task')
            return { task: tasks.find((task) => task.id === args.taskId) };
          if (command === 'get_task_logs') return { log: '真实接口格式的测试日志', lines: [] };
          if (command === 'cancel_task') {
            tasks.find((t) => t.id === args.taskId).status = 'cancelled';
            return { ok: true };
          }
          if (command === 'list_artifacts') return { artifacts };
          if (command === 'get_preview_url')
            return {
              url: window.location.origin + '/ui-fixture/tileset.json',
              path: '/models/source/tileset.json',
            };
          if (command === 'scan_osgb')
            return {
              valid: true,
              path: args.path,
              tileCount: 16,
              summary: { tileCount: 16, osgbFileCount: 128, srs: 'EPSG:4547', srsOrigin: '0,0,0' },
            };
          if (command === 'scan_model')
            return {
              valid: true,
              path: args.path,
              format: args.path.endsWith('.obj') ? 'obj' : 'fbx',
              materials: [],
              errors: [],
            };
          if (command === 'select_texture_root') return '/models/textures';
          if (command === 'select_model_file') return '/models/building.obj';
          if (command === 'select_output_directory') return '/results';
          if (command === 'select_input_directory') return '/survey/city';
          if (command === 'select_tileset_file') return '/models/source/tileset.json';
          if (command === 'submit_task') {
            if (failSubmit) throw new Error('输出目录已经存在');
            const id = 'submitted';
            tasks.push({ id, ...args.config, status: 'queued', createdAt: 1791223300 });
            return { id };
          }
          return { ok: true };
        },
      };
    },
    { failSubmit, converterReady, savedSettings },
  );
}

async function screenshot(page, name) {
  if (!process.env.GEOFORGE_UI_SCREENSHOTS) return;
  await mkdir(process.env.GEOFORGE_UI_SCREENSHOTS, { recursive: true });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: join(process.env.GEOFORGE_UI_SCREENSHOTS, `${name}.png`) });
}
async function submission(page) {
  return page.evaluate(
    () => window.__uiCalls.find((call) => call.command === 'submit_task')?.args.config,
  );
}
async function expectNoOverflow(page) {
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
}

test('tool navigation, logo, search and remembered sidebar', async ({ page }) => {
  await installDesktop(page);
  await page.goto('/');
  await expect(page.locator('.tool-entry')).toHaveCount(7);
  await expect(page.locator('.brand-mark img')).toHaveJSProperty('naturalWidth', 1672);
  await screenshot(page, 'tools');
  await page.getByLabel('搜索工具').fill('模型');
  await expect(page.locator('.tool-entry')).toHaveCount(3);
  await page.getByLabel('搜索工具').fill('无匹配');
  await expect(page.getByText('未找到工具')).toBeVisible();
  await page.getByLabel('搜索工具').fill('');
  await page.getByRole('link', { name: 'OSGB 转换', exact: false }).click();
  await expect(page.locator('.nav-item.active')).toHaveAttribute('aria-label', '工具');
  await expect(page.locator('.nav-item.active')).toHaveAttribute('aria-current', 'page');
  await page.getByLabel('收起侧栏').click();
  await page.reload();
  await expect(page.locator('.sidebar')).toHaveClass(/collapsed/);
  await page.getByLabel('展开侧栏').click();
  await expectNoOverflow(page);
});

test('OSGB common choices, modal focus and unchanged submit contract', async ({ page }) => {
  await installDesktop(page);
  await page.goto('/osgb/convert');
  await page.getByLabel('数据目录').fill('/survey/city');
  await expect(page.getByText('已识别 OSGB 数据')).toBeVisible();
  await page.getByLabel('重建质量').selectOption('speed');
  await expect(page.getByLabel('转换并发数')).toHaveValue('0');
  await page.getByLabel('转换并发数').selectOption('4');
  await page.getByLabel('成果目录（选择父目录后自动生成）').fill('/results/new-city');
  await screenshot(page, 'osgb');
  const open = page.getByRole('button', { name: '高级设置', exact: true });
  await open.click();
  const drawer = page.getByRole('dialog', { name: '高级设置' });
  await expect(drawer).toBeVisible();
  await page.getByLabel('重建层数').selectOption('2');
  await page.getByLabel('CRS 覆盖').fill('EPSG:4547');
  for (const [axis, value] of [
    ['X', '100'],
    ['Y', '200'],
    ['Z', '0'],
  ])
    await page.getByLabel(`原点覆盖 ${axis}`).fill(value);
  await page.getByText('任务信息', { exact: true }).click();
  await page.getByLabel('任务名', { exact: true }).fill('新城区');
  await screenshot(page, 'advanced');
  await page.keyboard.press('Escape');
  await expect(drawer).not.toBeVisible();
  await expect(open).toBeFocused();
  await open.click();
  await expect(page.getByLabel('重建层数')).toHaveValue('2');
  // Tab must remain within the native dialog.
  await drawer.getByRole('button', { name: '完成', exact: true }).focus();
  await page.keyboard.press('Tab');
  expect(
    await page.evaluate(() => document.querySelector('dialog').contains(document.activeElement)),
  ).toBe(true);
  await page.mouse.click(250, 250);
  await expect(drawer).not.toBeVisible();
  await page.getByRole('button', { name: '开始转换', exact: true }).click();
  await expect(page).toHaveURL(/processing\?task=submitted/);
  expect(await submission(page)).toEqual({
    operation: 'convert-osgb',
    input: '/survey/city',
    output: '/results/new-city',
    taskName: '新城区',
    options: {
      rebuildTop: {
        enabled: true,
        levels: 2,
        simplify: 0.5,
        textureScale: 0.5,
        l1MaxTriangles: 2000,
        l2MaxTriangles: 1000,
      },
      texture: { mode: 'keep' },
      convert: { threads: 4 },
      geo: {
        geographicExport: false,
        crs: 'EPSG:4547',
        originX: 100,
        originY: 200,
        originZ: 0,
        origin: '100,200,0',
      },
      geographicExport: false,
    },
  });
  await expect(page.locator('.split-drawer__panel')).toContainText('新城区');
});

test('a single convert worker chosen after the default change is kept', async ({ page }) => {
  await installDesktop(page, { savedSettings: { defaultConvertThreads: 1, settingsVersion: 2 } });
  await page.goto('/osgb/convert');
  await expect(page.getByLabel('转换并发数')).toHaveValue('1');
  await page.goto('/settings');
  await expect(
    page
      .locator('.field')
      .filter({ has: page.locator('label', { hasText: /^默认转换并发数$/ }) })
      .locator('select'),
  ).toHaveValue('1');
});

test('failed submission keeps form and unavailable capabilities stay disabled', async ({
  page,
}) => {
  await installDesktop(page, { failSubmit: true });
  await page.goto('/osgb/convert');
  await page.getByLabel('数据目录').fill('/survey/city');
  await page.getByLabel('成果目录（选择父目录后自动生成）').fill('/results/occupied');
  await page.getByRole('button', { name: '开始转换', exact: true }).click();
  await expect(page.getByText('输出目录已经存在')).toBeVisible();
  await expect(page.getByLabel('数据目录')).toHaveValue('/survey/city');
  await expect(page.getByLabel('成果目录（选择父目录后自动生成）')).toHaveValue(
    '/results/occupied',
  );
  await expect(page.getByLabel('纹理处理').locator('option[value="ktx2-uastc"]')).toBeDisabled();
});

test('model placement is conditional; advanced axes and textures preserve payload', async ({
  page,
}) => {
  await installDesktop(page);
  await page.goto('/model/convert');
  await page.getByLabel('模型文件').fill('/models/building.obj');
  await expect(page.getByText('已识别 OBJ 模型')).toBeVisible();
  await page.getByLabel('单位', { exact: true }).selectOption('meters');
  await page.getByLabel('定位模式').selectOption('anchor');
  await page.getByLabel('经度', { exact: true }).fill('0');
  await page.getByLabel('纬度', { exact: true }).fill('0');
  await page.getByLabel('椭球高（米）').fill('0');
  await page.getByLabel('定位模式').selectOption('projected');
  await expect(page.getByLabel('源 CRS')).toBeVisible();
  await expect(page.getByLabel('经度', { exact: true })).toHaveCount(0);
  await page.getByLabel('定位模式').selectOption('anchor');
  await page.getByLabel('保存位置（已有文件夹）').fill('/results');
  await screenshot(page, 'model');
  await page.getByRole('button', { name: '高级设置', exact: true }).click();
  await page.getByLabel('轴向', { exact: true }).selectOption('zUpRightHanded');
  await page.getByRole('button', { name: '添加目录', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('/models/textures');
  await page.getByRole('button', { name: '完成', exact: true }).click();
  await expect(page.getByText('已识别 OBJ 模型')).toBeVisible();
  await page.getByRole('button', { name: '开始转换', exact: true }).click();
  await expect(page).toHaveURL(/processing\?task=submitted/);
  const config = await submission(page);
  expect(config.operation).toBe('convert-model');
  expect(config.input).toBe('/models/building.obj');
  expect(config.output).toMatch(/^\/results\/building_tiles_[^/]+$/);
  expect(config.options).toEqual({
    model: {
      format: 'obj',
      unit: 'meters',
      axes: 'zUpRightHanded',
      missingTexturePolicy: 'error',
      textureRoots: ['/models/textures'],
    },
    georeference: { mode: 'anchor', longitudeDeg: 0, latitudeDeg: 0, ellipsoidHeightM: 0 },
    texture: { mode: 'keep' },
    modelOutput: { format: '3dtiles-1.0', tiling: 'single', lod: false },
  });
});

test('optimization preserves the existing processor request', async ({ page }) => {
  await installDesktop(page);
  await page.goto('/tiles/process?op=rebuild');
  await page.getByLabel('Tileset 目录').fill('/models/source');
  await page.getByLabel('成果目录（选择父目录后自动生成）').fill('/results/rebuilt');
  await screenshot(page, 'optimization');
  await page.getByRole('button', { name: '开始处理', exact: true }).click();
  await expect(page).toHaveURL(/processing\?task=submitted/);
  expect((await submission(page)).options).toEqual({
    rebuildTop: {
      enabled: true,
      levels: 0,
      simplify: 0.5,
      textureScale: 0.5,
      l1MaxTriangles: 4000,
      l2MaxTriangles: 2000,
    },
    texture: { mode: 'keep' },
  });
});

test('merge and clip keep inputs, region validation and task context', async ({ page }) => {
  await installDesktop(page);
  await page.goto('/tiles/merge');
  await page.getByLabel('Tileset 1', { exact: true }).fill('/models/a');
  await page.getByLabel('Tileset 2', { exact: true }).fill('/models/b');
  await page.getByLabel('成果目录', { exact: true }).fill('/results/merged');
  await screenshot(page, 'merge');
  await page.getByRole('button', { name: '开始合并', exact: true }).click();
  await expect(page).toHaveURL(/processing\?task=submitted/);
  expect(await submission(page)).toMatchObject({
    operation: 'merge-tilesets',
    input: '/models/a',
    output: '/results/merged',
    options: { merge: { additionalInputs: ['/models/b'] } },
  });
  await page.goto('/tiles/clip');
  await page.getByLabel('Tileset 路径').fill('/models/source');
  await page.getByLabel('成果目录', { exact: true }).fill('/results/clipped');
  for (const [label, val] of [
    ['西经度', '0'],
    ['南纬度', '0'],
    ['东经度', '0.001'],
    ['北纬度', '0.001'],
  ])
    await page.getByLabel(`${label}（度）`).fill(val);
  await screenshot(page, 'clip');
  await page.getByRole('button', { name: '开始裁剪', exact: true }).click();
  await expect(page).toHaveURL(/processing\?task=submitted/);
  const configs = await page.evaluate(() =>
    window.__uiCalls.filter((c) => c.command === 'submit_task').map((c) => c.args.config),
  );
  expect(configs.at(-1)).toMatchObject({
    operation: 'clip-tileset',
    input: '/models/source',
    output: '/results/clipped',
    options: { clip: { region: { bounds: [0, 0, 0.001, 0.001] } } },
  });
});

test('tasks support keyboard details, filters, logs and cancellation', async ({ page }) => {
  await installDesktop(page);
  await page.goto('/processing');
  await expect(page.locator('.split-drawer__panel')).toHaveCount(0);
  await screenshot(page, 'tasks');
  await page.getByRole('button', { name: '城区转换', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.split-drawer__panel')).toBeVisible();
  await expect(page.locator('.log-pre')).not.toBeVisible();
  await page.getByText('执行日志', { exact: true }).click();
  await expect(page.locator('.log-pre')).toContainText('真实接口格式的测试日志');
  await screenshot(page, 'task-detail');
  await page.getByRole('button', { name: '取消任务', exact: true }).click();
  await expect(page.locator('.split-drawer__panel')).toContainText('已取消');
  await page.getByRole('button', { name: '关闭', exact: true }).click();
  await page.getByRole('button', { name: '失败', exact: true }).click();
  // The existing failed filter also includes cancelled/interrupted tasks.
  await expect(page.locator('tbody tr')).toHaveCount(2);
});

test('results expose full paths, disabled missing files and dismissable menus', async ({
  page,
}) => {
  await installDesktop(page);
  await page.goto('/results');
  await expect(page.locator('.result-row')).toHaveCount(2);
  await expectNoOverflow(page);
  await screenshot(page, 'results');
  await page.getByLabel('更多 · 示例模型').click();
  await expect(page.getByRole('button', { name: '复制路径' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: '复制路径' })).toHaveCount(0);
  await expect(
    page
      .locator('.result-row')
      .filter({ hasText: '已移动模型' })
      .getByRole('link', { name: '预览', exact: true }),
  ).toHaveAttribute('tabindex', '-1');
  await page.getByLabel('搜索成果').fill('不存在');
  await expect(page.getByText('未找到成果')).toBeVisible();
});

test('settings keep resource choices and saved values', async ({ page }) => {
  await installDesktop(page);
  await page.goto('/settings');
  await screenshot(page, 'settings');
  await page
    .locator('.field')
    .filter({ has: page.locator('label', { hasText: /^资源模式$/ }) })
    .locator('select')
    .selectOption('custom');
  await page
    .locator('.field')
    .filter({ has: page.locator('label', { hasText: /^内存预算/ }) })
    .locator('input')
    .fill('2048');
  await page.getByRole('button', { name: '保存', exact: true }).click();
  await expect(page.getByText('已保存', { exact: true })).toBeVisible();
  const config = await page.evaluate(
    () => window.__uiCalls.find((c) => c.command === 'update_settings').args.settings,
  );
  expect(config.execution).toEqual({ resourceMode: 'custom', memoryBudgetMiB: 2048 });
});

test('compact forms fit 1366×768 and 1024×768 without page overflow', async ({ page }) => {
  await installDesktop(page);
  for (const width of [1366, 1024]) {
    await page.setViewportSize({ width, height: 768 });
    for (const path of [
      '/osgb/convert',
      '/model/convert',
      '/tiles/process?op=rebuild',
      '/tiles/merge',
      '/tiles/clip',
      '/settings',
    ]) {
      await page.goto(path);
      await expectNoOverflow(page);
      if (path === '/osgb/convert' || path === '/model/convert') {
        const button = await page
          .getByRole('button', { name: '开始转换', exact: true })
          .boundingBox();
        expect(button.y + button.height).toBeLessThanOrEqual(768);
      }
    }
  }
});

test('real Cesium loads with inspector, drawing and flatten controls', async ({ page }) => {
  await installDesktop(page);
  const root = await mkdtemp(join(tmpdir(), 'geoforge-ui-'));
  try {
    const input = await createMergeFixture(root, 'source', 0, true);
    await page.route('**/ui-fixture/*', async (route) => {
      const name = new URL(route.request().url()).pathname.split('/').at(-1);
      await route.fulfill({
        body: await readFile(join(input, name)),
        contentType: name.endsWith('.json') ? 'application/json' : 'model/gltf-binary',
      });
    });
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto('/preview/tiles?artifact=source');
    await expect(page.getByText('加载完成', { exact: true })).toBeVisible({ timeout: 30000 });
    await expect(page.locator('.info-panel')).toHaveCount(0);
    await screenshot(page, 'preview');
    await page.getByRole('button', { name: '信息', exact: true }).click();
    await expect(page.locator('.info-panel')).toBeVisible();
    await screenshot(page, 'preview-info');
    await page.getByLabel('关闭数据信息').click();
    await page.getByRole('button', { name: '模型操作' }).click();
    await page.keyboard.press('Escape');
    await expect(page.locator('.menu__panel')).toHaveCount(0);
    await page.getByRole('button', { name: '模型操作' }).click();
    await page.getByRole('button', { name: '范围裁剪并导出' }).click();
    await page.getByRole('button', { name: '开始绘制', exact: true }).click();
    await expect(page.getByText('已绘制 0 个控制点', { exact: true })).toBeVisible();
    await screenshot(page, 'preview-clip');
    await page.getByRole('button', { name: '关闭操作', exact: true }).click();
    await page.getByRole('button', { name: '模型操作' }).click();
    await page.getByRole('button', { name: '区域压平并导出' }).click();
    await expect(page.getByRole('button', { name: '显示压平拖拽工具' })).toBeDisabled();
    await expect(page.getByRole('button', { name: '导出压平模型' })).toBeDisabled();
    await screenshot(page, 'preview-flatten');
    await expectNoOverflow(page);
    expect(errors).toEqual([]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
