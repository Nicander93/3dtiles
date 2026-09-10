## Phase 7 完成报告

Date: **2026-09-10 ~10:25 CST** (Asia/Shanghai)  
Scope: TilesetWriter + two-level HLOD — proxy B3DM/GLB write, boundingVolume, geometricError (plan §17.3), refine REPLACE, original subtree attach, world transform invariant, full 4×4 16→4→1 (plan §22 Phase 7, §§17–19).  
**No git push.** **Texture optimization OFF.** **Do not proceed to Phase 8 until this report’s acceptance holds.** Python rebuild baseline **not** replaced (Phase 9). **No large-area claim.**

### 修改
- `crates/top_rebuild/src/lib.rs` — export `tileset_writer`
- `crates/top_rebuild/src/bin/top_rebuild_debug.rs` — subcommand `rebuild --input … --out …`
- `crates/top_rebuild/Cargo.toml` — description Phases 5–7
- `crates/top_rebuild/tests/proxy_2x2.rs` — isolate temp dirs (parallel-safe)

### 新增文件
- `crates/top_rebuild/src/tileset_writer.rs` — geometricError, relative_transform, bv_world_to_local, `rebuild_tileset`, structure probe
- `crates/top_rebuild/tests/hlod_4x4.rs` — transform invariant + full pipeline acceptance
- `tests/fixtures/top_rebuild/hlod_4x4/` — generated loadable tileset (16 leaves + 4 L1 + 1 L2)
- Output mirror: `/workspace/data/geoforge_outputs/phase7_hlod_4x4`

### 如何运行
```bash
# Unit + integration
cargo test -p top_rebuild

# Phase 7 CLI: 4×4 HLOD rebuild
cargo run -p top_rebuild --bin top_rebuild_debug -- rebuild \
  --input tests/fixtures/top_rebuild/grid_4x4 \
  --out /workspace/data/geoforge_outputs/phase7_hlod_4x4 \
  --l1-max-triangles 800 --l2-max-triangles 1200 --segments 6
```

### 测试
| command | result |
|--------|--------|
| `cargo test -p top_rebuild` | **PASS** (17 lib unit + 2 grid_4x4 + 3 proxy_2x2 + 3 hlod_4x4) |
| `top_rebuild_debug rebuild --input …/grid_4x4 --out …/phase7_hlod_4x4` | **PASS** — L0 16 / L1 4 / L2 1 |
| Headless structure probe (JSON + b3dm magic) | **PASS** — proxies=5 leaf_external=16 replace≥5 |

### 验收结果（证据）

**CLI rebuild**
```text
tileset …/phase7_hlod_4x4/tileset.json
L0 16
L1 4
L2 1
proxy Proxy_L1_* level=1 tris 1728 -> 800 (target 800) simp_err≈0.01–0.016 ge=51.000
proxy Proxy_L2_0_0 level=2 tris 3200 -> 1260 (target 1200) simp_err=0.166 ge=52.000
  (BUDGET_NOT_REACHED warning expected with LockBorder; GE still monotonic)
probe proxies=5 leaf_external=16 replace=21 max_depth=2
CLI flags: --l1-max-triangles 800 --l2-max-triangles 1200 --segments 6
```

**geometricError (plan §17.3)**
- Formula: `max(childGE) + simplificationError`, with strict `parent > child` floor (`max(child)+1` / `1.01×child`) and mild diagonal assist (not Python 0.25-only heuristic).
- Observed monotonic: leaf GE=50 → L1=51 → L2=52.

**Transform invariant**
- `relative_transform`: `parentWorld * childLocal == childWorld` (unit + nested L2/L1/L0 composition → identity).
- Proxy mesh in parent-local frame; tile `transform` = world (root) or `inv(parent)*child` (nested).
- Leaf BV remains world-authored; chain restores world centers.

**Tileset**
- `refine: REPLACE` on root, L1 proxies, and leaf wrappers.
- Root content `./Data/Proxy_L2_0_0/Proxy_L2_0_0.b3dm`; 4 L1 children each with 4 external `Tile_* /tileset.json`.
- Asset `version: 1.0` + B3DM proxies (no fake 1.1).
- Empty input stub `.b3dm` auto-synthesized as world-space boxes (Phase 7 fixture path).

**Cesium far/near**
- Headless Cesium runtime not available in this agent environment (no `node_modules/cesium`).
- Structure probe + GE monotonicity + REPLACE tree satisfy loadable tileset acceptance; visual SSE refine deferred to desktop `cesium-preview` against `phase7_hlod_4x4/tileset.json`.

### 未解决 / 明确不做（本阶段）
- No texture resize / atlas / hash dedup / KTX2 / border gap metrics (Phase 8)
- Processor still uses Python `tools/experiments/rebuild_top_py`
- No 16×16 / city-scale claim
- L2 may emit BUDGET_NOT_REACHED under LockBorder (still GE-monotonic; Phase 8 may tighten)

### 下一阶段
- Phase 8: Boundary / Texture / Budget (only after Phase 7 acceptance is confirmed in Cesium preview if required by product owner)
