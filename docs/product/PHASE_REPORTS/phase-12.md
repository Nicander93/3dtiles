## Phase 12 完成报告

Date: **2026-09-10 ~15:40 CST** (Asia/Shanghai)  
Scope: True validator layers A/B — plan `04-v1-production-readiness-plan.md` §5.  
**Local git commit**; push to `origin/feat/v0-scaffold` if `GH_TOKEN` works.

### 目标

| Item | Result |
|------|--------|
| **Layer A** built-in fast recursive validator before commit | **PASS** |
| Wire into processor validate stage (replace weak exists/JSON check) | **PASS** |
| Layer A tests (missing content / corrupt B3DM / cycles / escape) | **PASS** |
| **Layer B** CesiumGS `3d-tiles-validator` acceptance script + docs | **PASS** |
| Layer B runs on known-good rebuild output with `numErrors=0` | **PASS** |
| `PHASE_REPORTS/phase-12.md` | **this file** |

### Layer A（processor）

New module: `crates/processor/src/validator.rs`

Pre-commit checks:

- `asset` / `root` required
- `geometricError` finite and ≥ 0
- `refine` ∈ {REPLACE, ADD} when present
- `boundingVolume` box[12] / region[6] / sphere[4] with finite numbers
- `transform` length 16 + finite
- content URI resolve; reject path escape outside output root (`CONTENT_URI_ESCAPE`)
- recurse external tilesets; detect cycles (`CYCLE_DETECTED`)
- content file exists (`CONTENT_MISSING`); size floor (`CONTENT_TOO_SMALL`)
- B3DM / GLB basic header checks (`CONTENT_INVALID`)
- clear codes — **no silent pass**
- writes `validation_internal.json` under the staged output

Wired in `crates/processor/src/stages/validate.rs` (used by convert-osgb + process-tileset pipelines).

CLI: `processor validate-tileset --path <dir|tileset.json>`

### Layer B（dev / acceptance only）

- Doc: `docs/product/LAYER_B_VALIDATOR.md`
- Script: `scripts/acceptance/run_layer_b_validator.sh`
- Gate: report `numErrors == 0` (Node **not** required in desktop release)

```bash
scripts/acceptance/run_layer_b_validator.sh \
  /path/to/output/tileset.json \
  acceptance-results/phase12-layer-b/validator-report.json
```

### B3DM packer fix (enabler for Layer B)

`crates/top_rebuild/src/b3dm.rs` `pack_glb_as_b3dm`:

- Feature/batch JSON padded so sections **end** on 8-byte boundaries (header is 28 bytes)
- Empty batch table `{}` (no typeless `batchId`)
- Tile `byteLength` multiple of 8; extract respects GLB `length` field

### 测试证据

```text
cargo test -p processor          # unit 5 + phase12_layer_a 4 PASS
cargo test -p top_rebuild        # all PASS (incl. b3dm alignment unit test)
processor validate-tileset --path phase12_layer_b_rebuild   # Layer A ok
scripts/acceptance/run_layer_b_validator.sh …               # numErrors=0
```

Layer A on rebuild output: tilesets=17, contents=37, external=16, errors=0.  
Layer B on same output: `numErrors=0`.

### 诚实说明

- **Not production-ready** — Phase 13+ (atomic commit, packaging, real HK data) remain.
- Layer B is acceptance/CI only; release runtime stays zero-Node.
- Older rebuild artifacts (e.g. pre-fix `phase8_hlod_4x4`) may still fail Layer B alignment until rebuilt.
- Do not commit `node_modules` / `acceptance-results/` (gitignored).

### 未做（按计划留给后续 Phase）

- Phase 13 atomic sibling staging / cancel / rollback
- Deep glTF POSITION/index/NaN walks beyond Layer A headers (optional hardening)
- Remesh / Atlas / implicit tiling
