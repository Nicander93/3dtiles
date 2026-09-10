# Architecture rebuild — Phase 0–11 SUMMARY

Date: **2026-09-10** Asia/Shanghai  
Authority: [`../03-v1-architecture-rebuild-plan.md`](../03-v1-architecture-rebuild-plan.md)  
**No git push** in this series. Individual reports: `phase-0.md` … `phase-10.md`.

## Bottom line

| Area | Status |
|------|--------|
| Desktop shell | Tauri 2 + React/Cesium; Qt / OSGB native preview **cancelled for V1** |
| Processor | Independent Rust processor + JSONL; SQLite via Tauri |
| Convert | Existing OSGB→3D Tiles converter reused |
| TopRebuild | Rust `top_rebuild` is formal path; Python baseline retained under `tools/experiments/` |
| Scale | **4×4 + 16×16 synthetic PASS**; OSGBny sparse **FAIL honest**; **no 城区/百平方公里** |
| Large-area claim | **NOT allowed** — use: *algorithm complete for continuous regular grids ≤16×16 synthetic; large-scale validation pending* |

## Phase ladder

| Phase | Scope | Result |
|-------|--------|--------|
| **0** | Freeze baseline + decision docs | Done — plan in-repo; Qt/OSGB preview cancelled in docs; baseline smoke |
| **1** | Tauri shell + React | Done — `apps/desktop`; OSGB Preview UI removed |
| **2** | Tauri Task Store / resource server | Done — SQLite + local preview server |
| **3** | Processor | Done — convert / process-tileset / JSONL / cancel |
| **4** | Directory cleanup | Done — web→desktop; experiments/; Qt apps removed from product tree |
| **5** | TopRebuild models + TreeBuilder | Done — SourceBlock / adapter / selector / quadtree; grid_4x4 L0 16→4→1 tree |
| **6** | ProxyBuilder geometry | Done — meshoptimizer, parent-local, proxy GLB |
| **7** | TilesetWriter + HLOD | Done — REPLACE, GE, 4×4 16→4→1 loadable tileset |
| **8** | Boundary / texture / budget | Done — LockBorder, gaps, hash/resize, optional KTX2, `rebuild_metrics.json` |
| **9** | Replace Python rebuild in Processor | Done — default Rust `top_rebuild`; Python via `GEOFORGE_REBUILD_ENGINE=python` only |
| **10** | Scale validation ladder | **Partial** — 4×4 + **16×16** synthetic PASS; OSGBny `GRID_SPATIAL_MISMATCH`; no urban/百km² data |

## Phase 10 numbers (record)

| Dataset | Levels | Time | maxRSS | Output | Notes |
|---------|--------|------|--------|--------|-------|
| grid_4x4 (16 leaves) | L0–L2 | 0.321 s | ~14 MB | ~1.1 MB | reconfirm |
| grid_16x16 (256 leaves) | L0–L4 | 0.979 s | ~17 MB | ~13 MB | proxies=85, replace=341 |
| OSGBny (6 tiles) | — | fail | — | — | `GRID_SPATIAL_MISMATCH` |
| 城区 / 百平方公里 | — | n/a | — | — | **no data** |

## Supported vs not (TopRebuild V1)

**Supported**
- Continuous regular `Tile_+x_+y` grids with spatially consistent indices
- Proxy HLOD REPLACE pyramid via Rust core + Processor
- Metrics JSON; optional KTX2 when basisu present
- Demonstrated up to **16×16 / 256** synthetic blocks

**Not supported / not claimed**
- Sparse irregular Tile sets (OSGBny-class)
- Invented coverage / silent regrouping on mismatch
- Real urban / 百平方公里 production rebuild
- Atlas / remesh / cross-tile weld / adaptive trees

## How to verify quickly
```bash
cargo test -p top_rebuild
cargo build -p top_rebuild --bin top_rebuild -p processor
./target/debug/top_rebuild -i tests/fixtures/top_rebuild/grid_16x16 \
  -o /tmp/grid16_out --segments 4
```

## Reports index
- [phase-0.md](./phase-0.md) … [phase-11.md](./phase-11.md)

## Phase 11 (production readiness)
Correctness hardening P0-1..P0-4 in `top_rebuild`: no synthetic release path; original Block subtree preservation + `subtree_preservation.json`; coverage frontiers; oriented/world BV. See [phase-11.md](./phase-11.md). Authority: [`../04-v1-production-readiness-plan.md`](../04-v1-production-readiness-plan.md).
