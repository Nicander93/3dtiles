# GeoForge 3D V1 Status

Date: 2026-09-09 Asia/Shanghai. v0.1.0 deliverable. No git push.
Plan: V1_DELIVERY_PLAN.md (P0–P7 done; **P4 closed** via basisu post-process; **P7 Cesium KTX2 load confirmed**).

## How to run

bash scripts/run_geoforge.sh -> UI http://127.0.0.1:8787/

Default convert bin: `/workspace/runtime/3dtile-bin-ktx2/run.sh` when present
(falls back to `/workspace/runtime/3dtile-bin/run.sh`).

## P4 Texture keep / KTX2 — UNBLOCKED (path 2: post-process)

- keep works on convert and process-tileset
- **KTX2 ETC1S works** without a newer `_3dtile` binary:
  1. keep-convert with winner1/3dtiles:1.0
  2. post-process B3DM/GLB with **basisu** (`apps/desktop_server/app/texture_ktx2.py`)
     → `KHR_texture_basisu` / `image/ktx2` evidence
- Wrapper: `/workspace/runtime/3dtile-bin-ktx2/run.sh`
  - `--help` lists `--enable-texture-compress`
  - flag triggers convert then basisu post-process
- GeoForge caps: `ktx2Etc1s=true`, `postprocessBasisu=true`, `processTilesetTexture=true`
  (native CLI flag still false on the Aug-2023 binary)

### Smoke (2026-09-09 ~17:03 Asia/Shanghai / 09:03 UTC)

```
/workspace/runtime/3dtile-bin-ktx2/run.sh -f osgb \
  -i /workspace/data/osgb_one_tile \
  -o /workspace/data/geoforge_outputs/osgbny_ktx2_wrapper_smoke \
  --enable-texture-compress -v
```

Result: 20 b3dm / 30 textures converted; `output_has_ktx2_evidence` → found.

Additional E2E (same post-process path):
- `osgbny_p4_keep` → `osgbny_p4_ktx2_post`: 78 b3dm / 110 textures, 0 errors
- process-tileset task-e22f347ea059 → `one_tile_ktx2_task` status=succeeded, evidence found


Also available: Node `tools/ktx2_postprocess/b3dm_ktx2.mjs` (gltf-transform etc1s + toktx)
as an alternate encoder path.

### Paths tried / ruled out

1. **CI Linux artifact:** only non-expired artifact is `windows-x86_64` (fanvanzh/3dtiles).
   `linux.yml` builds with `package: false` — no Linux tarball to fetch.
2. **Post-process KTX2 (chosen):** basisu already in vcpkg_installed; wired into runner + wrapper.
3. **Source rebuild (background, not required for unblock):** vcpkg left running at
   `/workspace/tools/vcpkg` — GDAL finished (~43 min); **Building osg@3.6.5** in progress
   (`vcpkg-install4.log`, concurrency=1). Do not rely on it for P4; post-process is the
   success path on this RAM-constrained box.

## P5 Geo / CRS

- Scan shows effective CRS, origin, unit hint (OSGBny ENU + SRSOrigin)
- Override CRS / origin X Y Z into task options.geo
- ENU maps to 3dtile -c x/y; override Z may map to -c offset
- geographicExport without SRS/override: UI block + API 400 MISSING_CRS
- Settings: vertical datum independent; V1 does not auto-guess

## P6 Packaging

- docs/product/ACCEPTANCE.md
- docs/product/USER_GUIDE.md
- run_geoforge.sh prints URLs and UI dist status; prefers 3dtile-bin-ktx2
- Not claimed: Windows package, **in-process Qt WebEngine embed** (see P7b)
- Optional: Qt osgb_viewer / geoforge_shell native OSGB preview (DISPLAY often :2)


## P7 Hardening (P7a)

- Docs: P4 marked [x]; focus -> P7; ACCEPTANCE/USER_GUIDE synced
- E2E convert-osgb **task-3917f396ea60** (P7a-e2e-rebuild-ktx2):
  - input `/workspace/data/OSGBny/OSGBny`
  - options: rebuildTop enabled levels=1, texture.mode=ktx2-etc1s
  - output `/workspace/data/geoforge_outputs/osgbny_p7_e2e_rebuild`
  - evidence: tileset.json, Merge_L1_3_3, 78x KHR_texture_basisu / image/ktx2 (79 b3dm)
  - artifact art-c704b957a939; GET preview-url OK; tileset HTTP 200;
    Cesium `cesium-preview.html?tileset=` accepts URL
- process-tileset ktx2-only **task-5f1d943144ed**: `osgbny_p4_keep` -> `osgbny_p7_process_ktx2`

## P7b Qt WebEngine shell (2026-09-09 ~17:10+ Asia/Shanghai)

