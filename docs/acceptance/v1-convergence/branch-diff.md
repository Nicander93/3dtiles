# Branch Diff: master vs feat/v1-prod-align

**Generated:** 2026-09-21 17:06:49 CST  
**Baseline:** R00

## Summary

- **Master tip:** `8c84bb2f1d3c8220805b19ec9e6e4a1dec9b010e`
- **Compare branch tip:** `81f3494b4002c6fb97064e8c531c58ec1f92a7dc` (origin/feat/v1-prod-align)
- **Merge base:** `671145d9cf86ab28a3ce538aacc3224162a5bd2b`
- **Master ahead:** 9 commits
- **Compare ahead:** 10 commits

## Master-only Commits (9)

这些提交在 master 上但不在 feat/v1-prod-align 上：

```
8c84bb2 Merge PR #6: fix converter CRT DLL bundling for Windows release
9b23f22 fix: bundle MSVC CRT DLLs with converter when upstream zip omits them
495a64e feat: enhance convert diagnostics + wire M1 trial UI constraints (#5)
bff3104 Merge pull request #4 from Nicander93/fix/top-rebuild-ktx2-read
ea27e68 fix: allow unused_unsafe in process_manager Unix cancel
7a5ca31 ci: fix WebKit package name for Ubuntu 24.04
b5c2eec ci: prepare sidecars before Tauri tests and install GTK deps
6b0a81d Fix CI failures: macOS linker error and Windows test failure
677cf33 fix: package zstd.dll beside basisu.exe to fix 0xc0000135
```

### 主题分类

- **Windows 打包修复 (2):** CRT DLL 捆绑、zstd.dll 打包
- **UI/功能增强 (1):** 转换诊断增强、M1 试用限制
- **CI 修复 (4):** WebKit 包名、GTK 依赖、macOS 链接错误、sidecar 准备
- **Unix 兼容 (1):** process_manager unused_unsafe 修复
- **KTX2 修复合并 (1):** top-rebuild KTX2 读取修复

## feat/v1-prod-align-only Commits (10)

这些提交在 feat/v1-prod-align 上但不在 master 上：

```
81f3494 docs(phase-18): honest Windows package inventory + limited V1 claim
0626a28 fix(top_rebuild): Phase-11 transform tests + B3DM 8-byte realign for PlanD Layer B
b59daf6 merge: feat/v0-scaffold into hardening as feat/v1-prod-align
12f763a docs(audit): Phase 11–18 gap audit + start Cesium A/B harness
0ffa0c1 1
cbed0f1 feat(acceptance): Phase 15 public data test framework
6019b40 feat(release): Phase 14 zero-Python runtime path
83f889a fix(processor): Phase 13 sibling staging and safe commit
fb1768d feat(processor): Phase 12 tileset validator layers A/B
c41609f fix(top_rebuild): Phase 11 correctness hardening (P0-1..P0-4)
```

### 主题分类

- **验收测试框架 (2):** Phase 15 公开数据测试、Cesium A/B 对比工具
- **正确性修复 (3):** Phase 11 top_rebuild 硬化、Phase 13 processor 暂存、B3DM 对齐
- **发布流程 (1):** Phase 14 零 Python 运行时路径
- **验证工具 (1):** Phase 12 tileset 验证器 Layer A/B
- **文档 (2):** Phase 18 Windows 清单、Phase 11-18 差距审计
- **合并 (1):** feat/v0-scaffold 合并到强化分支
- **未分类 (1):** 提交 "1"

## File Changes Statistics

```
75 files changed, 11859 insertions(+), 2396 deletions(-)
```

### 主要变更区域

- **验收测试脚本:** `scripts/acceptance/*` (新增多个 Python/JS 脚本)
- **Rust 核心:** `crates/processor/*`, `crates/top_rebuild/*`, `crates/converter/*`
- **发布脚本:** `scripts/release/*` (smoke test, staging)
- **测试:** `tests/acceptance/*` (阈值配置)
- **文档:** `tools/texture_ktx2/README.md`, `docs/*`

## Convergence Strategy (R00-R12)

feat/v1-convergence 从 master@8c84bb2 创建，将通过以下阶段收敛：

1. **R00 (本基线):** 记录状态，不合并代码
2. **R01-R02:** 修复 converter 设置持久化问题
3. **R03+:** 逐步合并/cherry-pick feat/v1-prod-align 的验收和修复

**注意:** master 的 9 个独有提交已经包含一些修复（CRT DLL、CI 修复等），需要在合并时处理重复/冲突。
