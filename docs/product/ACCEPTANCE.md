> **Architecture rebuild acceptance (Phases 0–10, 2026-09-10 Asia/Shanghai):**  
> Algorithm / Processor / Tauri path: see [`PHASE_REPORTS/SUMMARY.md`](./PHASE_REPORTS/SUMMARY.md).  
> Top rebuild scale: **4×4 + 16×16 synthetic demonstrated**; sparse OSGBny **rejected** (`GRID_SPATIAL_MISMATCH`); **城区 / 百平方公里 NOT demonstrated** — do not treat historical “Top rebuild | pass” row as large-area production acceptance.  
> Status line: *V1 algorithm complete for continuous regular grids (≤16×16 synthetic); large-scale validation pending.*

> **CANCELLED FOR V1 — HISTORICAL ONLY.**  
> Prior acceptance table included OSGB native / Qt shell as partial. Those rows are cancelled for new V1; do not use this file as final V1 acceptance.  
> Qt main window, Qt WebEngine, OSGB native preview, `apps/geoforge_shell`, and `apps/osgb_viewer` are **out of V1 scope**. Code is **not deleted** in Phase 0.  
> Current architecture authority: [`03-v1-architecture-rebuild-plan.md`](./03-v1-architecture-rebuild-plan.md).

---

# GeoForge 3D V1 ACCEPTANCE

Date: 2026-09-09 Asia/Shanghai.
Maps design section 2 must-deliver rows to pass/partial/fail.
**P0–P7 done** (incl. KTX2 postprocess, Cesium ENU framing, rebuild levels 1|2 UI,
geoforge_shell browser fallback, `run_geoforge.sh` / `(deleted) run_geoforge_shell.sh`).
P4 closed via basisu. Later push list: `PUSH_CANDIDATES.md` (no push this batch).

| Item | Result | Evidence |
| --- | --- | --- |
| OSGB scan + errors | pass | API scan OSGBny shows ENU and origin |
| OSGB convert | pass | succeeded convert tasks; geoforge_outputs keep path |
| Top rebuild | pass | Merge_L* + REPLACE; UI/API levels 1\|2; GE flooring (P7c) |
| Artifacts preview | pass | SQLite artifacts and Cesium preview |
| Product UI | pass | React shell; rebuild levels + KTX2 options when caps allow |
| OSGB native / Qt shell | partial | osgb_viewer + geoforge_shell browser-fallback (no linked WebEngine); needs DISPLAY |
| 3D Tiles preview | pass | Cesium local; ENU modelMatrix + fit/framing (P7d); `p7_cesium_ktx2_v2.png` |
| Task lifecycle | pass | stages cancel logs history rerun |
| process-tileset | pass | tiles process UI keep/rebuild/KTX2 |
| Texture keep KTX2 | pass | P4 closed. keep green; KTX2 ETC1S via basisu post-process (`KHR_texture_basisu`). P7a E2E task-3917f396ea60; process task-5f1d943144ed |
| CRS origin elevation | pass | P5 UI API MISSING_CRS gate; vertical datum note |
| One-shot docs | pass | `run_geoforge.sh` / `(deleted) run_geoforge_shell.sh`; ACCEPTANCE USER_GUIDE |

## Evidence

- convert keep tasks and process keep tasks in local SQLite
- **P7a E2E:** `task-3917f396ea60` (convert-osgb, rebuildTop levels=1, ktx2-etc1s) →
  `/workspace/data/geoforge_outputs/osgbny_p7_e2e_rebuild` — tileset.json, Merge_L*,
  78× `KHR_texture_basisu` / image/ktx2; artifact `art-c704b957a939`; tileset HTTP 200;
  Cesium load + ENU framing (`p7_cesium_ktx2.png` / `p7_cesium_ktx2_v2.png`)
- process-tileset ktx2-only: `task-5f1d943144ed` → `osgbny_p7_process_ktx2` from `osgbny_p4_keep`
- Also: `osgbny_p4_ktx2_post`, `one_tile_ktx2_task` (task-e22f347ea059), wrapper smoke
- UI: OsgbConvert / ProcessTiles / Settings expose rebuild levels 1|2
- Qt shell screenshots: `geoforge_shell_shot.png`, `geoforge_shell_osgb.png`
- cancel samples recorded

## Non-claims

- No Windows installer
- No full / linked Qt WebEngine shell (`geoforge_shell` = browser fallback + OSGB embed)
- Optional Qt osgb_viewer + geoforge_shell (DISPLAY often :2)
- Native in-binary `--enable-texture-compress` still absent on Aug-2023 `_3dtile`;
  KTX2 is delivered via wrapper + basisu post-process (not a rebuilt encoder)

## Smoke

- Scan OSGBny ENU/origin
- Convert keep works; KTX2 + levels 1|2 selectable when caps OK
- Missing SRS geographic errors
- Web build and health check; Cesium preview framing

Task ids: task-3917f396ea60 (P7a E2E rebuild+ktx2), task-5f1d943144ed (P7a process ktx2-only), task-c838478f53ae, task-3e6ac31bb396, task-5e1a6cec5b80, task-356e36c69933, task-3bcd586217b2, task-0975f5589cba, task-e22f347ea059
