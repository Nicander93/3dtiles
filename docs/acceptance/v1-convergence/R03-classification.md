# R03 三路对比分类

**日期:** 2026-09-21  
**共同祖先:** `671145d` (docs(audit): Phase 11–18 gap audit)  
**对比分支:**
- `origin/master@3effd90` (包含 R00-R02)
- `origin/feat/v1-prod-align@81f3494` (参考源)

## 执行概要

**目标:** 将 feat/v1-prod-align Phase 11 的 TopRebuild 正确性修复安全合并到 master,同时保留 master 上的 R01/R02 改进。

**策略:** 分主题增量合并,每批通过测试后再进行下一批。

## 分类维度

### 1. 已在 master (无需移植)

| 文件/改动 | 状态 | 备注 |
|---------|------|------|
| `crates/processor/src/stages/convert.rs` R01 线程逻辑 | 已合并 | master 独有,保留 |
| `apps/desktop/src-tauri/src/settings_store.rs` R02 | 已合并 | master 独有,保留 |
| `apps/desktop/scripts/prepare-converter.ps1` CRT 修复 | 已合并 | master 改进版本 |
| `apps/desktop/scripts/fix-basisu-crt.ps1` | 已合并 | master 独有 |
| `.github/workflows/` CI 修复 | 已合并 | macOS/Windows CI 改进 |
| `docs/acceptance/v1-convergence/` | 已合并 | R00-R02 文档 |

### 2. 可直接移植 (无冲突)

#### 2A. TopRebuild 核心正确性 (P0-1 到 P0-4)

**提交:** `c41609f` (Phase 11 correctness hardening)

**核心文件:**
- `crates/top_rebuild/src/types.rs` (+445 lines)
  - 新增 `Aabb3d`, `SpatialBounds` 类型
  - `Mat4d::inverse()`, `transform_direction()`
  - 世界空间边界计算
  
- `crates/top_rebuild/src/adapter.rs` (~300 lines 改动)
  - `primary_world_transform()` 语义
  - 保留外部 tileset 引用的逻辑
  
- `crates/top_rebuild/src/tileset_writer.rs` (~500 lines 改动)
  - `subtree_preservation.json` 支持
  - 完整覆盖前沿选择
  - 嵌套 transform 语义

- `crates/top_rebuild/src/selector.rs` (~50 lines)
  - 边界选择器改进

- `crates/top_rebuild/src/tree_builder.rs` (~30 lines)
  - 世界空间构建逻辑

- `crates/top_rebuild/tests/phase11_correctness.rs` (新文件 +270 lines)
  - P0-1: 关闭合成回退
  - P0-2: 外部 tileset 保留
  - P0-3: 完整覆盖前沿
  - P0-4: 定向/世界空间边界

**依赖:**
- `crates/top_rebuild/src/error.rs` 新增错误类型
- `crates/top_rebuild/Cargo.toml` 小版本号更新

**测试状态:** 新增 4 个 phase11 测试,需验证与现有测试兼容

#### 2B. B3DM 8字节对齐修复

**提交:** `0626a28` (transform tests + B3DM realign)

**文件:**
- `crates/top_rebuild/src/b3dm.rs` (+180 lines)
  - 8字节对齐函数
  - table + byteLength padding
  
- `crates/top_rebuild/tests/transform_invariants.rs` (更新)
  - 使用新 API: `primary_world_transform()` / `world_aabb`
  - 标记 FIXME(phase-11) 两处 ECEF/remount 行为

**状态:** 需要小心合并,因为涉及现有测试更新

### 3. 需要适配合并

#### 3A. convert.rs 冲突区

**master 版本 (R01):**
- 添加 `ThreadConfig` 结构
- `resolve_thread_config()` 解析逻辑
- 完整的重试语义
- 13 个新测试

**v1-prod-align 版本:**
- 简化的命令构建
- 移除 Docker 回退
- 不同的错误处理

**合并策略:**
- **保留 master R01 逻辑为主**
- 从 v1-prod-align 提取:
  - KTX2 路径修复 (如果有)
  - basisu.exe PATH 处理
  - 任何其他正确性修复
  
**需要检查:** v1-prod-align 的 convert.rs 是否有 KTX2 读取相关修复

#### 3B. texture_ktx2.rs 新文件

**v1-prod-align:**
- `crates/processor/src/stages/texture_ktx2.rs` (+759 lines)
- 新的 KTX2 纹理处理流程

**master:**
- 该文件不存在

**合并策略:**
- 先评估是否需要立即引入
- 如果 Phase 11 不依赖,可推迟到 R04/R05
- 如果 convert.rs 的 KTX2 读取依赖它,需要同步移植

#### 3C. validator.rs 重构

**v1-prod-align:**
- 从 `stages/validate.rs` 分离出 `validator.rs` (+927 lines)
- Layer A/B 验证逻辑

**master:**
- `stages/validate.rs` 保持原样

