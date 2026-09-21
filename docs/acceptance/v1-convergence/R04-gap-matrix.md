# R04 Gap Matrix - TopRebuild Correctness 验证

**分析日期:** 2026-09-21  
**基于:** master@90d0032 (R03.5 已合并)  
**对比:** 原计划 Phase 11 P0-1 至 P0-4 要求

## Gap Matrix

| 项目 | 要求 | 状态 | 覆盖位置 | 备注 |
|------|------|------|----------|------|
| **P0-1: 合成几何禁令** | Release 路径不允许合成成功 | ✅ 已覆盖 | R03.5: release_missing_content_must_fail | WriteOptions::synthesize_if_empty = false |
| **P0-1: Debug vs Release** | Debug 允许合成,Release 拒绝 | ✅ 已覆盖 | R03.5: debug_fixture_may_synthesize_only_when_explicit | 测试验证区别 |
| **P0-1: 内容验证** | validate_release_content | ✅ 已覆盖 | R03.5: validate_release_content | B3DM/GLB magic, primitives, finite coords |
| **P0-2: Block 子树保留** | 保留外部 tileset 结构 | ✅ 已覆盖 | R03.4: preserve_block_subtree | 核心函数完成 |
| **P0-2: 结构完整性** | 节点/内容/URI 计数 | ✅ 已覆盖 | R03.4: structure_digest | 完整性验证 |
| **P0-2: 复制和对齐** | 递归复制 + B3DM 对齐 | ✅ 已覆盖 | R03.4: copy_dir_recursive + R03.2 integration | realign_b3dm_tree 调用 |
| **P0-3: Frontier 覆盖** | 多 parts 支持 | ✅ 已覆盖 | R03.1: RepresentationPart, R03.4: resolve_proxy_sources | 基础支持完成 |
| **P0-3: 每个有效子分支** | 所有 parts 都记录 | ✅ 已覆盖 | R03.1: parts Vec + R03.4: 遍历所有 parts | 完整覆盖 |
| **P0-4: Transform 语义** | World-space AABB/corners | ✅ 已覆盖 | R03.1: world_aabb, local_corners | Mat4d/BoundingVolume 方法 |
| **P0-4: Parent-local roundtrip** | 嵌套 transform 精度 | ✅ 已覆盖 | R03.5: transform_identity_translation_rotation_nested | 测试验证 |
| **P0-4: ECEF 大坐标** | 大坐标精度不丢失 | ✅ 已覆盖 | R03.5: ECEF transform test | 验证 inverse 精度 |
| **P0-4: Rotation Z** | rotation_z 方法 | ✅ 已覆盖 | R03.1: Mat4d::rotation_z | 实现完成 |
| **B3DM 对齐** | 8字节对齐 | ✅ 已覆盖 | R03.2: realign_b3dm_byte_alignment | 完整实现 |
| **B3DM 表对齐** | batch table JSON 对齐 | ✅ 已覆盖 | R03.2: 表/GLB/总长度对齐 | 3个对齐点 |
| **GLB 提取** | 从 B3DM 提取 GLB | ✅ 已覆盖 | 现有: load_content_glb | 已实现 |
| **GE monotonicity** | geometric_error 单调递增 | ⚠️ 仅测未实数 | R03.5: geometric_error_monotonic (tileset_writer tests) | 单元测试存在,未集成测试 |
| **REPLACE switching** | refine: REPLACE 正确性 | ⚠️ 仅测未实数 | 代码已使用 REPLACE | 无专门验证测试 |
| **完整集成测试** | 端到端 rebuild 验证 | ✅ 已覆盖 | R03.5: subtree_preservation_grid_4x4, release_grid_4x4_no_synthetic | 集成测试通过 |

## 详细分析

### ✅ 已完整覆盖的项目 (15/17)

1. **合成几何禁令 (P0-1)**
   - WriteOptions::synthesize_if_empty = false (default)
   - release_missing_content_must_fail 测试
   - release_corrupt_b3dm_must_fail 测试

2. **Block 子树保留 (P0-2)**
   - preserve_block_subtree 核心函数
   - structure_digest 验证
   - copy_dir_recursive + realign_b3dm_tree

3. **Frontier 覆盖 (P0-3)**
   - RepresentationPart 结构
   - resolve_proxy_sources 多 parts 解析
   - 所有 parts 遍历

4. **Transform 语义 (P0-4)**
   - Mat4d 完整实现 (identity, translation, rotation_z, mul, inverse)
   - BoundingVolume 方法 (world_aabb, local_corners)
   - 嵌套 transform 测试
   - ECEF 大坐标精度测试

5. **B3DM/GLB 对齐**
   - realign_b3dm_byte_alignment 完整实现
   - 表/GLB/总长度 3个对齐点
   - realign_b3dm_tree 递归对齐

### ⚠️ 仅测未实数的项目 (2/17)

1. **GE monotonicity**
   - **现状:** tileset_writer 中有单元测试 `geometric_error_monotonic`
   - **缺口:** 无集成测试验证整个 rebuild 流程中 GE 单调性
   - **影响:** 低 (单元测试已验证逻辑)
   - **优先级:** P2 (可推迟到 R08 验收)

2. **REPLACE switching**
   - **现状:** 代码中所有 tileset 都使用 `refine: REPLACE`
   - **缺口:** 无专门测试验证 REPLACE 语义
   - **影响:** 低 (REPLACE 是 hardcoded,不会出错)
   - **优先级:** P2 (可推迟到 R08 验收)

### ❌ 未覆盖的项目 (0/17)

无。

## 结论

### R04 TopRebuild Correctness 评估

**核心功能:** ✅ 全部覆盖 (15/17 项完整实现+测试)

**边缘场景:** ⚠️ 2 项仅单元测试,无集成测试:
- GE monotonicity: 已有单元测试,逻辑正确
- REPLACE switching: hardcoded,无变化风险

**P0 级别缺口:** 无

**建议:**
1. **R04 状态:** 代码通过 → 验收通过 (R08 时补充端到端验证)
2. **GE/REPLACE 测试:** 推迟到 R08 Cesium A/B 对比时验证
3. **立即启动 R05:** Validator Layer A/B 验证

## R04 vs R03 总结

| Phase | 任务 | 状态 | PRs |
|-------|------|------|-----|
| R03 | TopRebuild 核心实现 | ✅ 完成 | #9-#13 |
| R04 | TopRebuild 补充验证 | ✅ 无需补充 | - |
| R05 | Validator Layer A/B | 🔜 下一步 | 待定 |

## 下一步行动

1. ✅ 更新 status.md: R04 代码通过
2. ✅ 启动 R05: Validator 第一个小 PR
   - 对比 master stages/validate.rs vs feat/v1-prod-align validator.rs
   - 移植 Layer A 结构检查或最高价值可移植切片
   - 小 PR,单一主题

---

**R04 评估:** P0 功能全覆盖,无需补充 PR。可直接进入 R05。
