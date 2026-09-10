# Baseline Freeze — Phase 0

Date recorded: **2026-09-10 09:15:28 CST (Asia/Shanghai)**  
UTC equivalent: 2026-09-10T01:15:28Z

## Git identity

| Field | Value |
| --- | --- |
| Branch | `feat/v0-scaffold` (**not** `master`) |
| HEAD SHA | `68892faf7a90c0e8d6130576e3970254ccd81f9d` |
| HEAD commit date (git) | 2026-09-09 09:52:24 +0000 |
| HEAD subject | feat(geoforge): GeoForge 3D v0.1 product shell, pipeline, and KTX2 postprocess |
| Local `master` SHA | `acbcf603f33fdfe3c34b704a8b019c4fd32a8376` |
| Working tree | clean at freeze time |

## Note: master vs feat/v0-scaffold

Phase 0 freeze is on **`feat/v0-scaffold`** at `68892faf…`, which carries the GeoForge v0.1 product shell / pipeline / KTX2 postprocess work. Local `master` (`acbcf603…`) is behind this branch. Architecture rebuild work proceeds from this frozen HEAD; do not treat historical `V1_STATUS.md` “V1 complete” wording as the new V1 acceptance conclusion.

## Authority document

Current architecture authority: [`03-v1-architecture-rebuild-plan.md`](./03-v1-architecture-rebuild-plan.md)

## Smoke summary (Phase 0)

| Check | Result | Evidence |
| --- | --- | --- |
| React UI `apps/desktop` `npm run build` | **PASS** | `tsc --noEmit && vite build`; dist built (~923ms) |
| Converter smoke (`3dtile-bin-ktx2`) | **PASS** | see Phase 0 report |
| Python `rebuild_top.py` baseline | **PASS** | see Phase 0 report |

No git push performed in Phase 0.


## Phase 1 status (2026-09-10 Asia/Shanghai)

- Tauri 2 scaffold added under `apps/desktop/src-tauri/` (React stays in `apps/desktop`; not renamed to `apps/desktop`).
- OSGB Preview removed from product UI (Router / Sidebar / Workspace). Page file deleted.
- Directory commands: `select_input_directory`, `select_output_directory`, `select_tileset_file`.
- Python `desktop_server` :8787 still the task/API backend (not replaced).
- See `PHASE_REPORTS/phase-1.md`. No git push in Phase 1.

## Phase 2 status (2026-09-10)

See `docs/product/PHASE_REPORTS/phase-2.md`. Rust SQLite + artifact resource server landed; convert execution still bridges to Python until Phase 3.
