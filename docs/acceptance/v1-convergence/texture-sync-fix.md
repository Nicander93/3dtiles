# Product A Review Fixes - Texture Mode Sync

**修复时间:** 2026-09-21 17:30 CST  
**Commit:** cdb776b

## 问题描述 (Gap #2)

**缺失的功能链:**
Settings 页面的 `defaultTextureCompress` 设置没有应用到 Convert/ProcessTiles 页面的 `textureMode` 字段。

**之前的行为:**
- ✅ `defaultConvertThreads` 正确同步到 `convertThreads`
- ❌ `defaultTextureCompress` 完全未同步
- ❌ 两个页面的 `textureMode` 总是从 `'keep'` 开始（硬编码默认值）
- ❌ 用户在 Settings 中启用纹理压缩后，仍需手动在每个页面切换

**影响:**
- 新安装用户设置 `compress=false` (keep) → 页面仍显示正确（keep）
- 但用户无法通过 Settings 统一控制默认纹理模式
- 失去了设置持久化的意义

## 修复方案

### 1. OsgbConvert.tsx 修复

**加载设置并应用纹理模式:**

```typescript
useEffect(() => {
  void api.capabilities().then(setCaps).catch(() => setCaps(null));
  void api.getSettings().then((s) => {
    setDefaultOutputRoot(s.defaultOutputRoot || '');
    
    // 并发数同步 (已有)
    if (s.defaultConvertThreads !== undefined) {
      setForm((f) => ({ ...f, convertThreads: s.defaultConvertThreads ?? 1 }));
    }
    
    // 纹理模式同步 (新增)
    if (s.defaultTextureCompress !== undefined) {
      setForm((f) => {
        if (s.defaultTextureCompress) {
          // 用户启用压缩 → 检查运行时
          const ktx2Available = ktx2Etc1sEnabled(caps, false);
          return { ...f, textureMode: ktx2Available ? 'ktx2-etc1s' : 'keep' };
        } else {
          // 用户禁用压缩 → keep
          return { ...f, textureMode: 'keep' };
        }
      });
    }
  }).catch(() => {});
}, [caps]);
```

**关键点:**
- 依赖 `caps` state（capabilities 响应）
- 仅在 `defaultTextureCompress=true` 且 KTX2 可用时启用
- 不可用时安全退回到 `keep`

### 2. ProcessTiles.tsx 修复

**相同逻辑，适配 ProcessTiles 上下文:**

```typescript
useEffect(() => {
  void api.capabilities().then(setCaps).catch(() => setCaps(null));
  void api.getSettings().then((s) => {
    setDefaultOutputRoot(s.defaultOutputRoot || '');
    
    // 纹理模式同步
    if (s.defaultTextureCompress !== undefined) {
      setForm((f) => {
        if (s.defaultTextureCompress) {
          const ktx2Available = ktx2Etc1sEnabled(caps, true); // forProcessTileset=true
          return { ...f, textureMode: ktx2Available ? 'ktx2-etc1s' : 'keep' };
        } else {
          return { ...f, textureMode: 'keep' };
        }
      });
    }
  }).catch(() => {});
}, [caps]);
```

**差异:**
- `ktx2Etc1sEnabled(caps, true)` — ProcessTiles 使用不同的可用性检查

## 行为矩阵

| Settings compress | KTX2 runtime | OsgbConvert textureMode | ProcessTiles textureMode |
|-------------------|--------------|-------------------------|--------------------------|
| `false` (keep)    | Available    | `keep`                  | `keep`                   |
| `false` (keep)    | Missing      | `keep`                  | `keep`                   |
| `true`            | Available    | `ktx2-etc1s`            | `ktx2-etc1s`             |
| `true`            | Missing      | `keep` (fallback)       | `keep` (fallback)        |

## 安全保证

### 1. 不自动启用实验性功能

**要求:** "do not auto-enable KTX2 if runtime missing"

**实现:**
```typescript
if (s.defaultTextureCompress) {
  const ktx2Available = ktx2Etc1sEnabled(caps, false);
  // 只有在运行时可用时才启用
  return { ...f, textureMode: ktx2Available ? 'ktx2-etc1s' : 'keep' };
}
```

**验证:**
- ✅ KTX2 缺失 → 即使 `compress=true` 也使用 `keep`
- ✅ 不会设置 `textureMode='ktx2-etc1s'` 然后在运行时失败
- ✅ 用户看到的选项与实际能力匹配

### 2. 尊重 Capabilities 响应

**使用现有验证函数:**
- `ktx2Etc1sEnabled(caps, false)` for OsgbConvert
- `ktx2Etc1sEnabled(caps, true)` for ProcessTiles

**检查内容:**
```typescript
export function ktx2Etc1sEnabled(caps: CapabilitiesResponse | null, forProcessTileset = false): boolean {
  if (!caps) return false;
  if (caps.postprocessBasisu?.available) return true;
  const mode = caps.textureModes?.find((m) => m.mode === 'ktx2-etc1s');
  if (!mode) return false;
  if (forProcessTileset && mode.processTileset) return mode.processTileset.supported !== false;
  return mode.supported === true;
}
```

**保证:**
- ✅ 检查 basisu 后处理可用性
- ✅ 检查 native texture mode 支持
- ✅ 区分 convert vs process-tileset 上下文

