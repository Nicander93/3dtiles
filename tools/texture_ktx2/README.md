# texture_ktx2

Post-process 3D Tiles textures to KTX2 via vcpkg basisu (KHR_texture_basisu).

Implementation lives in `tools/experiments/desktop_server_py/app/texture_ktx2.py`
(legacy path was `apps/desktop_server`).

```bash
# preferred wrapper
python tools/texture_ktx2/run.py -i TILESET_DIR --mode ktx2-etc1s
python tools/texture_ktx2/run.py -i TILESET_DIR -o OUT_DIR --mode ktx2-etc1s

# or run module file directly
python tools/experiments/desktop_server_py/app/texture_ktx2.py -i TILESET_DIR --mode ktx2-etc1s
```

Env: `GEOFORGE_BASISU` overrides basisu path.
