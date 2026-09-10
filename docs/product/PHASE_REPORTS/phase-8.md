## Phase 8 完成报告

Date: **2026-09-10 ~10:45 CST** (Asia/Shanghai)  
Scope: Boundary / Texture / Budget — LockBorder documentation, maxGap/P95Gap, texture hash dedup, resize, maxTextureSize / maxTextureBytes / maxGlbBytes, optional KTX2 via basisu, `rebuild_metrics.json` (plan §22 Phase 8, §§15–16).  
**No git push.** **No atlas / remesh / weld.** **No Processor Python replacement (Phase 9).** **No large-area claim.**

### 修改
- `crates/top_rebuild/Cargo.toml` — deps: `image` 0.25 (png/jpeg), `sha2`, `hex`; description Phases 5–8
- `crates/top_rebuild/src/lib.rs` — export `gap`, `texture`, Phase 8 types
- `crates/top_rebuild/src/error.rs` — `BudgetExceeded`
- `crates/top_rebuild/src/glb.rs` — texture load/write, UV box helper, `make_textured_box_glb`, KHR_texture_basisu embed (with `source` for reload)
- `crates/top_rebuild/src/proxy_builder.rs` — extended `ProxyBudget`, LockBorder documented, gap + texture metrics on `ProxyBuildResult`
- `crates/top_rebuild/src/tileset_writer.rs` — budget flags on `WriteOptions`, textured synthesize, `rebuild_metrics.json`
- `crates/top_rebuild/src/bin/top_rebuild_debug.rs` — rebuild/proxy flags: `--max-texture-size`, `--max-texture-bytes`, `--max-glb-bytes`, `--ktx2`, `--strict-budget`, `--gap-warn-meters`, `--inject-test-textures`
- Tests updated for `..Default::default()` on budgets

### 新增文件
- `crates/top_rebuild/src/texture.rs` — hash dedup, resize, maxTextureBytes downscale, basisu KTX2 (PATH / `GEOFORGE_BASISU` / vcpkg), opaque KTX2 pass-through
- `crates/top_rebuild/src/gap.rs` — topological border verts, adjacent-pair maxGap / P95Gap (meters)
- `crates/top_rebuild/tests/phase8_metrics.rs` — 4×4 metrics report, KTX2 availability honesty, texture byte budget warn
- Output: `/workspace/data/geoforge_outputs/phase8_hlod_4x4` (+ `rebuild_metrics.json`)

### 如何运行
```bash
cargo test -p top_rebuild

cargo run -p top_rebuild --bin top_rebuild_debug -- rebuild \
  --input tests/fixtures/top_rebuild/grid_4x4 \
  --out /workspace/data/geoforge_outputs/phase8_hlod_4x4 \
  --l1-max-triangles 800 --l2-max-triangles 1200 --segments 6 \
  --max-texture-size 128 --max-texture-bytes 2097152 --max-glb-bytes 33554432 \
  --ktx2
```

### 测试
| command | result |
|--------|--------|
| `cargo test -p top_rebuild` | **PASS** (25 lib + 2 grid_4x4 + 3 proxy_2x2 + 3 hlod_4x4 + 3 phase8_metrics) |
| CLI rebuild 4×4 with budgets + `--ktx2` | **PASS** — L0 16 / L1 4 / L2 1 + metrics JSON |
| basisu at `vcpkg_installed/x64-linux/tools/basisu/basisu` | **found** — KTX2 encode OK |

### 验收结果（证据）

**CLI rebuild (excerpt)**
```text
tileset …/phase8_hlod_4x4/tileset.json
metrics …/phase8_hlod_4x4/rebuild_metrics.json
L0 16
L1 4
L2 1
gaps maxGap=60.000000 P95Gap=60.000000 pairs=4
textures unique=32 … ktx2_avail=true ktx2_enc=32
totals tris_after=4466 texture_bytes=10608 glb_bytes=225724
probe proxies=5 leaf_external=16 replace=21 max_depth=2
```

**Border protection (plan §15.2)**
- Simplify always uses meshoptimizer `LockBorder | ErrorAbsolute` (default `lock_border=true`).
- No cross-tile weld (V1).
- `BUDGET_NOT_REACHED` under LockBorder still possible (same as Phase 7 L2); warnings include `[LockBorder=true]`.

**Gap metrics (plan §15.3)**
- `maxGap` / `P95Gap` written in `rebuild_metrics.json` and per-proxy.
- On synthetic 4×4 boxes (half-extent < tile spacing), measured gaps ≈ 33 m (L1) / 60 m (L2) — **expected calibration gap between non-touching boxes**, not a seam regression. Emitted as `GAP_WARN` (not a hard fail).

**Texture path (plan §16)**
- SHA-256 hash dedup; resize to `maxTextureSize`; iterative downscale toward `maxTextureBytes`; `GLB_BYTES_OVER_BUDGET` / `TEXTURE_BYTES_OVER_BUDGET` warnings (hard error with `--strict-budget`).
- Empty fixture leaves get solid test textures when `--inject-test-textures` (default on for synthesize) so the path is exercised.
- Original UVs preserved; **no atlas / UV remap / baking**.

**KTX2**
- Optional via `--ktx2` → `basisu -ktx2 -etc1s` when CLI present; honest warning + PNG if unavailable.
- Embedded as `image/ktx2` + `KHR_texture_basisu` (with `source` so parent proxy reload parses).
- Parent levels pass through opaque KTX2 without re-decode when possible.

**Metrics report**
- Written next to tileset: `rebuild_metrics.json` (phase, budgets, gaps, textures, totals, proxies, warnings).

### 未解决 / 明确不做（本阶段）
- Processor still uses Python `rebuild_top` (Phase 9)
- No atlas / remesh / weld
- No 16×16 / city-scale claim
- Gap numbers on synthetic boxes are calibration (real OSGB adjacency will differ)
- Full Cesium visual SSE check deferred to desktop preview

### 下一阶段
- Phase 9: replace Processor Python rebuild with TopRebuild Core / CLI (keep Python baseline for regression)
