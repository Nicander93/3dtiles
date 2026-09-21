# R02 Implementation Summary

**任务:** 设置持久化、旧配置兼容与默认值（keep + 1 worker）  
**状态:** 代码通过  
**完成时间:** 2026-09-21 17:20 CST

## 实现的功能

### 1. Rust AppSettings 新增字段

在 `apps/desktop/src-tauri/src/settings_store.rs` 中：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
  // ... 原有字段 ...
  #[serde(default = "default_texture_compress")]
  pub default_texture_compress: bool,
  #[serde(default = "default_convert_threads")]
  pub default_convert_threads: i32,
  // ...
}

fn default_texture_compress() -> bool { false }
fn default_convert_threads() -> i32 { 1 }
```

**关键设计:**
- 使用 `#[serde(default = "...")]` 确保旧 JSON 缺失字段时使用默认值
- 新安装默认：`texture_compress = false` (keep), `convert_threads = 1`
- 旧安装：缺失字段使用默认值，已有显式值保留不变

### 2. 默认值修正

**新安装默认值 (Rust Default impl):**
- `default_texture_compress: false` (keep 模式，不压缩)
- `default_convert_threads: 1` (单 worker，稳定性优先)
- 其他字段保持原有默认值

**之前的错误默认:**
- `default_texture_compress: true` (自动启用压缩)

### 3. 旧配置兼容性

**场景 1: 旧 JSON 只有 `defaultTextureCompress`**
```json
{
  "defaultTextureCompress": true,
  "pythonServerUrl": "..."
}
```
→ `default_convert_threads` 使用默认值 `1`，`default_texture_compress` 保留 `true`

**场景 2: 旧 JSON 缺失两个新字段**
```json
{
  "defaultOutputRoot": "D:\\old",
  "pythonServerUrl": "..."
}
```
→ 两个字段都使用默认值

**场景 3: 新 JSON 显式设置**
```json
{
  "defaultTextureCompress": false,
  "defaultConvertThreads": 4,
  "pythonServerUrl": "..."
}
```
→ 显式值被保留

### 4. 前端默认值同步

**Settings.tsx 和 desktop.ts:**
```typescript
const defaults: DesktopSettings = {
  defaultTextureCompress: false,  // 改为 false (keep)
  defaultConvertThreads: 1,
  // ...
};
```

**Settings.tsx UI 改进:**
- 纹理压缩开关添加提示：`"需要 basisu 运行时；如不可用请保持关闭（keep）"`
- 转换并发数提示保持不变

### 5. 设置加载流程

**OsgbConvert.tsx 初始化:**
```typescript
void api.getSettings().then((s) => {
  setDefaultOutputRoot(s.defaultOutputRoot || '');
  if (s.defaultConvertThreads !== undefined) {
    setForm((f) => ({ ...f, convertThreads: s.defaultConvertThreads ?? 1 }));
  }
}).catch(() => {});
```

- 如果设置中有 `defaultConvertThreads`，使用该值
- 否则使用表单默认值（localStorage 或硬编码 1）

### 6. 数据库 JSON 往返

**存储格式 (SQLite):**
```sql
INSERT INTO settings (key, value_json) 
VALUES ('app', '{"defaultConvertThreads":4,...}')
ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json
```

**序列化/反序列化保证:**
- 所有字段正确往返
- 缺失字段自动补充默认值
- 显式 `null` 或缺失都使用 `#[serde(default)]` 值

## 测试覆盖

### Rust 单元测试 (7 tests)

**测试文件:** `apps/desktop/src-tauri/src/settings_store.rs`

1. **`default_settings_are_keep_and_1_worker`**
   - 验证默认值：`texture_compress = false`, `convert_threads = 1`

2. **`serialize_and_deserialize_round_trip`**
   - 完整 JSON 往返，所有字段保留

3. **`old_json_without_convert_threads_uses_default`**
   - 旧 JSON 缺失 `defaultConvertThreads` → 默认 1
   - 已有 `defaultTextureCompress` 保留原值

