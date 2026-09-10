## Phase 9 完成报告

Date: **2026-09-10 ~10:35 CST** (Asia/Shanghai)  
Scope: Replace Processor Python rebuild with TopRebuild Rust core / `top_rebuild` CLI (plan §22 Phase 9).  
**No git push.** **Python baseline not deleted.** **No 百平方公里 / large-area claim (Phase 10).**

### 修改
- `crates/top_rebuild/Cargo.toml` — add release binary `top_rebuild`; description Phases 5–9
- `crates/top_rebuild/src/bin/top_rebuild.rs` — **new** release CLI: `-i/--input`, `-o/--output|--out`, `--levels`, budgets, optional `--ktx2` (stable args for Processor)
- `crates/top_rebuild/src/lib.rs` — Phase 9 header note
- `crates/processor/src/util.rs` — resolve `top_rebuild` via `GEOFORGE_TOP_REBUILD` → sibling of processor → `target/{release,debug}/top_rebuild`
- `crates/processor/src/stages/rebuild.rs` — **default engine = Rust `top_rebuild`**; Python only when `GEOFORGE_REBUILD_ENGINE=python` (aliases `py` / `baseline`)
- `tools/experiments/rebuild_top_py/README.md` — baseline only; formal path is Rust

### 新增文件
- `crates/top_rebuild/src/bin/top_rebuild.rs`
- `docs/product/PHASE_REPORTS/phase-9.md`
- E2E outputs under `/workspace/data/geoforge_outputs/`:
  - `phase9_grid4x4_rust/` (REPLACE HLOD 16→4→1)
  - `phase9_one_tile_process/`
  - `phase9_one_tile_convert_rebuild/`
  - `phase9_python_fallback_smoke/` (explicit engine=python only)

### Env contract
| Env | Role |
|-----|------|
| *(default)* | Rust `top_rebuild` |
| `GEOFORGE_TOP_REBUILD` | Path override for Rust binary |
| `GEOFORGE_REBUILD_ENGINE=python` | Explicit Python baseline fallback |
| `GEOFORGE_REBUILD_TOP` | Python script path when engine=python |

### 如何运行
```bash
cargo build -p top_rebuild --bin top_rebuild -p processor
export GEOFORGE_TOP_REBUILD=$PWD/target/debug/top_rebuild

# Formal path (no Python rebuild)
./target/debug/processor process-tileset \
  -i tests/fixtures/top_rebuild/grid_4x4 \
  -o /workspace/data/geoforge_outputs/phase9_grid4x4_rust \
  --rebuild-top --levels 2 --texture keep

# Optional regression only
GEOFORGE_REBUILD_ENGINE=python ./target/debug/processor process-tileset ...
```

### 测试
| command | result |
|--------|--------|
| `cargo build -p top_rebuild --bin top_rebuild -p processor` | **PASS** |
| `cargo test -p top_rebuild` | **PASS** (25 lib + grid/proxy/hlod/phase8 integration) |
| `npm run build` (`apps/desktop`) | **PASS** |
| `processor process-tileset` grid_4x4 + rebuildTop | **PASS** — engine=rust, L0 16 / L1 4 / L2 1, replace=21 |
| `processor process-tileset` one_tile convert output + rebuildTop | **PASS** — engine=rust, no python |
| `processor convert-osgb` one_tile + `--rebuild-top` | **PASS** — convert then `top_rebuild`, no python |
| `GEOFORGE_REBUILD_ENGINE=python` one_tile process | **PASS** — baseline still available |
| OSGBny_3dtiles (6 sparse tiles) | **FAIL honest** — `GRID_SPATIAL_MISMATCH` (no silent wrong tree) |

### 验收结果（证据）

**Formal user path has no Python rebuild dependency**
- Logs (grid_4x4 / one_tile):
  - `[rebuild] engine=rust binary=.../target/debug/top_rebuild`
  - `$ .../top_rebuild -i ... -o ...`
  - **No** `python` / `rebuild_top.py` in default-engine logs
- Artifact `/workspace/data/geoforge_outputs/phase9_grid4x4_rust`:
  - `root.refine = REPLACE`, replace_count=21
  - Proxy dirs: `Proxy_L1_0_0`, `Proxy_L1_0_1`, `Proxy_L1_1_0`, `Proxy_L1_1_1`, `Proxy_L2_0_0`

**ps / logs**
- Rebuild completes in &lt;1s on these fixtures; process list evidence is the processor JSONL `$` line spawning `top_rebuild` (not python). Sample:
  ```text
  $ /workspace/repos/3dtiles/target/debug/top_rebuild -i .../grid_4x4 -o .../work --levels 2
  top_rebuild engine=rust-core ...
  probe proxies=5 leaf_external=16 replace=21 max_depth=2
  ```

### 未解决 / 明确不做（本阶段）
- OSGBny sparse 6-tile convert output correctly rejected by grid spatial check (plan §9.4) — needs contiguous block set / Phase 10 scale work, not a silent Python fallback
- one_tile alone cannot demonstrate multi-level proxies (L0 size=1); REPLACE pyramid proven on grid_4x4 via same Processor path
- No atlas / remesh / weld
- No 百平方公里 claim

### 下一阶段
- Phase 10: large-dataset validation 4×4 → 16×16 → 城区 → 百平方公里 (metrics + Cesium far-view request comparison)
