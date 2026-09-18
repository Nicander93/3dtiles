# Layer B — CesiumGS `3d-tiles-validator` (dev / acceptance)

Phase 12 of `04-v1-production-readiness-plan.md` defines two validation layers:

| Layer | Where | Role |
|-------|--------|------|
| **A** | Built into `processor` validate stage (`crates/processor/src/validator.rs`) | Fast recursive pre-commit checks; fail with stable codes; writes `validation_internal.json` |
| **B** | Dev / CI / acceptance only | Official CesiumGS schema + content validator |

**Node is not required in the desktop release runtime.** Layer B is an acceptance tool.

## Install / run

Requires Node.js 18+ with network access to npm (or a cached `3d-tiles-validator`).

```bash
# From repo root — auto-picks phase8/phase10 rebuild output, else hlod fixture
scripts/acceptance/run_layer_b_validator.sh

# Explicit tileset + report path
scripts/acceptance/run_layer_b_validator.sh \
  /path/to/output/tileset.json \
  acceptance-results/phase12-layer-b/validator-report.json
```

Equivalent manual invocation:

```bash
npx --yes 3d-tiles-validator \
  --tilesetFile <output>/tileset.json \
  --reportFile <acceptance>/validator-report.json
```

Optional local install (keeps `node_modules` out of git):

```bash
mkdir -p .cache/layer-b && cd .cache/layer-b
npm install 3d-tiles-validator@0.6.1
npx 3d-tiles-validator --tilesetFile ... --reportFile ...
```

## Gate

Production acceptance (later phases):

- validator **error severity count = 0** (exit code 0)
- warnings must be recorded / classified; not silently ignored without bound

Layer A must already pass before treating Layer B as meaningful for commit safety.

## What Layer A already covers in-process

- `asset` / `root` present
- `geometricError` finite and ≥ 0
- `refine` ∈ {REPLACE, ADD} when set
- `boundingVolume` box/region/sphere shape + finite numbers
- `transform` length 16 + finite
- content URI resolve; reject path escape outside output root
- recurse external tilesets; detect cycles
- content file exists; B3DM / GLB basic header checks (`magic`, `byteLength` / `version` / chunk bounds)
- clear codes (`CONTENT_MISSING`, `CONTENT_INVALID`, `CYCLE_DETECTED`, …) — no silent pass
