# geoforge_shell

Minimal GeoForge 3D Qt6 main shell. See `docs/product/QT_SHELL.md`.

```bash
micromamba activate osgb_viewer
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$CONDA_PREFIX"
cmake --build build -j
DISPLAY=:2 ./build/geoforge_shell
```
