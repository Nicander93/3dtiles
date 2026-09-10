# GeoForge 3D 本地验证指南（v0.1 / Phase 4）

面向从 GitHub 克隆后在本机跑通 Tauri 桌面壳与 Processor 流水线（keep / KTX2）。
Date: 2026-09-10 Asia/Shanghai.

## 1. 克隆

Clone Nicander93/3dtiles.
Repo may include `apps/desktop/dist` for the UI.

## 2. Deps

- Node 18+ for `apps/desktop`
- Rust toolchain for `processor` + `geoforge-desktop` (Tauri)
- Optional Python venv **only** for experiments (`GEOFORGE_REBUILD_ENGINE=python` / `GEOFORGE_TEXTURE_ENGINE=python`). Release path: Rust `top_rebuild` + Rust KTX2 walker + `basisu` sidecar.
- Converter binary outside repo; set `GEOFORGE_3DTILE` / runtime env. Prefer ktx2+basisu wrapper.

## 3. Sample

Prepare an OSGB folder with metadata.xml (SRS / SRSOrigin).
Sample data is not shipped with the repo.

## 4. Start (preferred)

```bash
cargo build -p processor
cd apps/desktop && npm install && npm run tauri:dev
```

Or print / launch helper:

```bash
bash scripts/run_geoforge.sh
```

Legacy Python API (reference):

```bash
bash scripts/run_geoforge.sh --legacy-server   # :8787
```

Task DB and preview cache live under `.geoforge` (gitignored).

## 5. Smoke keep + ktx2

1. Convert page: input OSGB root, scan (CRS/origin).
2. keep texture, submit, wait succeeded, Cesium preview.
3. ktx2-etc1s (optional rebuild levels=1); needs basisu postprocess capability.
4. Or continue-process tiles on an existing keep artifact.

## 6. Qt shell

**Removed in Phase 4.** See `docs/product/historical/QT_SHELL.md`.

## 7. FAQ

| issue | fix |
|------|------|
| missing web dist | `cd apps/desktop && npm run build` |
| converter missing | set `GEOFORGE_RUNTIME` / `GEOFORGE_3DTILE` |
| top_rebuild missing | `cargo build -p top_rebuild --bin top_rebuild` or `GEOFORGE_TOP_REBUILD` / sidecar |
| ktx2 grayed | check `basisu` sidecar (`GEOFORGE_BASISU` / `scripts/release/stage_sidecars.sh`) |

## Related

- docs/product/USER_GUIDE.md
- docs/product/ACCEPTANCE.md
- docs/REBUILD_TOP.md
- tools/experiments/rebuild_top_py/README.md
- tools/texture_ktx2/README.md
