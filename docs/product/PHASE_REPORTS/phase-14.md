## Phase 14 完成报告

Date: **2026-09-10 ~15:50 CST** (Asia/Shanghai)  
Scope: Zero-Python Release Runtime — plan `04-v1-production-readiness-plan.md` §7.  
**Dedicated git commit** + push to `origin/feat/v0-scaffold` when green.

### 目标

| Item | Result |
|------|--------|
| Processor release path without Python on PATH | **PASS** — convert / rebuild keep / KTX2 via Rust+basisu |
| TopRebuild default `GEOFORGE_REBUILD_ENGINE=rust` | **PASS** (Phase 9); Python only with explicit env |
| Converter resolve relative to app/resources (not `/workspace` hard-required) | **PASS** — sidecar / resources / env override; `/workspace` optional probe only |
| KTX2 prefer basisu sidecar; Python experiments-only | **PASS** — Rust walker in `processor::stages::texture_ktx2` |
| Tauri `externalBin` / resources for Linux | **PASS** — configured + `scripts/release/stage_sidecars.sh` |
| Smoke: unset PYTHONPATH; convert+rebuild keep; ktx2 if basisu | **PASS** — logs prove no python invocation |
| `PHASE_REPORTS/phase-14.md` honest about sidecars | **this file** |

### What still needs sidecars (not fully static)

Release is **zero-Python**, not “single static binary with everything linked”:

| Component | Ship as | Notes |
|-----------|---------|-------|
| `processor` | Tauri `externalBin` sidecar | Required |
| `top_rebuild` | sidecar (or future in-process lib) | Currently sibling CLI |
| `basisu` | sidecar CLI | KTX2 only; keep mode needs none |
| `_3dtile` + OSG/GDAL libs | `resources/bin/**` | Native converter + shared libs; **not** fully static |
| Cesium / UI assets | app resources | unchanged |

Windows installer / clean-machine E2E remains **Phase 18**.

### Env contract (developer overrides only)

| Env | Role |
|-----|------|
| *(default)* | Rust rebuild + Rust KTX2 walker |
| `GEOFORGE_PROCESSOR` / `GEOFORGE_TOP_REBUILD` / `GEOFORGE_3DTILE` / `GEOFORGE_BASISU` | Path overrides |
| `GEOFORGE_RESOURCES` | Extra search root for sidecars |
| `GEOFORGE_REBUILD_ENGINE=python` | Experiments baseline only |
| `GEOFORGE_TEXTURE_ENGINE=python` | Experiments texture_ktx2 only |

### 修改摘要

- `crates/processor/src/util.rs` — sidecar-first tool resolution; `basisu` on `ToolPaths`
- `crates/processor/src/stages/texture_ktx2.rs` — **NEW** Rust B3DM/GLB/glTF KTX2 walker (basisu CLI)
- `crates/processor/src/stages/texture.rs` — default engine=rust; Python behind env
- `crates/top_rebuild/src/glb.rs` — `parse_gltf_lenient` demotes unknown `extensionsRequired` (e.g. `KHR_techniques_webgl` from `_3dtile`)
- `crates/top_rebuild/src/texture.rs` — basisu next to exe / resources first
- `apps/desktop/src-tauri/tauri.conf.json` — `externalBin` + `resources/bin/**`
- `apps/desktop/src-tauri/src/process_manager.rs` / `commands.rs` — no `/workspace` required for health/capabilities
- `scripts/release/stage_sidecars.sh` / `smoke_zero_python.sh` — **NEW**
- `tools/texture_ktx2/README.md` — experiments only
- Commit inject-fail serialized (`with_inject_fail_stage_to_final`) to fix parallel test flake

### Smoke evidence

```bash
./scripts/release/stage_sidecars.sh
./scripts/release/smoke_zero_python.sh
```

Result (**2026-09-10**):

- convert-osgb + `--rebuild-top` `--texture keep` → **exit 0**
- Logs: `[rebuild] engine=rust`, `[texture] mode=keep — skipped (no Python)`
- **No** `python` / `rebuild_top.py` / `texture_ktx2.py` in command lines
- `process-tileset --texture ktx2-etc1s` → `[texture] engine=rust basisu=...`, 20 files / 30 textures, Layer A OK
- `which python` may exist on the machine; it is **not called** on the release path

Log artifact: `/tmp/geoforge_phase14_zero_py/convert_rebuild.log` (+ `ktx2.log`)

### Tests

| command | result |
|---------|--------|
| `cargo test -p processor` | **PASS** |
| `cargo test -p top_rebuild --lib` | **PASS** |
| `scripts/release/smoke_zero_python.sh` | **PASS** |

### 诚实说明 / NOT production ready

- **Not production-ready** — Phase 15–18 (acceptance harness, HK PlanD real data, Cesium A/B, Windows clean package) remain.
- Sidecars must be **staged** before `tauri build` (`stage_sidecars.sh`); binaries are gitignored.
- Converter still depends on native shared libraries under `resources/bin/lib` (OSG/GDAL) — capability check should fail clearly if missing (Phase 18 polish).
- Do **not** download Hong Kong PlanD data in this phase.
- Grid fixtures under `tests/fixtures/top_rebuild/grid_*` intentionally ship empty `.b3dm` placeholders; release tests synthesize only via debug opts / filled copies.

### 未做（后续）

- Phase 15 acceptance harness  
- Phase 16–17 HK real data + Cesium A/B  
- Phase 18 Windows package + clean-machine E2E  
- Optional: link `top_rebuild` as processor library (plan §7.2 方案 B) to drop one process  
