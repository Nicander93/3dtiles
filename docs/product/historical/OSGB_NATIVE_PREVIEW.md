> **Moved under historical/ in Phase 4.** Product Qt / OSGB viewer trees deleted from `apps/`.
> **CANCELLED FOR V1 — HISTORICAL ONLY.**  
> OSGB native preview / `osgb_viewer` notes. Capability cancelled for V1; Cesium 3D Tiles preview remains.  
> Qt main window, Qt WebEngine, OSGB native preview, `apps/geoforge_shell`, and `apps/osgb_viewer` are **out of V1 scope**. Code is **not deleted** in Phase 0.  
> Current architecture authority: [`03-v1-architecture-rebuild-plan.md`](./03-v1-architecture-rebuild-plan.md).

---

# OSGB native preview — prototype notes

Date: 2026-09-09 (Asia/Shanghai)  
App: `apps/osgb_viewer`  
Aligns with product principle: open OSGB directory and progressively load
original data (not convert-to-GLB / WebGL first). See `01-product-definition.md`
§2.3–2.4 and §4 OSGB 预览.

## What worked on this machine

| Criterion | Result |
| --- | --- |
| Install Qt6 + OSG in user space | **Yes** — micromamba/conda-forge env `osgb_viewer` |
| Compile C++20 CMake project | **Yes** — `osgb_viewer` + `osgb_headless_load` |
| Headless load OSGBny | **Yes** — 6 roots, 78 `.osgb`, bs radius ≈ 969.8, 6× PagedLOD |
| GUI window + embedded OSG | **Yes** — Qt6 `QOpenGLWidget` + `GraphicsWindowEmbedded` |
| Geometry visible | **Yes** — textured tiles visible (lighting forced OFF for albedo) |
| PagedLOD honored | **Yes** — roots load as PagedLOD; children remain deferred until frame/range |

Headless output (reference):

```text
dataset: /workspace/data/OSGBny/OSGBny
metadata: SRS=ENU:35.90924,... origin=-958,-993,69
root tiles: 6
estimated .osgb files: 78
bounding sphere center: (955.684, 961.638, 34.0691)
bounding sphere radius: 969.766
nodes: 19 geodes: 6 drawables: 6 PagedLOD: 6
OK
```

Screenshot artifact: `apps/osgb_viewer/build/osgb_preview_shot.png`.

## Version pins

| Component | Pin |
| --- | --- |
| micromamba | 2.9.0 |
| MAMBA_ROOT_PREFIX | `/workspace/conda` |
| env | `osgb_viewer` |
| openscenegraph | **3.6.5** (`conda-forge`, build `h9389e23_21`) |
| qt6-main | **6.11.2** |
| cmake | 4.4.3 |
| g++ | 15.3.0 (conda-forge) |
| OSG plugins path | `$CONDA_PREFIX/lib/osgPlugins-3.6.5` |
| `.osgb` reader | `osgdb_ive.so` |

Install script: `apps/osgb_viewer/scripts/install_deps.sh`.

## Approaches tried / rejected

1. **Prebuilt OSG under `/workspace/runtime/3dtile-bin/lib` (3.7.0)** — shared libs
   link on glibc 2.41, but **no headers**, CentOS-era layout; not used for the
   app build. Kept only as `_3dtile` converter runtime.
2. **`/workspace/tools/3dtiles-root`** — old image root; not required once
   conda-forge OSG worked.
3. **`osgQOpenGL`** — **not present** in conda-forge `openscenegraph 3.6.5`.
   Used manual embed instead.
4. Early GUI crash — starting `QTimer`/`update()` before `initializeGL`
   raced `QOpenGLWidget` FBO setup (`QOpenGLContext::functions` nullptr).
   Fixed by deferring the frame timer until GL init and avoiding
   `makeCurrent`/`doneCurrent` inside `paintGL`.

## Gaps vs full Qt shell integration

Prototype only — not yet the product shell in `01-product-definition.md`.

| Full shell expectation | Prototype gap |
| --- | --- |
| Left nav + multi-page workspace | Single-purpose window |
| WebEngine / tiles preview host | Not integrated |
| Task manager / convert export from preview | Status + Open only |
| Unified branding / dock panels | Bare toolbar + status |
| Database paging HUD (LOD level, MB loaded) | No live paging meter |
| SRS ENU → world transform gizmo | Metadata text only |
| Multi-dataset / layer list | Loads all `Tile_*` roots into one group |
| osgQOpenGL / Qt3D interop | Manual `QOpenGLWidget` embed |
| Installer / runtime bundling | Conda env assumed |

## Follow-ups

1. Live status: DatabasePager active / completed / paged-out counts.
2. Tile list panel with per-root visibility toggles.
3. Optional ENU origin overlay from `metadata.xml`.
4. Integrate as a QWidget page inside the future GeoForge Qt shell.
5. Consider shipping `osgQOpenGL` build or vendor a thin embed helper.
6. Better initial framing for sparse multi-tile bounds (already slightly zoomed).

## Success verdict

**Best criterion met:** GUI window loads OSGBny and shows geometry.  
**Acceptable also met:** headless bounds/stats CLI; reproducible install script.