4. **`old_json_without_both_new_fields_uses_defaults`**
   - 两个新字段都缺失 → 都使用默认值

5. **`explicit_false_texture_compress_is_preserved`**
   - 显式 `false` 保留

6. **`explicit_true_texture_compress_is_preserved`**
   - 显式 `true` 保留（用户历史选择）

7. **`sqlite_round_trip`**
   - SQLite 存储 → 读取往返测试

## 边界条件处理

### 1. 首次启动（无设置）

- Rust `get()` 返回 `AppSettings::default()`
- 前端收到默认值：`threads = 1`, `texture = false`
- 用户未修改时提交，不会写入空数据

### 2. 旧安装升级

- 旧设置 JSON 缺失新字段
- Serde 使用 `#[serde(default)]` 补充
- 用户历史 `texture_compress = true` 保留
- 新字段 `convert_threads` 自动为 1

### 3. 非法值防护

**前端验证 (Settings.tsx):**
```tsx
<select value={form.defaultConvertThreads ?? 1}>
  <option value={1}>1（推荐）</option>
  <option value={2}>2</option>
  <option value={4}>4</option>
  <option value={0}>自动</option>
</select>
```

- 只允许 1/2/4/0
- R01 已在 processor 中验证范围 ≤16

### 4. KTX2 运行时缺失

**UI 提示:**
- Settings 页面：`"需要 basisu 运行时；如不可用请保持关闭（keep）"`
- 默认值 `false` 确保新用户不会意外启用

**不做:**
- ❌ 不自动启用实验性功能
- ❌ 不在 KTX2 不可用时隐藏选项（用户可能之后安装）

## 文件变更

1. **`apps/desktop/src-tauri/src/settings_store.rs`** (+131 / -5 行)
   - 添加 `default_convert_threads` 字段
   - 修改默认值：`texture_compress = false`
   - 添加 7 个单元测试

2. **`apps/desktop/src/api/desktop.ts`** (+1 / -1 行)
   - `settingsDefaults.defaultTextureCompress: false`

3. **`apps/desktop/src/pages/Settings.tsx`** (+2 / -1 行)
   - `defaults.defaultTextureCompress: false`
   - 纹理压缩提示文本

4. **`docs/acceptance/v1-convergence/status.md`**
   - R02 状态：进行中 → 代码通过

## 验收要点

### 代码级验收 (已完成)
- ✅ Rust 编译通过
- ✅ 7 个单元测试通过（模拟 SQLite 往返）
- ✅ 前端类型定义一致
- ✅ 默认值前后端对齐

### 功能级验收 (需实机测试)
- ⏳ 保存设置 → 重启应用 → 设置保留
- ⏳ 旧配置升级不丢失字段
- ⏳ 新安装使用 keep + 1 worker
- ⏳ Convert 页面使用设置中的并发数
- ⏳ 非法线程数被验证拦截

## 与 R01 的协同

**R01 提供:**
- `resolve_thread_config(options)` 解析 `options.convert.threads`
- 优先级：API 显式值 > 环境变量 > converter 默认

**R02 提供:**
- 持久化用户偏好的 `defaultConvertThreads`
- 前端加载设置并填充到表单 `convertThreads`
- 表单值作为 `options.convert.threads` 传递给 API

**完整流程:**
1. 用户在 Settings 页面设置 `defaultConvertThreads = 2`
2. OsgbConvert 页面加载设置，表单初始化 `convertThreads = 2`
3. 提交任务时，`options.convert.threads = 2` 传给 processor
4. R01 的 `resolve_thread_config` 解析为 `override_value = Some(2)`
5. 第一次 `run_native` 调用传递 `thread_override = Some(2)`
6. Converter 收到 `GEOFORGE_CONVERT_THREADS=2` 环境变量

## 下一步

R03-R12: 合并 feat/v1-prod-align 的修复和测试框架。
