## Phase B

Date: **2026-09-11** Asia/Shanghai  
Scope: 三个已知正确性问题（selector fallback / 原 Block LOD 保留 / Release 禁止自动 synthesize）  
**未改 irregular-grid。未进入 Phase C。**

### 本阶段目标

TopRebuild 只应在原始 Block 上方加 HLOD，不能为了任务成功而：
- 选一个过粗的 Representation
- 把原始 LOD 树砍成 2 层
- 把读不了的城市 mesh 换成测试 box 并报告 succeeded

### 发现的问题

1. Selector 在没有任何 Representation 满足 `sourceError <= threshold` 时取 **最大** geometricError。阈值 5m、可用 50/20/10 时会选 50m，远景更糊，且没有 warning。
2. `ensure_leaf_content` 按 `representations[0]` / `[1]` 重写 `tileset.json`，原始 4+ 层 LOD、refine、子目录 URI、外部纹理都会丢。OSGBny 单块就有 L16–L21。
3. Release CLI 写死 `synthesize_if_empty: true`，`WriteOptions::default()` 同样为 true。空/坏 B3DM 会被换成盒子且任务成功。

### 修改

- Fallback 改为选 **最小** geometricError（当前最精细层），并写 `SOURCE_ERROR_TARGET_NOT_REACHED`（block / representation / threshold / selected_error）。
- 整目录复制原始 Block（`tileset.json` 及依赖），不再生成简化 LOD 树。
- 默认 `synthesize_if_empty = false`。Release 增加 `--synthesize-if-empty`。缺失/不可读/不支持 primitive 分别报 `CONTENT_MISSING` / `CONTENT_UNREADABLE` / `UNSUPPORTED_CONTENT`。
- Processor 仅在选项显式打开时才传 `--synthesize-if-empty`。
- Windows 下 `processor` 的 unused unix stub 加了 `#[allow(dead_code)]`，否则 `-D warnings` 无法 `cargo test -p processor`。

### 关键文件

```text
crates/top_rebuild/src/selector.rs
crates/top_rebuild/src/tileset_writer.rs
crates/top_rebuild/src/types.rs
crates/top_rebuild/src/adapter.rs
crates/top_rebuild/src/error.rs
crates/top_rebuild/src/b3dm.rs
crates/top_rebuild/src/glb.rs
crates/top_rebuild/src/bin/top_rebuild.rs
crates/processor/src/stages/rebuild.rs
crates/processor/src/util.rs
crates/top_rebuild/tests/phase_b.rs
```

### Tests

```powershell
cd d:\code\3dtiles
cargo test -p top_rebuild --offline
cargo test -p processor --offline
```

实际结果：

```text
cargo test -p top_rebuild --offline
cargo test: 56 passed (11 suites, 0.61s)

cargo test -p processor --offline
cargo test: 0 passed (3 suites, 0.00s)
```

processor 包没有单元测试，命令用于确认能通过 `-D warnings` 编译。

本阶段新增：

| 测试 | 覆盖 |
|------|------|
| `fallback_when_none_satisfy` | 50/20/10、threshold=2.5 → 选 10，且有 `SOURCE_ERROR_TARGET_NOT_REACHED` |
| `multi_level_original_subtree_preserved` | 4 层 GE/refine/URI 原样保留 |
| `unreadable_content_release_failure` | 坏 B3DM → `CONTENT_UNREADABLE` |
| `missing_content_release_failure` | 缺文件 → `CONTENT_MISSING` |
| `synthetic_debug_explicit_opt_in` | `--synthesize-if-empty` 才补盒子 |
| `default_write_options_do_not_synthesize` | 默认关闭 synthesize |

原 4×4 / 16×16 / transform 测试仍通过（fixture 测试显式 `synthesize_if_empty: true`）。

### Real Data Evidence

本阶段无新的真实数据跑批。Phase A 的 OSGBny 重测结果不变（`GRID_SPATIAL_MISMATCH`），与 B1–B3 无关。

### 验收

- [x] Selector fallback 取最精细层 + warning
- [x] 原始 Block LOD Tree 整目录复制，不再重写成 ≤2 层
- [x] Release 默认不 synthesize；显式 opt-in
- [x] `cargo test -p top_rebuild` 56 passed
- [x] `cargo test -p processor` 编译通过（0 tests）

### 未解决

- OSGBny 仍 `GRID_SPATIAL_MISMATCH`（坐标已排除）。分析见 `docs/product/OSGBNY_GRID_ANALYSIS.md`，**未改 TreeBuilder**。
- 没有连续 4×4～8×8 真实倾斜摄影，Phase C 还不能跑完整用户链路。
- 未宣称大范围 / 生产可用。

### 下一步

计划顺序：Phase C 真实 4×4 / 8×8。当前没有合格数据集，不能进入 C 的视觉验收。  
Phase D 分析已根据 OSGBny 重测写出，供人工决定是否扩展 sparse regular grid 校验。
