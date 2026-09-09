# GeoForge 3D 本地验证指南（v0.1）

面向从 GitHub 克隆后在本机跑通产品壳与转换流水线（keep / KTX2）。
Date: 2026-09-09 Asia/Shanghai.

## 1. 克隆

Clone Nicander93/3dtiles, checkout feat/v0-scaffold.
Repo includes apps/web/dist for the UI.

## 2. Deps

Use a virtualenv and pip requirements for the desktop API.
npm optional when dist exists.
Converter binary outside repo; set runtime env vars. Prefer ktx2+basisu wrapper.

## 3. Sample

Prepare an OSGB folder with metadata.xml (SRS / SRSOrigin).
Sample data is not shipped with the repo.

## 4. Start

Launch the desktop API via scripts/run_geoforge.
Open the UI on local port 8787 and hit /api/health.
Task DB and preview cache live under .geoforge (gitignored).

## 5. Smoke keep + ktx2

1. Convert page: input OSGB root, scan (CRS/origin).
2. keep texture, submit, wait succeeded, Cesium preview.
3. ktx2-etc1s (optional rebuild levels=1); needs basisu postprocess capability.
4. Or continue-process tiles on an existing keep artifact.

## 6. Qt shell optional

Needs osgb_viewer conda env and built geoforge_shell binary.
Start API first, then the shell launcher with DISPLAY and MAMBA_ROOT_PREFIX.
Browser-fallback shell (not full WebEngine). See QT_SHELL.md.

## 7. FAQ

| issue | fix |
|------|------|
| missing web dist | rebuild apps/web |
| converter missing | set runtime / 3dtile env |
| ktx2 grayed | check wrapper and basisu |
| Qt fails | conda env, DISPLAY, binary |

## Related

- docs/product/USER_GUIDE.md
- docs/product/ACCEPTANCE.md
- docs/REBUILD_TOP.md
- tools/texture_ktx2/README.md
