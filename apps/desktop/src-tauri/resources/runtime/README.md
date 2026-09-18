# Packaged Windows runtime (`resources/runtime`)

Populated by `apps/desktop/scripts/package-windows.ps1` → `prepare-runtime.ps1` on a **Windows** build machine / CI (`windows-latest`).

Expected layout after staging (do not commit binaries):

```text
resources/runtime/
  converter/_3dtile.exe + OSG/GDAL/PROJ + MSVC CRT DLLs
  bin/processor.exe + top_rebuild.exe + MSVC CRT DLLs
  texture/geoforge-texture.exe + basisu.exe + MSVC CRT DLLs (unless -SkipTextureBundle)
  manifest.json
```

Linux Phase 14 staging uses `resources/bin/` via `scripts/release/stage_sidecars.sh`.
Both trees are listed in `tauri.conf.json` `bundle.resources`.

See `docs/product/PHASE_REPORTS/phase-18.md` and `docs/product/05-v1-hardening-release-runbook.md`.
