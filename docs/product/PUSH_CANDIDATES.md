# Push candidates (do NOT push/commit unless asked)

Date: 2026-09-09 Asia/Shanghai. Branch context: `feat/v0-scaffold` (local).
Purpose: list major **new / product** paths ready to consider for a future commit+push.
**No git push / no commit performed by this note.**

## Major new paths

### Apps / product shell
- `apps/web/` — React + Vite UI (dist, `public/cesium-preview.html`, pages, API client)
- `apps/desktop_server/` — FastAPI GeoForge API (tasks, artifacts, preview-url, `texture_ktx2.py`, runner)
- `apps/geoforge_shell/` — Qt product shell (browser fallback; OSGB embed)
- `apps/osgb_viewer/` — native OSGB preview helper

### Scripts / runtime note
- `scripts/run_geoforge.sh` — one-shot API+UI (:8787)
- `scripts/run_geoforge_shell.sh`
- `scripts/run_osgb_viewer.sh`
- Runtime (outside repo, document only): `/workspace/runtime/3dtile-bin-ktx2/`
  (`run.sh` wrapper → convert + basisu post-process; not a git path)

### Tools (supporting)
- `tools/ktx2_postprocess/` — Node alternate encoder path
- `tools/texture_ktx2/` — related helpers if present
- `tools/rebuild_top/rebuild_top.py` (+ `docs/REBUILD_TOP.md` edits)

### Docs / product
- `docs/product/` — V1_STATUS, V1_DELIVERY_PLAN, ACCEPTANCE, USER_GUIDE, QT_SHELL,
  OSGB_NATIVE_PREVIEW, mockups, screenshots (`p7_cesium_ktx2.png`, geoforge_shell_*.png),
  this `PUSH_CANDIDATES.md`

## Suggested exclude / caution
- `apps/web/node_modules/`, `apps/*/build/`, `__pycache__`, `.geoforge/`
- Large local outputs under `/workspace/data/geoforge_outputs/` (not in repo)
- Do not force-push; do not include secrets

## Verification already done (P7)
- Artifact art-c704b957a939 preview-url + Cesium load + KHR_texture_basisu evidence
