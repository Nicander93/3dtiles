# Real Data Validation

Date: **2026-09-12** Asia/Shanghai

```text
TopRebuild algorithm implemented and validated on regular synthetic grids
and real contiguous HK LandsD OSGB at 5×4, 8×8, and 16×16.
Urban / production-scale validation is still pending.
```

Do not read this file as: 支持大范围倾斜摄影顶层重建 / 百平方公里 / 生产可用.

## Dataset

| 项 | 值 |
|----|-----|
| source | 香港地政总署 3D Visualisation Map（Tile-based OSGB） |
| sheet | `11-NW-10B` 的 20 个 150 m 分块 `11-NW-10B-1.zip` … `-20.zip` |
| 不用 | 用户从 `3d.map.gov.hk` 下的 `11-NW-10B.zip`（839 MB，内容是单体建筑 FBX，不是 OSGB mesh） |
| 输入 | `data/real/hk_11-NW-10B/osgb`（gitignore） |
| metadata | `EPSG:2326`，`SRSOrigin=800000,800000,0` |
| 网格 | `Tile_+5578_+5481` … `Tile_+5582_+5484`，连续 **5×4 / 20 blocks** |
| OSGB 体积 | 888.67 MB |
| 放大 | `data/real/hk_8x8`：`Tile_+5575_+5477` … `+5582_+5484`，**8×8 / 64**，OSGB 2359.18 MB |
| 再放大 | `data/real/hk_16x16`：`Tile_+5567_+5469` … `+5582_+5484`，**16×16 / 256**，OSGB 10183.89 MB |

## Pipeline

```text
OSGB
 → docker winner1/3dtiles:1.0 _3dtile -f osgb
 → sheet_3dtiles（baseline，不重建）
 → top_rebuild（不带 --levels，建到单根）
 → sheet_rebuild
 → Cesium 1.125
```

| 阶段 | 命令 / 结果 |
|------|-------------|
| convert | 20.03 s，`tileset.json` 根 transform 为 ECEF，20 个 child |
| baseline 体积 | 1172.48 MB / 4585 files |
| rebuild | `target/release/top_rebuild.exe` exit 0，**39.13 s** |
| rebuild 体积 | 1180.04 MB |
| 树 | L0 20 / L1 9 / L2 4 / L3 1 |
| probe | proxies=14，leaf_external=20，replace=34，max_depth=3 |

没有 `GRID_SPATIAL_MISMATCH`。OSGBny 的失败与这份连续图幅无关。

## Metrics（`sheet_rebuild/rebuild_metrics.json`）

| 项 | 值 |
|----|-----|
| 输入三角形（proxy 简化前合计） | 见各 proxy `triangles_before` |
| 输出 proxy 三角形 | 68385 |
| proxy texture bytes | 4075287 |
| proxy glb/b3dm bytes | 7880444 |
| maxGap | 471.30 m |
| P95Gap | 439.59 m |
| pair_count | 6 |
| lock_border | true |
| KTX2 | 未开 |
| 峰值 RSS | 本机未单独采样 |
| 临时磁盘 | 未单独采样 |

`GAP_WARN` / `BUDGET_NOT_REACHED` 都是警告。LockBorder 下部分 L2/L3 压不到预算；gap 阈值仍是 1 m，对 150 m 实景块偏紧。

## Cesium

预览：`apps/desktop/public/cesium-preview.html?tileset=…/sheet_rebuild/tileset.json&debug=1`

| 观察 | 结果 |
|------|------|
| 定位 | boundingSphere lon=114.18573 lat=22.34087（黄大仙 / 横头磡一带），未落到地心 |
| 翻转 / 旋转 | 未见 |
| 整块消失 | 未见，20 块都在 |
| 近景纹理 | 楼体、道路、植被可辨，无明显错贴 |
| 原 Block LOD | 近景走到 L14–L22，外部 `tileset.json` 原样保留 |
| refine | 父树 34 个节点均为 `REPLACE` |

## LOD / Proxy 调度

父树 geometricError 实际是：

```text
L3 1030
L2 1020
L1 1010
L0 1000   ← 转换器写在每个 Block 根上
```

`geometric_error_proxy` 只保证 parent > child（`max_child * 1.01` 或 `+1`）。子节点已经是 1000 m 时，代理层几乎不会被 Cesium 选中。中远景统计到的是原始 Block 的 L14/L16，不是 `Proxy_L*.b3dm`。

这不是坐标错误，也不是树建错。要让远景真吃代理，需要标定 GE（Phase F），不能靠再堆更大 synthetic。

## Baseline vs rebuild

| | baseline `sheet_3dtiles` | rebuild `sheet_rebuild` |
|--|--------------------------|-------------------------|
| 根 | 20 个 Block 平铺 | L3 单根 + 代理金字塔 |
| 原 Block | 转换器输出 | 整目录复制，L14–L22 仍在 |
| 体积 | 1172 MB | 1180 MB（多 14 个 proxy） |

## 8×8

| 阶段 | 结果 |
|------|------|
| convert | 50.25 s，根 transform 为 ECEF，64 个 child |
| rebuild | exit 0，**72.35 s**（改原点前：64→25→9→4→1） |
| probe | proxies=39，leaf_external=64，replace=103，max_depth=4 |
| Cesium | lon=114.18361 lat=22.33832，矩形完整 |

## 16×16

| 阶段 | 结果 |
|------|------|
| convert | 205.74 s，256 个 child |
| rebuild | exit 0，**373.5 s**，256→64→16→4→1 |
| probe | proxies=85，leaf_external=256，replace=341，max_depth=4 |
| Cesium | lon=114.17764 lat=22.33273，r≈1953 m，矩形完整 |

第一次用绝对格子号重建时是 `NO_FURTHER_REDUCTION (4)`。L0 改成本地原点后通过。没有 `GRID_SPATIAL_MISMATCH`。GE 阶梯仍是 1040→1000。

## 不宣称

- 城区 / 百平方公里（16×16 真数据约 2.4 km 见方）
- 远景请求数、下载量、FPS 对比（未做网络/帧耗时采样）
- TopRebuild 生产可用
