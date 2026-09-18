# Branch merge status — `feat/v1-prod-align`

Date: **2026-09-18 ~23:20 Asia/Shanghai (CST)**  
Integration branch: `feat/v1-prod-align`  
Merge: `feat/v0-scaffold` → base `fix/top-rebuild-ktx2-read` (`12f763a`)

## Divergence (pre-merge)

| Side | Unique commits | Role |
|------|----------------|------|
| `feat/v0-scaffold` | 6 (`c41609f`…`0ffa0c1`) | Phase 11–15 production-readiness (correctness, validator, staging, zero-Python, acceptance scripts) |
| `fix/top-rebuild-ktx2-read` | 74 | Hardening/release + KTX2 B3DM read + basisu PATH/CRT packaging + Phase 17 harness start |

Merge-base: `5fe12bd`.

## Strategy

Prefer **hardening as base**, merge scaffold **into** it (this branch). Keep:

- KTX2/`KHR_texture_basisu` read, opaque pass-through, sibling `../bin` CRT PATH (`glb.rs`, `texture.rs`, packaging scripts)
- Phase 11–15 modules from scaffold (`validator.rs`, `texture_ktx2.rs`, `phase11_correctness.rs`, acceptance `.py`, phase-11..15 reports, production-readiness plan)

## Conflict resolution summary (26 files)

| Area | Resolution |
|------|------------|
| Docs / `.gitignore` / thresholds / acceptance README | Union both tracks; keep `cesiumAb` thresholds + Phase 15 scripts docs |
| `top_rebuild` types/adapter/tileset_writer/selector/tree_builder/error/bin | Prefer scaffold (Phase 11) |
| `top_rebuild` `glb.rs` / `texture.rs` / `lib.rs` | Prefer HEAD KTX2 + re-export Phase 11 symbols |
| `top_rebuild` `b3dm.rs` | HEAD `LoadedContent`/RTC + scaffold GLB-length extract; keep `load_content_glb` |
| `processor` commit/validate/texture/convert/pipeline/main | Prefer scaffold (Phase 12–14) |
| `processor` `util.rs` | Prefer HEAD packaging (`texture_bin`, `packaged`, `hide_console_window`) |
| Desktop `commands.rs` / `process_manager.rs` | Prefer scaffold (Phase 14 sidecar) |

## Build / test evidence (post-resolve)

| Check | Result |
|-------|--------|
| `cargo build -p top_rebuild --bin top_rebuild` | PASS |
| `cargo build -p processor` | PASS |
| `cargo test -p top_rebuild --lib` | **45 passed** |
| `cargo test -p top_rebuild --test phase11_correctness` | **7 passed** |
| `cargo test -p top_rebuild --test hlod_4x4` | **3 passed** |
| `cargo test -p top_rebuild --test transform_invariants` | **FAIL compile** — tests still use pre–Phase-11 `Representation.world_transform` / `BoundingVolume::transformed_center` APIs |

## Remaining follow-ups (not force-pushed)

1. Update or gate `tests/transform_invariants.rs` for Phase-11 `RepresentationPart` / world BV APIs.
2. Re-verify desktop Tauri build after Phase-14 commands merge vs Job Object hardening (commands taken from scaffold).
3. Re-run processor `phase12_layer_a` / `phase13_commit` integration tests once convenient.
4. PlanD Layer B validator still historically red (`numErrors=16`) — not fixed by this merge.
5. Do **not** claim limited V1 production-ready until Phase 17 far-view gate + Layer B policy are honest green.

## Non-claims

- No force-push.
- Merge is local integration; remote push only to **new** branch name when clean.


## Phase 17 A/B (same session)

| Item | Path / result |
|------|----------------|
| Baseline | `/workspace/data/acceptance/outputs/phase16_hk_4x4/baseline` |
| Candidate | `/workspace/data/acceptance/outputs/phase16_hk_4x4/rebuild` (L0→L3 full pyramid) |
| Results | `acceptance-results/phase17_ab_full/` (gitignored) |
| Far-view §15.4 | **PASS 2/3** (requests −81.5%, bytes −57.7%, time −6.1%) |
| Report | `docs/product/PHASE_REPORTS/phase-17.md` |
