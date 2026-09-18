## Phase 15 完成报告

Date: **2026-09-10 ~16:05 CST** (Asia/Shanghai)  
Scope: Real public data test framework — plan `04-v1-production-readiness-plan.md` §8.  
**Dedicated git commit:** `feat(acceptance): Phase 15 public data test framework`

### 目标

| Item | Result |
|------|--------|
| `scripts/acceptance/` pipeline | **PASS** |
| `environment.json` capture (§8.1) | **PASS** — `capture_environment.py` |
| HK PlanD download helpers (§9) | **PASS** — GridIdx + contiguous select + pdmap ZIP |
| inventory / convert / rebuild / validate orchestration | **PASS** — `run_acceptance.py` |
| README: data outside git | **PASS** — `$GEOFORGE_ACCEPTANCE_ROOT` or `/workspace/data/acceptance/` |
| Datasets not committed | **PASS** — `.geoforge-testdata/` + acceptance-results gitignored |
| `tests/acceptance/thresholds.json` | **PASS** |

### Layout

```text
scripts/acceptance/
  acceptance_paths.py
  capture_environment.py
  download_hk_pland.py
  prepare_hk_pland.py
  inventory_source.py
  run_acceptance.py
  compare_tilesets.py
  download_public_alternates.py
  cesium_benchmark.mjs          # Phase 17 stub
  run_layer_b_validator.sh      # Phase 12
  README.md
tests/acceptance/
  README.md
  thresholds.json
```

### Evidence (framework smoke)

- `python3 -m py_compile scripts/acceptance/*.py` → OK
- `capture_environment.py` → disk free **~73.4 GiB**, git commit recorded
- `download_hk_pland.py --probe-only --width 4 --height 4 --region kowloon` →
  contiguous 16 grids selected from official `GridIdx.geojson`; estimate ~1.9 GiB;
  disk guard passed (free ≥ 2.5× need)
- Data root used: `/workspace/data/acceptance/hk_pland/` (outside git)

### Host note (2026-09)

- Official JS still references `ppgis.pland.gov.hk`; DNS often **NXDOMAIN**.
- Working ZIP host matches remarks PDF: `pdmap.pland.gov.hk/PLANDWEB/public/3d_photo_realistic_models/...`
- Scripts try pdmap first, then ppgis fallback.

### Copyright

PlanD data for **internal non-commercial verification only**. ZIPs/OSGB/B3DM stay out of git; reports keep stats / hashes / grid names / screenshots only.

### Next

Phase 16 runs this framework against a real contiguous HK region (OSGB→tiles→rebuild→gates).
