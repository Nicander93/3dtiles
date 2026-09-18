# Phase 17 start — Cesium A/B harness

Date: **2026-09-18 ~23:15 Asia/Shanghai (CST)**  
Branch: `fix/top-rebuild-ktx2-read`  
Follows gap audit: `phase-11-18-gap-audit.md` (Phase 17 was the highest-priority **MISSING** item actionable in-session).

## What was added

| Path | Role |
|------|------|
| `scripts/acceptance/cesium_ab_harness.mjs` | Chrome (puppeteer-core) runner: far/mid/near cameras, Network CDP bytes/requests, screenshots, §15.4 far-view gate |
| `scripts/acceptance/cesium_ab_probe.html` | Offline Cesium 1.125 probe (`apps/desktop/public/vendor/cesium`) |
| `scripts/acceptance/README.md` | How to run; notes scaffold scripts not yet merged |
| `scripts/acceptance/package.json` + lock | `puppeteer-core` only (node_modules gitignored) |
| `tests/acceptance/thresholds.json` | Restored thresholds + `cesiumAb` §15.4 numbers |

## Smoke evidence (single-arm wiring)

Command:

```bash
cd apps/desktop && npm run prepare:cesium
npm --prefix scripts/acceptance install
node scripts/acceptance/cesium_ab_harness.mjs \
  --baseline /workspace/data/acceptance/outputs/phase16_hk_4x4/baseline \
  --out acceptance-results/phase17_ab_smoke \
  --sse 16 --settle-ms 12000
```

Result (2026-09-18 CST):

| Camera | tilesLoaded | tOverview | net content reqs | bytes |
|--------|-------------|-----------|------------------|-------|
| far-overview | true | ~1824 ms | 54 | ~18.4 MB |
| mid | true | ~1753 ms | 84 | ~23.5 MB |
| near | false* | ~2101 ms | 126 | ~39.3 MB |

\* Near arm hit settle timeout before full `tilesLoaded` on this 748 MB PlanD baseline — expected under short settle; increase `--settle-ms` for near gate runs.

Artifacts (gitignored): `acceptance-results/phase17_ab_smoke/{result.json,result.md,screenshots/*.png,cesium-baseline.json}`.

Far-view §15.4 gate: **not applicable** (no `--candidate`). Honest: this session proves the harness, not TopRebuild benefit.

## Still required for Phase 17 DONE

1. Same-dataset `--baseline` convert-only **and** `--candidate` TopRebuild (PlanD 4×4 rebuild unfinished; LandsD sheet pair not on this Linux disk).
2. Far-view gate: ≥2 of {requests −50%, bytes −40%, timeToOverview −25%}.
3. Screenshot set for both arms + SSE sweep 8/16/32 (§17) recorded into acceptance report.
4. Optional: wire into `run_acceptance.py` once Phase 15 scripts are restored onto this branch.

## Non-claims

- Does **not** close Phase 17.
- Does **not** claim production-ready or far-view benefit.
- Does **not** merge `feat/v0-scaffold` Phase 11–15 code.
