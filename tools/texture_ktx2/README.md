# texture_ktx2

Post-process 3D Tiles textures to KTX2 via vcpkg basisu (KHR_texture_basisu).

```bash
python -m apps.desktop_server.app.texture_ktx2 -i TILESET_DIR --mode ktx2-etc1s
python -m apps.desktop_server.app.texture_ktx2 -i TILESET_DIR -o OUT_DIR --mode ktx2-etc1s
```

Env: GEOFORGE_BASISU overrides basisu path.
