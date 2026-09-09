# osgb_viewer — GeoForge native OSGB preview (prototype)

Minimal **Qt6 Widgets + OpenSceneGraph** window that opens a standard OSGB
dataset directory (`Data/` + `metadata.xml`) and shows progressive/paged
loading via `osg::PagedLOD`.

Product direction: **native OSGB preview** (not GLB/Web). See
`docs/product/OSGB_NATIVE_PREVIEW.md`.

## Sample data

Public sample used on this machine:

```text
/workspace/data/OSGBny/OSGBny
├── Data/Tile_*/Tile_*.osgb
└── metadata.xml
```

## Dependencies (user-space, no root/apt)

Preferred: micromamba + conda-forge.

```bash
bash apps/osgb_viewer/scripts/install_deps.sh
```

Validated pins (2026-09-09, this box):

| Package | Version |
| --- | --- |
| micromamba | 2.9.0 |
| openscenegraph | 3.6.5 (`h9389e23_21`) |
| qt6-main | 6.11.2 |
| cmake | 4.4.3 |
| gxx (conda-forge) | 15.3.0 |
| env prefix | `/workspace/conda/envs/osgb_viewer` |

`.osgb` is loaded by the OSG **IVE** plugin (`osgdb_ive.so`).

## Build (exact commands used here)

```bash
export MAMBA_ROOT_PREFIX=/workspace/conda
eval "$(/home/box/bin/micromamba shell hook -s bash)"
micromamba activate osgb_viewer

cd /workspace/repos/3dtiles/apps/osgb_viewer
cmake -S . -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$CONDA_PREFIX"
cmake --build build -j
```

Binaries:

- `build/osgb_viewer` — Qt GUI
- `build/osgb_headless_load` — headless load + bounding sphere / node stats

## Run

```bash
export LD_LIBRARY_PATH="$CONDA_PREFIX/lib:${LD_LIBRARY_PATH:-}"
export OSG_LIBRARY_PATH="$CONDA_PREFIX/lib/osgPlugins-3.6.5"
export DISPLAY=:2   # box desktop

# Headless smoke (no window)
./build/osgb_headless_load /workspace/data/OSGBny/OSGBny

# GUI
./build/osgb_viewer /workspace/data/OSGBny/OSGBny

# GUI CI smoke (auto-quit ~2.5s)
./build/osgb_viewer --smoke /workspace/data/OSGBny/OSGBny
```

### UI

- **Open Directory…** — pick dataset root (`Data/` + optional `metadata.xml`)
- **Reset View** (`R`) — trackball home (slightly zoomed in)
- Status line: path, root tile count, ~`.osgb` estimate, SRS/origin, errors
- Mouse: trackball (LMB orbit, RMB/wheel zoom)

## Layout

```text
apps/osgb_viewer/
├── CMakeLists.txt
├── README.md
├── include/{MainWindow,OsgWidget,DatasetLoader}.h
├── src/{main,MainWindow,OsgWidget,DatasetLoader,headless_load}.cpp
└── scripts/install_deps.sh
```

Embedding approach: `QOpenGLWidget` + `osgViewer::Viewer::setUpViewerAsEmbeddedInWindow`
(no `osgQOpenGL` — not shipped in conda-forge OSG 3.6.5).
