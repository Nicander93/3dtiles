# Existing feature validation — 2026-10-04

Feature development is paused. This record describes observed results on the source checkout and converter v0.2.4; it is not a claim that every dataset or installed release is correct.

## Validation matrix

| Existing capability | Evidence | Boundary |
| --- | --- | --- |
| FBX import | Actual processor converts ufbx's Blender cube binary FBX using metadata | This audit does not cover every exporter, animation or embedded material |
| OBJ import and resources | Strict missing-texture failure, warn-mode output, PNG loaded from a separate Unicode/space-containing texture root | One asymmetric textured triangle; detailed unit/axis/anchor coverage is in the coordinate audit |
| OSGB conversion | A copied complete block from the workspace's HK 8×8 dataset converts to 156 B3DM files, with 156 images | All LOD files are inspected; this is one block, not a full-city throughput or survey-accuracy acceptance |
| Native KTX2 | The same block produces 156 KTX2 images; each embedded image signature is checked | Native ETC1S path; external postprocessor/UASTC is not available in this workspace |
| Existing-tiles texture availability | Capabilities advertise native OSGB KTX2 separately from `processTileset.supported=false`; ProcessTiles uses that operation-specific gate | No claim that the missing external texture component works |
| Top rebuild | Actual process-tileset run on a complete synthetic 4×4 source, one merge level, 36 geometry files | The source combines the grid skeleton with valid existing HLOD leaf content; placeholders alone do not count as geometry validation |
| Limited rebuild levels | Rust tests cover 0, 1 and 2 merge levels, all 16 source attachments, geometric errors and exact cumulative world transforms in an ECEF-sized translated frame | Full pyramids retain their existing representation; partial pyramids use a content-free grouping root |
| Merge | CLI tests check portable copies, nested external tilesets, transforms, output conflicts and cancellation; browser tests submit actual processor tasks | Existing metadata-preserving merge, not mesh union |
| Clipping and flattening | CLI geometry/attributes/LOD/RTC/transform/failure tests, real Cesium drawing/drag/undo/export browser workflows | Existing regional and up-axis limits remain; see coordinate audit |
| Task lifecycle and resource serving | 19 Tauri library tests cover persistence, concurrent updates, logs, resource confinement and actual Windows child-process shutdown | Native WebView menus/dialogs and installer upgrades were not manually exercised |
| Input/output protection | Each conversion audit hashes the input tree; failures must emit no result, preserve an existing output, and leave no owned temporary directory | Covers tested failures, not every disk-full, power-loss or filesystem scenario |

## Defect found and fixed

The CLI and UI allow a limited number of top rebuild levels. The writer previously required exactly one top proxy, so a valid 4×4 source with one merge level failed after producing four proxies. Earlier tests mainly exercised full pyramids and missed this parameter combination.

The writer now groups remaining top proxies beneath a content-free root. It does not add another merge level, change source placement or discard original subtrees. The regression compares all 16 source world transforms before/after for 0/1/2 levels and checks output content. The actual processor one-level pipeline also succeeds.

In those Rust tests, zero means `max_levels=Some(0)` in the core, with no merges. The desktop/Processor convention remains unchanged: `levels=0` means the full pyramid, and positive numbers limit the number of merges.

## Reproduction and artifacts

Build the processor and TopRebuild with `rtk proxy cargo build --workspace --offline`. Run:

```text
rtk proxy python scripts/audit-existing-features.py --fbx <independent-FBX-file> --osgb <complete-OSGB-root>
```

The script copies inputs into a fresh directory under `target/feature-audit-*`. It retains task JSON, JSONL events, stderr, generated assets and `report.json`. Failure cases check their expected reason, not only a nonzero exit. Supplying neither real-data argument marks those checks `not-run`; skipped checks are not passes.

Set `GEOFORGE_AUDIT_OUTPUT` to that report directory and run `npm run test:e2e` from `apps/desktop`. The three data-bearing browser tests load original-texture OSGB, KTX2 OSGB and rebuilt tiles in actual Cesium. They reject JavaScript/loading errors and inspect central canvas pixels for visible geometry. Screenshots and render evidence are attached to Playwright results. The core merge/clip/flatten tests run without optional data. Optional-data tests explicitly skip when no report directory is supplied.

The current native audit passes 13 cases (6 positive pipelines and 7 expected failures). Complete Rust workspace tests, the 19 Tauri tests, frontend pure-function tests and frontend build pass. The final complete browser regression passes all 16 cases (53.6 seconds), including the three data-bearing rendering checks. Earlier failed browser runs were diagnosed rather than counted as successful runs: tests now wait for camera inertia to settle before clicking the handle, and use rendered pixels rather than unreliable legacy content statistics.

See [coordinate audit](COORDINATE_AUDIT.md) for the independently calculated 16-case coordinate matrix and remaining CRS/vertical-datum limits. Historical acceptance records are historical evidence; they do not supersede the boundaries above.

## Remaining acceptance work

- Native installed-app startup, file dialogs, upgrade and long-lived queue interaction.
- Representative FBX exporters/materials and a broader range of projected OSGB datasets with control points.
- Full-city conversion/rebuild memory, cancellation and recovery under load.
- External ETC1S/UASTC postprocessing with the real texture component installed.
- Z-up clipping is explicitly unsupported and must not be counted as passed.

These are validation gaps. No additional product features are introduced by this audit.