**合并策略:**
- Phase 12 工作 (R04)
- 当前跳过,除非 Phase 11 测试依赖

### 4. 文档和脚本 (分开处理)

#### 4A. 产品文档

**v1-prod-align 新增:**
- `docs/product/04-v1-production-readiness-plan.md` (+2351 lines)
- `docs/product/PHASE_REPORTS/phase-11.md` 等

**处理:**
- 与代码分开的 PR
- 或在 Phase 11 代码 PR 中包含 phase-11.md

#### 4B. 验收脚本

**v1-prod-align:**
- `scripts/acceptance/*.py` (Phase 15 工作)
- Cesium A/B harness

**处理:**
- R07/R08 (验收测试框架)

### 5. 明确推迟

| 分类 | 推迟到 | 原因 |
|-----|--------|------|
| Phase 12 validator | R04 | 独立功能 |
| Phase 13 commit 修复 | R05 | 兄弟暂存逻辑 |
| Phase 14 零 Python | R06 | 发布流程 |
| Phase 15 验收框架 | R07 | 测试基础设施 |
| Cesium A/B 工具 | R08 | 验收工具 |
| 打包/installer 变更 | 按需 | 需要与 master CRT 修复协调 |

## R03 首次 PR 范围

**包含:**

1. **Phase 11 核心正确性** (提交 c41609f)
   - 所有 `top_rebuild/` 改动
   - 新增 `phase11_correctness.rs` 测试
   - `error.rs` 扩展

2. **B3DM 对齐修复** (提交 0626a28 的 b3dm.rs 部分)
   - `b3dm.rs` 8字节对齐函数
   - 跳过 `convert.rs` KTX2 部分 (如果冲突)
   - 更新 `transform_invariants.rs` 测试

3. **文档**
   - 本分类文档 (`R03-classification.md`)
   - `docs/product/PHASE_REPORTS/phase-11.md` (可选)

**不包含 (推迟到后续 PR):**
- `texture_ktx2.rs` 新文件
- `validator.rs` 分离
- Phase 12+ 的任何改动
- 验收脚本

**验收标准:**
- `cargo test -p top_rebuild` 全部通过
- `cargo test -p processor` 保持原有通过状态
- 无编译警告
- CI 绿色

## 冲突处理预案

### convert.rs 冲突

**如果 v1-prod-align 有关键修复 (如 KTX2 读取):**

```rust
// 在 master R01 逻辑中插入 v1-prod-align 的修复点
// 保持 resolve_thread_config() 和重试逻辑不变
// 添加:
// - basisu 路径处理
// - KTX2 opaque pass-through (如需要)
```

**如果无关键修复:**
- 保持 master 版本,标记已评估

### transform_invariants.rs 冲突

**master 版本:** 可能有 CI 修复  
**v1-prod-align:** API 更新 + FIXME 标记

**合并:**
- 采用 v1-prod-align 的 API 更新
- 保留 master 的任何修复
- 保持 FIXME(phase-11) 标记

## 后续批次规划

| 批次 | 内容 | 估计工作量 |
|-----|------|-----------|
| R03.1 (本批) | Phase 11 核心 + B3DM | 中等 |
| R03.2 | KTX2 路径修复 (如需) | 小 |
| R04 | Phase 12 validator | 中等 |
| R05 | Phase 13 commit | 小 |
| R06-R08 | Phase 14-15 | 按计划 |

## 验证计划

### 单元测试
```bash
# TopRebuild 全套
cargo test -p top_rebuild

# Phase 11 新增测试
cargo test -p top_rebuild phase11_correctness

# Processor 回归
cargo test -p processor
```

### 集成测试
- 等待 CI 运行完整测试矩阵

### 手动验证
- 推迟到 R11 (完整回归)

## 风险评估

**低风险:**
- `types.rs` 新类型添加 (纯新增)
- `phase11_correctness.rs` 新测试文件

**中风险:**
- `adapter.rs` / `tileset_writer.rs` 大幅改动
  - 缓解: 现有测试覆盖,逐步验证
  
- `transform_invariants.rs` 测试更新
  - 缓解: 对比 diff 确保只更新 API 调用

**高风险:**
- `convert.rs` 合并冲突
  - 缓解: 如无 KTX2 关键修复,保持 master 版本

## 状态追踪

| 文件类别 | 分析完成 | 移植完成 | 测试通过 |
|---------|---------|---------|---------|
| types.rs | ✓ | ⏳ | ⏳ |
| adapter.rs | ✓ | ⏳ | ⏳ |
| tileset_writer.rs | ✓ | ⏳ | ⏳ |
| b3dm.rs | ✓ | ⏳ | ⏳ |
| tests/phase11_correctness.rs | ✓ | ⏳ | ⏳ |
| tests/transform_invariants.rs | ✓ | ⏳ | ⏳ |
| error.rs | ✓ | ⏳ | ⏳ |
| 文档 | ✓ | ⏳ | N/A |

---

**下一步:** 开始移植 types.rs 新类型定义
