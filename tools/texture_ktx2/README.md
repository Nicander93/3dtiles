# texture_ktx2 (experiments / regression)

**Release path (Phase 14):** Processor uses a **Rust** tileset walker that calls the
bundled `basisu` CLI. No Python is required for `texture.mode=ktx2-*`.

This directory keeps the Python wrapper for regression only:

```bash
# Release / default
GEOFORGE_TEXTURE_ENGINE=rust   # default
# processor process-tileset -i DIR -o OUT --texture ktx2-etc1s

# Experiments fallback only
GEOFORGE_TEXTURE_ENGINE=python python tools/texture_ktx2/run.py -i TILESET_DIR --mode ktx2-etc1s
```

Env: `GEOFORGE_BASISU` overrides basisu path (sidecar preferred).
