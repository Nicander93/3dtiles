# v0 Scope / 首版范围

## One-liner / 一句话

Fork of [fanvanzh/3dtiles](https://github.com/fanvanzh/3dtiles) aiming at an open-source CesiumLab-like **oblique photogrammetry** toolchain: **OSGB → 3D Tiles** (reuse upstream KTX2 / Draco / simplify) + **post-process top-level rebuild** + **desktop UI** (native OSG preview + Cesium 3D Tiles preview).

基于 fanvanzh/3dtiles 的开源倾斜摄影处理小工具：OSGB→3D Tiles（复用上游压缩能力）+ 后处理顶层重建 + 桌面 UI（原生 OSG 看源、Cesium 看结果）。

## In scope / 范围内

- CLI convert: existing `osgb` / `shape` / `fbx` paths and `--enable-simplify` / `--enable-draco` / `--enable-texture-compress`
- CLI `rebuild-top` post-process (algorithm in [REBUILD_TOP.md](./REBUILD_TOP.md); stub in this PR)
- Desktop UI: job panel, native OSG viewer for OSGB, CesiumJS for 3D Tiles ([UI_PLAN.md](./UI_PLAN.md))
- Smart3D-style input: `Data/` + `metadata.xml`
- Docs + sample acceptance notes

## Out of scope / 不做（v0）

Terrain / imagery / point cloud tiling, distributed processing, multi-tileset stitching, advanced seam healing, full texture atlas, commercial GUI polish.

## Architecture / 架构

```text
Desktop UI ──► _3dtile CLI (convert)
           └─► _3dtile rebuild-top (post-process)
OSGB ──(native OSG viewer)──► preview
3D Tiles ──(Cesium WebView)──► preview
```

Heavy work stays in CLI; UI only orchestrates processes and hosts viewers.

## Milestones / 里程碑

| ID | Goal |
|----|------|
| M0 | Fork builds; convert + compress flags verified on a small sample |
| M0.5 | Minimal UI: jobs + Cesium preview + OSG open source dir |
| M1 | `rebuild-top` v0 (2×2 merge + simplify + texture downsample) |
| M2 | Parameter polish, before/after compare, docs/release |

## Attribution

Upstream: fanvanzh/3dtiles (Apache-2.0). This fork keeps that license and credit.
