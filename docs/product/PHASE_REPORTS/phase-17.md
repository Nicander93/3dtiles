# Phase 17 — Cesium automated A/B (PlanD HK 4×4)

Date: **2026-09-18 ~23:46 Asia/Shanghai (CST)**  
Branch: `feat/v1-prod-align` (merge of `fix/top-rebuild-ktx2-read` + `feat/v0-scaffold`)  
Harness: `scripts/acceptance/cesium_ab_harness.mjs`  
Authority: production-readiness plan §15 / §15.4 far-view gate

## Dataset

| Arm | Path | Notes |
|-----|------|-------|
| Baseline (convert-only) | `/workspace/data/acceptance/outputs/phase16_hk_4x4/baseline` | PlanD Kowloon contiguous **4×4** (~748 MB), already on disk |
| Candidate (TopRebuild) | `/workspace/data/acceptance/outputs/phase16_hk_4x4/rebuild` | Fresh `top_rebuild` full pyramid **L0 16 → L1 6 → L2 2 → L3 1** (~751 MB, ~15 min debug build) |

Source OSGB: `/workspace/data/acceptance/hk_pland/extracted/osgb_4x4` (PlanD photo-realistic).

## Command

```bash
cargo build -p top_rebuild --bin top_rebuild
./target/debug/top_rebuild \
  -i /workspace/data/acceptance/outputs/phase16_hk_4x4/baseline \
  -o /workspace/data/acceptance/outputs/phase16_hk_4x4/rebuild
# (omit --levels so pyramid merges to a single root)

cd apps/desktop && npm run prepare:cesium && cd ../..
npm --prefix scripts/acceptance install
node scripts/acceptance/cesium_ab_harness.mjs \
  --baseline /workspace/data/acceptance/outputs/phase16_hk_4x4/baseline \
  --candidate /workspace/data/acceptance/outputs/phase16_hk_4x4/rebuild \
  --out acceptance-results/phase17_ab_full \
  --sse 16 --settle-ms 15000
```

Artifacts (gitignored): `acceptance-results/phase17_ab_full/{result.json,result.md,cesium-baseline.json,cesium-rebuild.json,screenshots/*.png}`.

## Metrics (SSE=16)

| Arm | Camera | tilesLoaded | tOverview (ms) | content reqs | bytes |
|-----|--------|-------------|----------------|--------------|-------|
| baseline | far-overview | **true** | 1809 | 54 | ~18.4 MB |
| baseline | mid | **true** | 1731 | 84 | ~23.5 MB |
| baseline | near | false* | 2068 | 126 | ~39.3 MB |
| rebuild | far-overview | **true** | 1699 | **10** | ~7.8 MB |
| rebuild | mid | **true** | 1759 | 92 | ~25.7 MB |
| rebuild | near | false* | 2068 | 139 | ~44.1 MB |

\* Near arm did not reach full `tilesLoaded` within settle on this 748 MB tileset (same pattern as Phase 17 smoke). Far/mid overview gates are the §15.4 contract.

## Far-view gate (§15.4)

Thresholds (need **≥2 of 3**): content requests −50%, bytes −40%, timeToOverview −25%.

| Metric | Baseline | Candidate | Improve % | Hit? |
|--------|----------|-----------|-----------|------|
| content requests | 54 | 10 | **81.5%** | **YES** (≥50) |
| bytes | 18351799 | 7753969 | **57.7%** | **YES** (≥40) |
| timeToOverview | 1809 ms | 1699 ms | **6.1%** | no (<25) |

**Result: PASS (2/3 hits).** Harness exit code 0.

Honest notes:

- Time-to-overview barely improved; most far-view benefit is **request and byte** reduction from proxy HLOD, not wall-clock first overview on this machine (SwiftShader / no discrete GPU).
- Mid/near arms show **higher** content traffic on rebuild than baseline — expected when the camera is close enough to refine into leaves (and proxies still participate). Do **not** claim mid/near bandwidth wins from this run.
- Rebuild emitted many `GAP_WARN` / `BUDGET_NOT_REACHED` warnings under LockBorder (maxGap hundreds of meters). Spatial/seam quality is **not** certified by this A/B; only Cesium load + far-view network/time gates.
- PlanD Layer B validator was historically **FAIL** (`numErrors=16`); this Phase 17 run does **not** clear Layer B.
- SSE sweep 8/16/32 not repeated in this session (SSE=16 only).

## Verdict

| Item | Status |
|------|--------|
| Same-dataset baseline + TopRebuild candidate | **DONE** |
| Automated far/mid/near harness | **DONE** |
| Far-view §15.4 gate (≥2/3) | **PASS** |
| Near full tilesLoaded | **NOT MET** (settle) |
| SSE 8/16/32 sweep | **PARTIAL** (16 only) |
| Production-ready claim | **NOT claimed** — see gap audit + Layer B |

## Related

- `phase-17-start.md` — harness wiring smoke (single-arm)
- `phase-11-18-gap-audit.md` — track split context
- `branch-merge-status.md` — `feat/v1-prod-align` merge notes
