# GeoForge acceptance scripts (Phase 15)

Production-grade **public data** test framework. Real datasets stay **outside git**.

## Data root

| Priority | Path |
|----------|------|
| 1 | `$GEOFORGE_ACCEPTANCE_ROOT` |
| 2 | `/workspace/data/acceptance/` |
| 3 | `~/geoforge-acceptance/` |

Do **not** place multi-GB OSGB / B3DM / ZIP under the git worktree. Tiny indexes (e.g. `GridIdx.geojson`) may live under the data root `meta/` folder. Optional `.geoforge-testdata/` is gitignored for tiny caches only.

Results: `acceptance-results/<run-id>/` (gitignored). Phase reports under `docs/product/PHASE_REPORTS/` are committed.

## Layout

```text
scripts/acceptance/
  acceptance_paths.py          # resolve data root
  capture_environment.py       # environment.json (§8.1)
  download_hk_pland.py         # PlanD GridIdx + contiguous select + download (§9)
  prepare_hk_pland.py          # non-destructive staging (§10)
  inventory_source.py          # source inventory (§11.1)
  run_acceptance.py            # convert → validate → rebuild → gates
  compare_tilesets.py          # spatial compare vs official Cesium (§11.3)
  download_public_alternates.py# §21 CesiumGS samples / Utrecht probe
  run_layer_b_validator.sh     # CesiumGS 3d-tiles-validator
  cesium_benchmark.mjs         # stub for Phase 17 A/B
  README.md

tests/acceptance/
  README.md
  thresholds.json
```

## Hong Kong PlanD

- Download page: https://www.pland.gov.hk/pland_en/info_serv/3D_models/download.htm
- ZIP pattern (remarks PDF): `https://pdmap.pland.gov.hk/PLANDWEB/public/3d_photo_realistic_models/<format>/<GRID>_<FORMAT>.zip`
- Note (2026-09): `ppgis.pland.gov.hk` often NXDOMAIN; scripts prefer `pdmap`.
- Copyright: internal non-commercial verification only; do not redistribute ZIPs; reports may keep stats/hashes/grid names/screenshots.

```bash
# Probe contiguous 4×4 (sizes + disk guard) without downloading
python3 scripts/acceptance/download_hk_pland.py --width 4 --height 4 --region auto --probe-only

# Download + stage + full pipeline
python3 scripts/acceptance/run_acceptance.py --width 4 --height 4 --region kowloon
```

## Disk guards

- Estimated download vs free space: require free ≥ 2.5 × need (plan §9.2).
- If need > 30 GiB and disk insufficient → **STOP** (plan §0.1 blocker).

## Hard rules

- No synthetic substitution for final acceptance.
- No silent ignore of transform / texture / missing URI / budget / gap errors.
- On `GRID_SPATIAL_MISMATCH` or structural failure: fix or document honest **NOT READY**.
