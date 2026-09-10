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
6. Rewrite tileset: parents as children of a new root; each parent uses `refine: REPLACE` and hangs original 2×2 kids; parent `geometricError` via max(2×child, 0.25×AABB diagonal, child+1) (P7c).
7. Validate in Cesium: fewer far-view requests; visual sanity.

## Constraints

- Stream **by group** — never load a whole city.
- Unify tile local / ECEF frames before merge.
- v0 skips: cross-tileset stitch, seam healing, full atlas, distributed jobs.

## Status

**Status: v0 + P7c geometricError heuristic — baseline only, not release runtime**

Phase 4: moved to `tools/experiments/rebuild_top_py/`. Formal V1 TopRebuild is C++ (Phase 5+).

- Implementation: `tools/experiments/rebuild_top_py/rebuild_top.py`
- CLI entry: `_3dtile rebuild-top ...` (via `crates/rebuild_top_cli`) or
  `python tools/experiments/rebuild_top_py/rebuild_top.py -i <tileset> -o <out>`
- Behavior: 2×2 group of `Tile_+r_+c` root external tilesets; merge root `.b3dm` meshes;
  write `Data/Merge_L*_*/` parent with `refine: REPLACE`; copy remainder of tileset tree.

## P7c quality tweaks (2026-09-09)

Small, safe changes only (no algorithm rewrite):

- **geometricError heuristic:** `parent_geometric_error()` = max(
  `2 * max(child GE)`,
  `0.25 * parent AABB diagonal`,
  `max(child)+1`,
  and strictly `> max(child)`).
  Root GE similarly floored above children / diagonal.
- **`--levels 2`:** already supported; cheap when no further 2×2 groups merge (pass-through). Documented in CLI help.
- Still writes parents with **`refine: REPLACE`** and original kids hung under merge nodes.

### Limits (honest)

- Only merges root `Tile_+r_+c` external tilesets; sparse grids often yield many size-1 pass-through groups.
- AABB-only box union (diagonal half-axes); no ECEF reproject / seam heal / atlas.
- Mesh simplify via trimesh quadric; texture bilinear downsample; no Draco/KTX2 on parents here.
- Multi-level only re-groups previous merge indices with cell=2 — needs a dense enough grid to reduce further.

### Smoke

```bash
python tools/experiments/rebuild_top_py/rebuild_top.py \
  -i /workspace/data/geoforge_outputs/osgbny_p7_e2e \
  -o /workspace/data/geoforge_outputs/osgbny_p7c_rebuild --levels 1 -v
# → 6 tiles → 5 groups; 1 Merge_L1_3_3 REPLACE parent

python tools/experiments/rebuild_top_py/rebuild_top.py \
  -i /workspace/data/geoforge_outputs/osgbny_p7_e2e \
  -o /workspace/data/geoforge_outputs/osgbny_p7c_rebuild_l2 --levels 2 -v
# → level2: 5→5 (no extra merge); REPLACE still intact
```