- Installed `qt6-webengine=6.11.2` into `/workspace/conda/envs/osgb_viewer` (micromamba/conda-forge) — **package OK**, CMake finds `Qt6::WebEngineWidgets`.
- **Link failed** (conda `libudev.so.1` vs conda sysroot glibc symbols `GLIBC_2.25`–`2.34`). Exact errors in `docs/product/QT_SHELL.md`.
- Delivered shell with `-DGEOFORGE_ENABLE_WEBENGINE=OFF`: browser fallback + **yellow status banner** + status bar; OSGB预览 still native `OsgbWidget`.
- Rebuilt `apps/geoforge_shell/build/geoforge_shell`; screenshots on DISPLAY=:2:
  `docs/product/geoforge_shell_shot.png`, `geoforge_shell_osgb.png`.
- Did **not** touch `/workspace/runtime/3dtile-bin-ktx2`.

## P7c rebuild-top quality (small)

- Better `geometricError`: max(2×child, 0.25×AABB diagonal, child+1), parent > children; root GE floored similarly.
- `--levels 2` smoke cheap on sparse grids; still **REPLACE** parents.
- Smoke: `osgbny_p7_e2e` → `osgbny_p7c_rebuild` (1 Merge_L1 REPLACE) and `_l2`.
- Limits documented in `docs/REBUILD_TOP.md` (no full algorithm rewrite).

## P7 Cesium KTX2 load (2026-09-09 ~17:22 Asia/Shanghai)

Confirmed Cesium can load **P7 E2E KTX2+rebuild** artifact:

- API `:8787` up; artifact **art-c704b957a939** (`osgbny_p7_e2e_rebuild` / P7a-e2e-rebuild-ktx2)
- preview-url: `/artifacts/art-c704b957a939/tileset.json` (HTTP 200)
- Preview URL used:
  `http://127.0.0.1:8787/cesium-preview.html?tileset=%2Fartifacts%2Fart-c704b957a939%2Ftileset.json`
- Content URI fetch: nested leaf B3DM has **`KHR_texture_basisu`** + `image/ktx2`
  (disk: 78/79 b3dm); Merge_L1 parent may lack textures (expected)
- Headless Chrome on box (swiftshader WebGL): **WebGL OK**
  (`WebGL 1.0 (OpenGL ES 2.0 Chromium)`); Cesium probe
  `Cesium3DTileset.fromUrl` → **TILESET_OK** `tilesLoaded=true`,
  boundingSphere radius≈960 m (ECEF center)
- Screenshot: `docs/product/p7_cesium_ktx2.png` (Cesium UI + tileset path label;
  mesh may be sparse/ENU-zoom dependent in headless; load path proven)
- computerUse MCP unavailable this session; used headless Chrome instead

## P7d Cesium camera / ENU polish (2026-09-09 ~17:31 Asia/Shanghai)

- `apps/web/public/cesium-preview.html` (+ `examples/preview/index.html` / `rebuild.html`):
  - After load: `flyTo` / `viewBoundingSphere` with `HeadingPitchRange`, wait for `tilesLoaded`, then re-frame on loaded **content** spheres (fallback root)
  - Local / near-ECEF-origin tilesets get ENU `modelMatrix` (`?enu=lat,lon[,h]` or OSGBny default 35.90924,117.13183)
  - Load error banner (`#errorBanner`); optional `?debug=1` extent ellipsoid + HUD stats
  - `postMessage({type:'geoforge-fit'})` and Home button re-fit
- UI: Processing / OsgbPreview / TilesPreview toolbars closer to mockups; KTX2 options enabled when `caps.postprocessBasisu.available` (`apps/web/src/lib/textureCaps.ts`)
- Smoke: art-c704b957a939 → `tilesLoaded`, camera over lon≈117.14 lat≈35.92 (not starfield); screenshot `docs/product/p7_cesium_ktx2_v2.png` (headless Chrome). Mesh still sparse for this sample’s tile footprint.


## Local polish - rebuildTop.levels 1|2 (2026-09-09 ~17:35 Asia/Shanghai)

- UI OsgbConvert ProcessTiles Settings: levels 1 or 2 default 1 wired to options.rebuildTop.levels
- API runner clamp 1|2; passes --levels to rebuild_top.py
- Processing page shows rebuild levels in task summary when present
- API smoke process-tileset L2 on one_tile_3dtiles succeeded quickly; art-4d2b91740770
- build and health ok; no git push

## Bottom line

Deliverable v0.1 usable on Linux box: OSGB convert (keep) + **KTX2 ETC1S via basisu
post-process** + rebuild + Cesium preview + tasks + CRS UX + docs.
**P4 closed.** **P7 complete** for V1 hardening scope: E2E rebuild+ktx2
(task-3917f396ea60 / art-c704b957a939), process-tileset ktx2, Cesium load+WebGL
screenshot, Qt shell browser-fallback, rebuild-top GE tweak.
Native `--enable-texture-compress` inside `_3dtile` still absent (Aug-2023 binary);
wrapper + GeoForge texture stage provide honest KTX2 with KHR_texture_basisu evidence.
