# GeoForge acceptance scripts

Production-readiness acceptance helpers (Phases 15–17).

## Restored / present on this branch

| Path | Role |
|------|------|
| `cesium_ab_harness.mjs` | **Phase 17** Cesium baseline vs rebuild A/B (Chrome + puppeteer-core) |
| `cesium_ab_probe.html` | Headless probe page (fixed SSE / camera range multipliers) |
| `package.json` + `node_modules/` | Local harness deps (`puppeteer-core`); do not commit `node_modules` |

## Still on `feat/v0-scaffold` only (not merged here)

`download_hk_pland.py`, `prepare_hk_pland.py`, `run_acceptance.py`, `capture_environment.py`, `compare_tilesets.py`, `run_layer_b_validator.sh`, etc. See `docs/product/PHASE_REPORTS/phase-11-18-gap-audit.md`.

## Phase 17 quick start

```bash
cd apps/desktop && npm run prepare:cesium
cd ../..
# optional: npm --prefix scripts/acceptance install
node scripts/acceptance/cesium_ab_harness.mjs \
  --baseline /path/to/convert_only \
  --candidate /path/to/rebuild \
  --out acceptance-results/phase17_ab_run
```

Data stays outside git. Results under `acceptance-results/` (gitignored).
