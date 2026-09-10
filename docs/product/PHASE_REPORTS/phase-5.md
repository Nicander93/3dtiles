## Phase 5 完成报告

Date: **2026-09-10 ~10:15 CST** (Asia/Shanghai)  
Scope: TopRebuild core foundation — SourceBlock / Representation / Tileset adapter / selector / regular Quadtree TreeBuilder (plan §22 Phase 5, §§8–11).  
**No git push.** **No** mesh simplification, texture atlas, or ProxyBuilder geometry merge. Python rebuild baseline **not** replaced (Phase 9).

### 修改
- Root `Cargo.toml` workspace members: added `crates/top_rebuild`

### 新增文件
- `crates/top_rebuild/` — Rust-first TopRebuild core (Phase 5 models + tree)
  - `src/types.rs` — `SourceBlock`, `Representation`, `BoundingVolume`, `Mat4d`, `TreeNode`, `RebuildTree`
  - `src/adapter.rs` — Tileset Source Adapter (`tileset.json` + external `Tile_x_y` → SourceBlocks; `GRID_SPATIAL_MISMATCH`)
  - `src/selector.rs` — Representation selector (`sourceError <= targetProxyError * sourceErrorRatio`, coarsest; fallback)
  - `src/tree_builder.rs` — Regular Quadtree, bottom-up (`parent = floor(child/2)`); stub proxy nodes (bounds/transform/rep ids only)
  - `src/dump.rs` — acceptance print helpers
  - `src/bin/top_rebuild_debug.rs` — CLI dump
  - `tests/grid_4x4.rs` — fixture integration test
- `tests/fixtures/top_rebuild/grid_4x4/` — synthetic 4×4 continuous tileset (16 blocks, coarse+fine stub content)

### 如何运行
```bash
# Unit + integration
cargo test -p top_rebuild

# Acceptance print
cargo run -p top_rebuild --bin top_rebuild_debug -- \
  --input tests/fixtures/top_rebuild/grid_4x4
```

### 测试
| command | result |
|--------|--------|
| `cargo test -p top_rebuild` | **PASS** (8 unit + 2 integration) |
| `cargo run -p top_rebuild --bin top_rebuild_debug -- --input tests/fixtures/top_rebuild/grid_4x4` | **PASS** — prints `L0 16` / `L1 4` / `L2 1` + per-node bounds / transform / source representation ids |

### 验收结果（证据）
```text
L0 16
L1 4
L2 1
```
- L0: 16 source nodes, each with selected coarse `#rep0`, identity transform, 100×100 cell bounds
- L1: 4 proxies (`Proxy_L1_*`), each aggregating 4 children; transform origin = bounds center
- L2: 1 root proxy covering all 16 representation ids; box center (200,200,10), half (200,200,10)
- Selector picks coarsest complete coverage (`#rep0`, GE=50) under default ratio
- Spatial grid validation present (`GRID_SPATIAL_MISMATCH` unit test)

### 未解决 / 明确不做（本阶段）
- No mesh load / simplify / meshoptimizer
- No texture atlas / resize
- No ProxyBuilder geometry merge / parent GLB
- No TilesetWriter / REPLACE HLOD write
- Processor still uses Python `tools/experiments/rebuild_top_py` for rebuild stage
- C++ under `src/top_rebuild/` deferred (Rust models+tree sufficient for Phase 5; C++ core when meshoptimizer lands in Phase 6+)

### 下一阶段
- Phase 6: ProxyBuilder geometry prototype (B3DM→GLB, parent local frame, primitive grouping, meshoptimizer, proxy GLB)
