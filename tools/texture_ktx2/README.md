# texture_ktx2 (experiments / regression)

**Release path (Phase 14):** Processor uses a **Rust** tileset walker that calls the
bundled `basisu` CLI. No Python is required for `texture.mode=ktx2-*`.

Windows packaging may still ship a PyInstaller `geoforge-texture.exe` beside BasisU
under `resources/runtime/texture/` for older installers; prefer the Rust walker +
`basisu` sidecar for new builds.

This directory keeps the Python wrapper for regression only:

```bash
# Release / default
GEOFORGE_TEXTURE_ENGINE=rust   # default
# processor process-tileset -i DIR -o OUT --texture ktx2-etc1s

# Experiments fallback only
GEOFORGE_TEXTURE_ENGINE=python python tools/texture_ktx2/run.py -i TILESET_DIR --mode ktx2-etc1s --basisu /path/to/basisu
python tools/texture_ktx2/run.py -i TILESET_DIR -o OUT --mode ktx2-uastc --report report.json
```

```powershell
# Optional Windows onedir package (legacy)
cd tools/texture_ktx2
pyinstaller --onedir --name geoforge-texture run.py
```

Env: `GEOFORGE_BASISU` overrides basisu path (sidecar preferred);
`GEOFORGE_TEXTURE` may point at geoforge-texture.exe on older packages.
