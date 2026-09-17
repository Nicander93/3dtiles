# GeoForge 3D 本地验证指南（v0.1 / Phase 4）

面向从 GitHub 克隆后在本机跑通 Tauri 桌面壳与 Processor 流水线（keep / KTX2）。
Date: 2026-09-17 Asia/Shanghai.

## 1. 克隆

Clone Nicander93/3dtiles.
Repo may include `apps/desktop/dist` for the UI.

## 2. Deps

- Node 18+ for `apps/desktop`
- Rust toolchain for `processor` + `geoforge-desktop` (Tauri)
- Optional Python venv for rebuild baseline / KTX2 (`tools/experiments/rebuild_top_py`, `tools/texture_ktx2`)
- Windows release builds stage the pinned converter under `resources/runtime/converter`; for local development, set `GEOFORGE_3DTILE` or `GEOFORGE_RUNTIME_ROOT` when using a converter outside the repository.

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

### Windows bundle

```powershell
cd apps/desktop
powershell -NoProfile -File scripts/package-windows.ps1
```

`package-windows.ps1` 会清理并 staging `processor.exe`、`top_rebuild.exe`、`_3dtile.exe`、`geoforge-texture.exe`、`basisu.exe` 及其 OSG/GDAL/PROJ 和 MSVC runtime DLL。Cesium 运行时已经随应用打包，可离线加载本地 3D Tiles。

NSIS 产物位于 `src-tauri/target/release/bundle/nsis/`。2026-09-12 的 `0.1.0` 包已通过隔离静默安装和首次启动冒烟。

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
| converter missing | 重新运行 `prepare-runtime.ps1`；开发覆盖时设置 `GEOFORGE_RUNTIME_ROOT` / `GEOFORGE_3DTILE` |
| rebuild script missing | `tools/experiments/rebuild_top_py/rebuild_top.py` or `GEOFORGE_REBUILD_TOP` |
| ktx2 grayed | check wrapper and basisu (`tools/texture_ktx2`) |

## Related

- docs/product/USER_GUIDE.md
- docs/product/ACCEPTANCE.md
- docs/REBUILD_TOP.md
- tools/experiments/rebuild_top_py/README.md
- tools/texture_ktx2/README.md
