# Coordinate audit (2026-10-04)

This audit covers the processor and the installed geoforge-converter v0.2.4. It does not certify arbitrary survey data or vertical datums.

## Coordinate contracts

| Boundary | Contract |
| --- | --- |
| OSGB metadata `SRS=ENU:latitude,longitude` | Latitude first, longitude second, degrees; local east/north/up in metres |
| Model anchor | `longitudeDeg`, `latitudeDeg`, `ellipsoidHeightM`; WGS84 degrees and ellipsoidal metres |
| Model normalization | Explicit metres/centimetres/millimetres/feet and right-handed Y-up/Z-up |
| Model projected mode | Explicit source CRS, east/north or north/east mapping, source-space origin offset; projection performed by converter |
| Model heading/pitch/roll | Degrees, Cesium convention: heading about -Z, pitch about -Y, roll about +X |
| Model pivots | Original normalized origin, geometry bounding-box centre, or bottom centre; tile padding excluded |
| GeoJSON clipping/flattening | Longitude/latitude in degrees; height in ellipsoidal metres |
| 3D Tiles `boundingVolume.region` | Longitude/latitude in radians; heights in metres |
| GLB/B3DM | Tile transform, RTC, declared up-axis and node transform compose in that order |

No automatic geoid correction is established. Orthometric survey heights must not be assumed interchangeable with ellipsoidal heights. A temporary preview anchor is display placement, not proof of geographic positioning.

## Findings fixed

1. Anchored OBJ output added its bounding-box centre to the root placement while retaining that centre in geometry. The asymmetric fixture displaced its original origin by about 38.5 metres. The processor now applies an explicit ENU-to-ECEF placement before validation and commit.
2. Anchor HPR fields used camelCase in the desktop configuration but did not deserialize under those names. The protocol now reads them and rejects unknown georeference fields instead of silently defaulting rotation to zero.
3. Converter anchor flags with separate negative values were rejected by its argument parser. `--lon=value`, `--lat=value`, and `--alt=value` support west/south and negative heights.
4. Bottom-centre pivots based on padded tile bounds introduced a 0.5 metre height error in the fixture. Pivots now use POSITION accessor bounds and node/RTC/tile transforms. The reader skips binary buffers/textures and limits each JSON table to 16 MiB; tile traversal respects cancellation.
5. OSGB CRS/origin overrides could be recorded as effective coordinates without changing the converter's metadata-based coordinate transformation. Unsupported changes now fail before conversion. Equal metadata overrides are a no-op, never an additional translation. Partial/non-finite origins and invalid ENU ranges are rejected.

WGS84 ECEF math is shared by anchoring and clipping.

## Repeatable validation

From `apps/desktop`, run `npm run test:coordinates` after preparing the pinned converter runtime. This builds the processor and runs actual OBJ conversions, reads exported vertices, and compares world positions against Cesium Core calculations. Set `GEOFORGE_3DTILE` / `GEOFORGE_AUDIT_PROCESSOR` for custom executable paths. Set `GEOFORGE_AUDIT_REPORT` to save JSON results.

The 16 cases cover:

- Four units times two up-axis conventions (8 local cases).
- Three anchors: China, longitude/latitude zero, and west/south with negative height.
- Nonzero heading/pitch/roll with all three pivots.
- EPSG:3857 with both axis mappings and nonzero origin offsets; reference positions use the independent analytical Mercator inverse and Cesium WGS84 ECEF.

All 16 passed locally. The acceptance threshold is 0.02 m; the maximum observed discrepancy for these small synthetic fixtures was approximately 0.000002504 m. This measures these fixtures only, not achievable accuracy on large source data.

Rust regressions cover WGS84 known points, ENU axes/ranges, OSGB override rejection, HPR deserialization and padding-independent bottom pivots. Existing merge, clipping and flattening tests cover transformed nodes, RTC, external roots and nonzero geographic frames. Browser E2E exercises actual processor outputs through the preview workflow.

## Remaining scope

- Real OSGB/EPSG/WKT datasets and FBX metadata need representative source data for independent ground-control checks. The legacy C++ coordinate test file is not part of the current Rust build and is not counted as validation.
- EPSG:3857 validation does not establish correctness for all projected CRSs, zone numbers, axis conventions or vertical transformations.
- Clipping/flattening currently accepts Y-up GLB/B3DM and rejects other declared up axes. The model converter's Z-up tiles can be previewed, but clipping them remains an explicit unsupported case. Do not silently rotate those tiles to bypass this check.
- Clipping remains a local-region operation with its existing latitude and extent limits, not a global planar approximation.
- Native desktop interaction and large-data performance were not manually verified in this audit.
