# Top-level rebuild (`rebuild-top`) — design

Post-process **after** normal OSGB→3D Tiles conversion. Does not rewrite the converter’s OSGB visitor.

## CLI (planned)

```text
_3dtile rebuild-top -i <tileset_dir> -o <out_dir> --levels N [--simplify 0.5] [--texture-scale 0.5]
```

- Input: directory with `tileset.json` (+ tile content)
- Output: **new** directory (never overwrite input; enables A/B)
- `--levels N`: N=1 ≈ ~1/4 root tiles via quadtree 2×2

## Pipeline

1. Parse `tileset.json`; collect first-level children / root tiles (often `Tile_r_c`).
2. Build a grid index from names or boundingVolume centers.
3. Group neighbors (2×2); undersized edge groups merge degraded or pass through.
4. For each group: load b3dm/glb → unify coordinates → meshoptimizer simplify → texture downsample (atlas deferred to later).
5. Write parent tiles; optional Draco/KTX2 on parents.
6. Rewrite tileset: parents as children of a new root; each parent uses `refine: REPLACE` and hangs original 2×2 kids; parent `geometricError ≈ max(child) * 2` (tune later).
7. Validate in Cesium: fewer far-view requests; visual sanity.

## Constraints

- Stream **by group** — never load a whole city.
- Unify tile local / ECEF frames before merge.
- v0 skips: cross-tileset stitch, seam healing, full atlas, distributed jobs.

## Status

Scaffold stub only: see `crates/rebuild_top_cli`. Implementation lands in M1.
