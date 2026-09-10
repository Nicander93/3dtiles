## Phase 10 完成报告

Date: **2026-09-10 ~10:40 CST** (Asia/Shanghai)  
Scope: Scale validation ladder — 4×4 → 16×16 → (urban / larger if data exists) (plan §22 Phase 10, §20).  
**No git push.** **`--allow-partial-grid` not implemented** (plan prefers stop on `GRID_SPATIAL_MISMATCH`; documented refusal).

### Large-area claim (honest)

**NOT allowed** to write: `支持大范围倾斜摄影顶层重建` / 百平方公里生产可用.

**Allowed status line:**

```text
V1 algorithm complete for continuous regular grids (demonstrated ≤16×16 / 256 blocks synthetic);
large-scale / sparse / urban validation pending — not claimed.
```

### 修改
- `crates/top_rebuild/Cargo.toml` — description Phases 5–10
- `crates/top_rebuild/src/adapter.rs` — document Phase 10 refusal on `GRID_SPATIAL_MISMATCH` (no silent wrong tree; no partial-grid invent)
- `crates/top_rebuild/src/tileset_writer.rs` — clearer multi-root error (need enough `--levels` / omit for full pyramid)
- `crates/processor/src/stages/rebuild.rs` — Rust path: `levels <= 0` omits `--levels` → full pyramid until single root
- `crates/processor/src/main.rs` — CLI help for `--levels 0`

### 新增文件
- `tests/fixtures/top_rebuild/grid_16x16/` — synthetic 16×16 continuous tileset (256 leaves)
- `crates/top_rebuild/tests/grid_16x16.rs` — L0 256 / L1 64 / L2 16 / L3 4 / L4 1
- `scripts/generate_grid_fixture.py` — regenerate N×N (power of 2) fixtures
- `docs/product/PHASE_REPORTS/phase-10.md`
- `docs/product/PHASE_REPORTS/SUMMARY.md` (Phases 0–10)
- Outputs under `/workspace/data/geoforge_outputs/`:
  - `phase10_4x4/` (+ `rebuild_metrics.json`, run log)
  - `phase10_16x16/` (+ metrics)
  - `phase10_4x4_processor/`, `phase10_16x16_processor/`
  - `phase10_osgbny_run.log` (expected fail)

### 如何运行
```bash
# Fixture (already checked in)
python3 scripts/generate_grid_fixture.py --n 16 --out tests/fixtures/top_rebuild/grid_16x16

cargo build -p top_rebuild --bin top_rebuild -p processor
export GEOFORGE_TOP_REBUILD=$PWD/target/debug/top_rebuild

# Ladder step 1 — 4×4
./target/debug/top_rebuild -i tests/fixtures/top_rebuild/grid_4x4 \
  -o /workspace/data/geoforge_outputs/phase10_4x4 \
  --levels 2 --segments 6 --inject-test-textures \
  --l1-max-triangles 800 --l2-max-triangles 1200 --max-texture-size 128

# Ladder step 2 — 16×16 full pyramid (omit --levels or use processor --levels 0)
./target/debug/top_rebuild -i tests/fixtures/top_rebuild/grid_16x16 \
  -o /workspace/data/geoforge_outputs/phase10_16x16 \
  --segments 4 --l1-max-triangles 2000 --l2-max-triangles 3000 --max-texture-size 64

./target/debug/processor process-tileset \
  -i tests/fixtures/top_rebuild/grid_16x16 \
  -o /workspace/data/geoforge_outputs/phase10_16x16_processor \
  --rebuild-top --levels 0 --texture keep
```

### 测试
| command | result |
|--------|--------|
| `cargo test -p top_rebuild` | **PASS** (25 lib + grid_4x4 + **grid_16x16** + proxy/hlod/phase8) |
| `cargo build -p top_rebuild --bin top_rebuild -p processor` | **PASS** |
| top_rebuild 4×4 | **PASS** — L0 16 / L1 4 / L2 1; 0.321 s; maxRSS ≈14 MB |
| top_rebuild 16×16 | **PASS** — L0 256 / L1 64 / L2 16 / L3 4 / L4 1; 0.979 s; maxRSS ≈17 MB |
| processor 4×4 `--levels 2` | **PASS** — engine=rust |
| processor 16×16 `--levels 0` | **PASS** — full pyramid, replace=341 |
| OSGBny / osgbny_p4_keep | **FAIL honest** — `GRID_SPATIAL_MISMATCH` |
| denser public OSGB under `/workspace/data` | **none** (only OSGBny 6-tile + one_tile + synthetic 16×16) |

### 验收结果（证据）

#### Ladder 4×4 (reconfirm Phases 7–9)
```text
L0 16 / L1 4 / L2 1
probe proxies=5 leaf_external=16 replace=21 max_depth=2
ELAPSED_SEC=0.321 MAX_RSS_KB=14404
totals tris_after=4466 texture_bytes=10552 glb_bytes=223920
output volume ≈1.1M
```

#### Ladder 16×16 (256 blocks)
```text
L0 256 / L1 64 / L2 16 / L3 4 / L4 1
probe proxies=85 leaf_external=256 replace=341 max_depth=4
ELAPSED_SEC=0.979 MAX_RSS_KB=17164
totals tris_after=121952 texture_bytes=0 glb_bytes=4106040
output volume ≈13M
metrics: /workspace/data/geoforge_outputs/phase10_16x16/rebuild_metrics.json
```
Note: synthetic empty leaf stubs + box synthesize; GAP_WARN / some BUDGET_NOT_REACHED under LockBorder on upper levels — same class as Phase 7/8, not a silent tree invent.

#### Real data / urban / 百平方公里
- **OSGBny full (6 sparse Tile_*):**  
  `GRID_SPATIAL_MISMATCH: block Tile_+006_+005 grid=(6,5) center=(325.003,284.942) expected≈(343.924,275.613) dist=21.095 tol=17.286`  
  → **known limit** of regular-grid spatial check on sparse/non-rect / non-uniform spacing sets. **No silent wrong tree.**
- **城区 / 百平方公里:** **no dataset available** under `/workspace/data` this session → **not demonstrated**.
- Cesium far-view request/download comparison: **not run** (no headless Cesium metrics for 16×16 in this phase).

### What IS supported
- Continuous **regular** `Tile_+x_+y` grids whose centers match grid indices within tolerance.
- Demonstrated rebuild: **4×4 (16→4→1)** and **16×16 (256→64→16→4→1)** synthetic, REPLACE HLOD, metrics JSON, Processor Rust path (`--levels 0` = full pyramid).
- Honest stop on `GRID_SPATIAL_MISMATCH`.

### What is NOT supported (exact limits)
- Sparse / irregular `Tile_*` sets (OSGBny-class): rejected, not rebuilt.
- Incomplete grids with invented coverage / silent regrouping.
- `--allow-partial-grid` / maximal contiguous 2ⁿ subset rebuild: **not implemented** (documented refusal preferred).
- Real urban / multi-km² / **百平方公里** oblique photography: **not validated** (no data + no claim).
- Processor default `--levels 1` is insufficient for 16×16 (multi-root error unless `--levels ≥ 4` or `--levels 0`).

### 未解决 / 明确不做（本阶段）
- No atlas / remesh / weld / adaptive tree
- No Cesium request-count A/B for 16×16
- No production packaging claim
- No 百平方公里 claim

### 下一阶段
- Architecture rebuild Phases 0–10 ladder closed at **partial large-scale evidence** (synthetic 16×16 only).
- Next product work (outside this phase): obtain contiguous regular-grid real OSGB (≥16×16 or urban), re-run ladder, only then reconsider large-area wording.
