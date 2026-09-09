# Desktop UI plan (v0)

## Panels

1. **Project / job** — pick OSGB root (`Data/` + `metadata.xml`), output dir, convert flags, `rebuild-top` levels
2. **OSGB preview** — **native OpenSceneGraph viewer** (inspire by [xrui94/OSGB-3DTiles](https://github.com/xrui94/OSGB-3DTiles) viewer)
3. **3D Tiles preview** — CesiumJS in a WebView / embedded browser; load produced `tileset.json` (before/after rebuild switch)
4. **Logs** — stdout/stderr from CLI child processes; retry with same params

## Shell options

- Qt (+ WebEngine) keeps OSG and UI in one C++-friendly stack
- Or Tauri/Electron + separate OSG viewer process

## Rule

UI does **not** reimplement tiling; it only spawns `_3dtile` / `rebuild-top` and hosts viewers.
