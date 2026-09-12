## Phase A

Date: **2026-09-11** Asia/Shanghai  
Scope: Coordinate / Transform correctness（计划 Phase A：A1–A5）  
**未进入 Phase B。未改 irregular-grid 算法。**

当前准确描述仍为：

```text
TopRebuild algorithm implemented and validated on regular synthetic grids.
Real-world dataset validation is still in progress.
```

### 本阶段目标

真实 3D Tiles 大量使用 tile / external tileset / glTF node / ECEF transform。  
synthetic fixture 几乎全是 identity，无法暴露 bounds 坐标系混用。  
本阶段把「一个 bounds 在哪个坐标系」钉死，并保证 grid 校验与 ProxyBuilder 使用同一套 world 信息。

### 发现的问题

1. `BoundingVolume::aabb_min_max()` 把 `box` 当成对角 half-extent，而不是 Cesium 的 3 个 half-axis vector。旋转 OBB 的 AABB 是错的。
2. `validate_grid_spatial()` 直接读 local `bounds.center()`。local center 全是 `(0,0,0)`、靠 tile transform 拉开的规则网格会被判成 `GRID_SPATIAL_MISMATCH`。
3. 若把 world center 的 ECEF X/Y 直接当平面坐标，带 root ECEF 旋转的规则网格也会误报。比较必须在一个公共局部笛卡尔系里做。
4. L0 `NodePayload.world_transform` 被写死为 identity，注释假设「mesh 已经在 world」。只对 synthetic 成立。真实 B3DM 顶点在 content local，Representation 的 world transform 进 ProxyBuilder 时丢失。
5. B3DM `RTC_CENTER` 被整段跳过。顶点相对 RTC，未加到 content→parent 变换上。

OSGBny 的 `Tile_x_y` 本身是稀疏抽样（见下方），即使坐标修对，也不应期待它通过规则网格校验。

### 修改

- 明确坐标约定：mesh = content local；`Representation.world_transform` = content local→world；`SourceBlock.bounds` = block local；`TreeNode.bounds` = world AABB。
- `corners()` / `transform_bounds()` / `world_bounds()`：`center ± axisX ± axisY ± axisZ` 八角点，再变到目标系后包 AABB。
- Grid 校验：先把 center 变到 world，再变到 origin block 的 local frame，用该 frame 的 X/Y 比间距。
- TreeBuilder L0 bounds 改为 `block.bounds.world_bounds(world_transform)`。
- L0 payload 携带选中 Representation 的 `world_transform`，不再写 identity。
- 解析并应用 B3DM `RTC_CENTER`（`tile_transform * T(RTC) * vertex`）。

### 关键文件

```text
crates/top_rebuild/src/types.rs
crates/top_rebuild/src/adapter.rs
crates/top_rebuild/src/tree_builder.rs
crates/top_rebuild/src/tileset_writer.rs
crates/top_rebuild/src/b3dm.rs
crates/top_rebuild/src/proxy_builder.rs
crates/top_rebuild/tests/transform_invariants.rs
```

### Tests

```powershell
cd d:\code\3dtiles
cargo test -p top_rebuild --offline
```

实际结果：

```text
cargo test: 51 passed (10 suites, 0.70s)
```

本阶段新增（不止于 “tests passed”）：

| 测试 | 覆盖 |
|------|------|
| `obb_corners_and_translated_world_aabb` | local box + translate → world AABB |
| `rotated_half_axes_aabb_uses_corners` | 非对角 half-axes |
| `grid_spatial_valid_when_centers_come_from_tile_transform` | local center=0 + tile translate |
| `grid_spatial_valid_under_shared_ecef_rotation` | 共享 ECEF 旋转不误报 |
| `rtc_center_roundtrip` / `rtc_center_applied_into_parent_local` | B3DM RTC |
| `local_bounds_plus_tile_transform_is_regular_grid` | adapter 加载 |
| `world_bounds_union_after_child_transforms` | world union |
| `external_tileset_cumulative_transform` | root × child × ext_root |
| `parent_local_and_world_roundtrip` | parent local |
| `ecef_large_coordinate_parent_local_proxy` | 大 ECEF；proxy 顶点不在百万米量级 |
| `gltf_node_transform_baked_on_load` | glTF node matrix |
| `remount_accumulated_world_matches_source` | 重挂载前后 world 一致 |

原 4×4 / 16×16 / proxy / hlod 测试仍通过。

### Real Data Evidence

| 项 | 结果 |
|----|------|
| dataset | `data/real/OSGBny/OSGBny` → `data/real/OSGBny_3dtiles` |
| convert | `docker run winner1/3dtiles:1.0 _3dtile -f osgb`，0.66 s |
| block count | 6：`Tile_+005_+033`, `+006_+005`, `+006_+006`, `+006_+007`, `+006_+009`, `+032_+031` |
| root transform | 真实 ECEF 矩阵（translation ≈ `-2.36e6, 4.60e6, 3.72e6`） |
| child transform | 无（identity）；`boundingVolume.box` 已在 ENU/local 米制 |
| 内部 LOD | 原样转到多层 `.b3dm`（如 `Tile_+006_+006` 含 L16–L21） |

```powershell
.\target\debug\top_rebuild.exe -i data\real\OSGBny_3dtiles -o data\real\OSGBny_rebuild
```

实际输出：

```text
GRID_SPATIAL_MISMATCH: block Tile_+006_+005 grid=(6,5) center=(325.003,284.942) expected≈(343.924,275.613) dist=21.095 tol=17.286
```

与 Phase 10 字符串一致。Phase A 把 center 变到 origin-local 后，数值仍是 local ENU `(325.003, 284.942)`，说明 **不是 tile transform / ECEF 把中心叠在一起**。  
`Tile_x_y` 是空间索引，但这 6 块是离散抽样（`+005_+033`、`+032_+031` 远离 `+006_*`），规则网格外推必然失败。诚实停止，不改 TreeBuilder。

### 验收

- [x] Bounds / Transform 坐标语义明确（`world_bounds` / `transform_bounds`）
- [x] Grid validation 基于 world-space（再投到 origin local，避免 ECEF XY 误报）
- [x] Representation transform 进入 ProxyBuilder / L0 payload，不再写死 identity
- [x] `cargo test -p top_rebuild` 全部通过（51）
- [x] 新增坐标系 regression tests（上表）
- [x] 重新跑 OSGBny 转换后的 3D Tiles CLI：仍为 `GRID_SPATIAL_MISMATCH`（与 Phase 10 同一条，坐标问题已排除）

### 未解决

- OSGBny 6 块稀疏集仍 `GRID_SPATIAL_MISMATCH`（已确认不是 transform/bounds bug）。分析报告留给 Phase D，不改 TreeBuilder。
- 原始 Block 内部 LOD 仍被 `ensure_leaf_content` 重写成最多 2 层（Phase B2）。
- Selector 无候选时仍 fallback 到最粗层（Phase B1）。
- Release CLI 仍 `synthesize_if_empty: true`（Phase B3）。
- 未宣称大范围 / 生产可用。

### 下一步

Phase B：selector fallback、原 Block LOD 原样保留、Release 默认禁止 synthesize。  
完成后停止，再交 Phase B 报告。不在本阶段改 irregular-grid。
