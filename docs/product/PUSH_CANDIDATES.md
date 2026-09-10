# Push candidates (do NOT push/commit unless asked)

Date: 2026-09-10 Asia/Shanghai.
Purpose: list major **product** paths after Phase 4 cleanup.
**No git push / no commit performed by this note.**

## Major paths

### Apps / product shell
- `apps/desktop/` — React + Vite + Tauri 2 (`src-tauri`, crate `geoforge-desktop`)
- `crates/processor/` — Rust processor CLI (JSONL events)
- `tools/experiments/desktop_server_py/` — legacy FastAPI reference (former `apps/desktop_server`)
- ~~`apps/geoforge_shell/`~~ / ~~`apps/osgb_viewer/`~~ — **deleted Phase 4**

### Scripts / runtime
- `scripts/run_geoforge.sh` — prints Tauri/processor usage; `--legacy-server` for Python API
- Runtime (outside repo): `/workspace/runtime/3dtile-bin-ktx2/` (document only)

### Tools
- `tools/experiments/rebuild_top_py/` — Python rebuild **baseline only**
- `tools/ktx2_postprocess/` — Node alternate encoder path
- `tools/texture_ktx2/` — KTX2 wrapper → experiments desktop_server_py module

### Docs / product
- `docs/product/` — architecture plan, USER_GUIDE, PHASE_REPORTS, historical Qt/OSGB notes

## Suggested exclude / caution
- `apps/desktop/node_modules/`, `apps/desktop/src-tauri/target/`, `__pycache__`, `.geoforge/`
- Large local outputs under `/workspace/data/geoforge_outputs/` (not in repo)
- Do not force-push; do not include secrets

## Verification already done (P7 / Phase 3)
- Artifact preview-url + Cesium load + KHR_texture_basisu evidence (historical)
- Phase 3: Tauri happy path without Python HTTP for convert/scan when processor present
