# Converter resources (Phase 14)

Stage the OSGB → 3D Tiles converter here before packaging:

```text
resources/bin/
  run.sh          # or _3dtile.exe on Windows
  _3dtile         # native binary
  lib/            # shared libs / OSG plugins required by _3dtile
```

Use `scripts/release/stage_sidecars.sh` to copy from a developer runtime
(e.g. `$GEOFORGE_RUNTIME/3dtile-bin` or `.runtime/3dtile-bin`).

Release resolution is **relative to the app / resources**, never `/workspace/...`.