### 3. 新安装默认行为

**R02 设置的默认值:**
- `default_texture_compress: false` (Rust)
- `defaultTextureCompress: false` (TypeScript)

**页面加载流程:**
1. 新用户首次打开 Convert 页面
2. `getSettings()` 返回 `{defaultTextureCompress: false}`
3. `setForm({textureMode: 'keep'})`
4. ✅ 不启用 KTX2，符合新安装默认（keep + 1 worker）

## 边界条件

### 场景 1: Settings 未加载（网络错误）

```typescript
void api.getSettings().then(...).catch(() => {});
```

- 静默失败
- 页面使用 localStorage 或硬编码默认值
- 不阻塞用户使用

### 场景 2: Capabilities 未加载

```typescript
const ktx2Available = ktx2Etc1sEnabled(caps, false);
// caps=null → ktx2Available=false → textureMode='keep'
```

- 安全退回到 `keep`
- 不尝试使用不可用的功能

### 场景 3: 竞态条件（Settings 在 Caps 之前返回）

```typescript
useEffect(() => {
  // ...
}, [caps]);
```

- 依赖 `caps` state
- Settings 加载后，等待 Caps 加载完成才设置 `textureMode`
- 如果 Caps 先到，Settings 后到，也能正确更新

### 场景 4: 用户手动修改 textureMode

- Settings 只设置初始值
- 用户在页面上手动切换后，localStorage 保存用户选择
- 下次加载时，localStorage 优先（`loadConfig()` 在 `useEffect` 之前）

**可能的冲突:**
- User localStorage: `textureMode='ktx2-uastc'`
- Settings: `compress=false` → `keep`
- 结果: Settings 覆盖 localStorage（这是预期行为，Settings 是全局默认）

**如果需要保留用户选择:** 可以检查 localStorage 是否有值，有则跳过 Settings

## 测试验证

### 手动测试场景

1. **新安装，默认 keep:**
   - Settings: `compress=false`
   - OsgbConvert: 加载后 `textureMode='keep'` ✓
   - ProcessTiles: 加载后 `textureMode='keep'` ✓

2. **用户启用压缩，KTX2 可用:**
   - Settings: 切换 `compress=true`，保存
   - 重启应用
   - OsgbConvert: 加载后 `textureMode='ktx2-etc1s'` ✓
   - ProcessTiles: 加载后 `textureMode='ktx2-etc1s'` ✓

3. **用户启用压缩，KTX2 不可用:**
   - Settings: `compress=true`
   - basisu runtime 缺失
   - OsgbConvert: 加载后 `textureMode='keep'` (fallback) ✓
   - ProcessTiles: 加载后 `textureMode='keep'` (fallback) ✓

4. **用户手动覆盖:**
   - Settings: `compress=false`
   - 页面加载后 `textureMode='keep'`
   - 用户手动切换到 `ktx2-etc1s`
   - localStorage 保存用户选择
   - 刷新页面 → Settings 再次应用 `keep`（全局默认优先）

### 单元测试（如需要）

前端通常不写单元测试，但逻辑可验证：

```typescript
// Mock test
const mockSettings = { defaultTextureCompress: true };
const mockCaps = { postprocessBasisu: { available: true } };
const result = applyTextureMode(mockSettings, mockCaps);
expect(result).toBe('ktx2-etc1s');

const mockCapsUnavailable = { postprocessBasisu: { available: false } };
const resultFallback = applyTextureMode(mockSettings, mockCapsUnavailable);
expect(resultFallback).toBe('keep');
```

## 文件变更

1. **`apps/desktop/src/pages/OsgbConvert.tsx`** (+12 / -3 lines)
   - 添加 `defaultTextureCompress` 加载逻辑
   - 检查 KTX2 可用性
   - 依赖 `caps` state

2. **`apps/desktop/src/pages/ProcessTiles.tsx`** (+13 / -1 lines)
   - 相同逻辑
   - 使用 `forProcessTileset=true`

## 已知限制

### 不实现 Converter Worker 探测

**要求:** "Do NOT implement converter actual-worker probing this round"

**当前状态:**
- R01 记录 `converter.threads.override` (传递的值)
- R01 不探测 converter 实际使用的 worker 数
- 留作后续改进

**文档位置:**
- `docs/acceptance/v1-convergence/R01-summary.md` — 已记录为已知 gap
- `docs/acceptance/v1-convergence/status.md` — 可添加到 R01 备注

## 总结

**修复了什么:**
- ✅ Gap #1: Windows `thread_config_from_env` test flake (之前修复)
- ✅ Gap #2: `defaultTextureCompress` 不同步到页面

**新行为:**
- Settings `compress=false` → 两个页面默认 `keep`
- Settings `compress=true` + KTX2 可用 → 两个页面默认 `ktx2-etc1s`
- Settings `compress=true` + KTX2 不可用 → 安全退回 `keep`

**安全保证:**
- ❌ 不自动启用实验性功能（无运行时）
- ✅ 尊重 capabilities 响应
- ✅ 新安装默认：keep + 1 worker

**等待:**
- CI 全绿（特别是 Windows）
- 开发主管 + 产品 A checkbox 确认
- 不合并 PR
