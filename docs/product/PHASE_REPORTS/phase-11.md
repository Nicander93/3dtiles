## Phase 11 完成报告

Date: **2026-09-10 ~15:30 CST** (Asia/Shanghai)  
Scope: TopRebuild Correctness Hardening — close **P0-1 … P0-4** (plan `04-v1-production-readiness-plan.md` §4).  
**Local git commit only** (push optional if token available).

### 目标
关闭生产路径上的四项正确性缺口，不重做 Phase 0–10 架构，不做 packaging / remesh / atlas / implicit tiling。

| ID | 主题 | 结果 |
|----|------|------|
| **P0-1** | release 禁止 synthetic | **PASS** — `top_rebuild` CLI + `WriteOptions::default` → `synthesize_if_empty=false`, `inject_test_textures=false`；缺/坏 content 以稳定错误码失败 |
| **P0-2** | 保留原始 Block 子树 | **PASS** — 复制原 external tileset（不再 `ensure_leaf_content` 扁平重写）；输出 `subtree_preservation.json` |
| **P0-3** | Representation = 完整覆盖 frontier | **PASS** — `Representation.parts[]` + coverage frontier 抽取；缺分支 → `SOURCE_COVERAGE_INCOMPLETE` |
| **P0-4** | world-space BV / transform | **PASS** — oriented box 八角点 AABB；nested / ECEF transform 测试 |

### V1 路线保持
Quadtree · bottom-up · meshoptimizer · LockBorder · 原 UV · **no atlas** · **no remesh** · **REPLACE**

### 主要修改
- `crates/top_rebuild/src/error.rs` — `CONTENT_MISSING` / `CONTENT_INVALID` / `GLTF_INVALID` / `TEXTURE_MISSING` / `SOURCE_COVERAGE_INCOMPLETE`
- `crates/top_rebuild/src/types.rs` — `RepresentationPart`, `Representation.parts`, `Aabb3d`, `SpatialBounds`, oriented-box corners, `Mat4d::rotation_z`
- `crates/top_rebuild/src/adapter.rs` — coverage frontier 收集（chain / root+4 / root-empty+4 / missing-child）
- `crates/top_rebuild/src/selector.rs` — 选择完整 frontier；记录 `frontier_parts`
- `crates/top_rebuild/src/tileset_writer.rs` — subtree copy + `subtree_preservation.json` + release content 校验 + multi-part proxy sources；defaults 生产安全
- `crates/top_rebuild/src/bin/top_rebuild.rs` — `synthesize_if_empty: false`
- `crates/top_rebuild/tests/phase11_correctness.rs` — 计划 §4.4 所列验收测试
- `docs/product/04-v1-production-readiness-plan.md` — NEW plan 入仓

### 测试证据
```text
cargo test -p top_rebuild
```

| suite | result |
|-------|--------|
| lib unit (36) | **PASS** — frontiers, oriented box, nested transform, selector, … |
| grid_4x4 / grid_16x16 | **PASS** |
| hlod_4x4 / phase8_metrics / proxy_2x2 | **PASS** |
| **phase11_correctness (7)** | **PASS** |

Phase 11 新测：
- `release_missing_content_must_fail`
- `release_corrupt_b3dm_must_fail`
- `debug_fixture_may_synthesize_only_when_explicit`
- `subtree_preservation_grid_4x4`
- `release_grid_4x4_no_synthetic`
- coverage frontier unit tests in `adapter::tests`
- `oriented_bounding_box_corners` + nested/ECEF transform tests

### 诚实说明
- 仓库内 `grid_4x4` / `grid_16x16` 的 `.b3dm` 仍是 **0-byte placeholders**；既有 fixture 测试继续显式 `synthesize_if_empty: true`。
- Release 路径验收用测试内生成的真实 tiny B3DM（非 synthetic success 冒充生产数据）。
- 未宣称大范围城区 / 百平方公里能力（Phase 10 边界仍有效）。

### 未做（按计划留给后续 Phase）
- Phase 12 Validator packaging
- Remeshing / Atlas / implicit tiling
- Push 非必须（本 Phase 以 local commit 为准）
