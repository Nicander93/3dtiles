# OSGBny GRID_SPATIAL_MISMATCH 分析

Date: **2026-09-11**  
数据：`data/real/OSGBny/OSGBny` → `data/real/OSGBny_3dtiles`（`winner1/3dtiles:1.0` 转换，0.66 s）  
约束：本文件只分析。**不关闭校验，不改 TreeBuilder，不做 nearest-neighbor 分组。**

## 重测命令与结果

```powershell
.\target\debug\top_rebuild.exe -i data\real\OSGBny_3dtiles -o data\real\OSGBny_rebuild
```

```text
GRID_SPATIAL_MISMATCH: block Tile_+006_+005 grid=(6,5)
center=(325.003,284.942) expected≈(343.924,275.613) dist=21.095 tol=17.286
```

与 Phase 10 相同。Phase A 之后 center 仍是 ENU 米制，不是 ECEF XY 误读。

## 问题清单

### Tile_x_y 是否空间网格？

是。同一 `x=6` 的四块，center.x 都在 325 m 附近；`y` 增加时 center.y 增加。文件名里的 x/y 对应空间行列，不是随机编号。

### root child boundingVolume 在哪个坐标系？

子节点 **没有** `transform`。`boundingVolume.box` 在 root 的 local / ENU 米制。  
root 有完整 ECEF `transform`（translation ≈ `-2.36e6, 4.60e6, 3.72e6`）。  
world = `root.transform × local_center`。Grid 校验在 origin block 的 local frame 比较，因此看到的就是下表的 local center。

### transform 之后的 center？

子块 identity，local center = origin-local center。

| blockId | gridX | gridY | localCenter (m) | half-axes hx,hy (m) | child transform |
|---------|-------|-------|-----------------|---------------------|-----------------|
| Tile_+005_+033 | 5 | 33 | 295.602, 1658.499, 21.574 | 4.59, 8.69 | identity |
| Tile_+006_+005 | 6 | 5 | 325.003, 284.942, 33.902 | 25.20, 15.27 | identity |
| Tile_+006_+006 | 6 | 6 | 324.995, 325.002, 35.406 | 25.20, 25.20 | identity |
| Tile_+006_+007 | 6 | 7 | 325.013, 374.996, 33.895 | 25.21, 25.20 | identity |
| Tile_+006_+009 | 6 | 9 | 325.008, 474.998, 32.728 | 25.20, 25.19 | identity |
| Tile_+032_+031 | 32 | 31 | 1600.296, 1574.998, 36.639 | 0.50, 25.19 | identity |

nearest spatial neighbors（按 local XY）：

- `+006_+005` ↔ `+006_+006` ≈ 40 m（y）
- `+006_+006` ↔ `+006_+007` ≈ 50 m（y）
- `+006_+007` ↔ `+006_+009` ≈ 100 m（y，中间缺 `+006_+008`）
- `+005_+033`、`+032_+031` 相对 `+006_*` 是千米级远离

### 缺块还是拓扑不同？

两者都有一点：

1. **稀疏规则网格**：上传者写过「拿了部分数据」。`+006_+008` 缺失，且 `+005_+033` / `+032_+031` 来自同一套编号的远处。
2. **格子尺寸不均**：`+006_+006/007/009` 约 50 m × 50 m；`+006_+005` 的 hy 只有 15 m；`+005_+033` 和 `+032_+031` 是细长/很小的块（hx 4.6 m / 0.5 m）。边块或裁切块。
3. **校验算法放大了稀疏**：origin 取 `min(grid_x, grid_y)` → `Tile_+005_+033` (5,33)。再用全部 pair 的中位间距，从 (5,33) 外推到 (6,5)，期望值 (343.9, 275.6) 对不上真实 (325.0, 284.9)。

没有发现子块 rotation / non-uniform matrix。问题不是「编号与空间无关」，也不是 Phase A 的 bounds transform。

## 决策（尚未实现）

### 情况 1：只是 transform / bounds bug

否。Phase A 后数字不变。

### 情况 2：规则网格 + 缺块

部分成立。`+006_y` 大致 50 m 一格，缺 y=8。当前 Quadtree 已允许 1–4 个 child。若要支持这类数据，应 **扩展 spatial validation**（例如按邻接间距、允许空洞），而不是关掉错误。

边块尺寸不均会让「单一 cell size + 单一 origin 外推」仍然失败。即便做 sparse-regular 合法化，也要定义对非均匀格子的容忍度。

### 情况 3：Tile_x_y 与空间无可靠关系

否。不要为此改成 bounds-derived / adaptive quadtree。

## 不允许的修复（保持拒绝）

- 关闭 `GRID_SPATIAL_MISMATCH` 继续跑
- 校验失败后仍 `floor(x/2), floor(y/2)` 建树
- 按文件顺序强制分组
- 静默改成 nearest neighbor clustering

## 建议下一步

人工选择：

1. 保持现状：OSGBny 继续诚实失败；Phase C 换连续 4×4～8×8 真数据。
2. 设计 sparse-regular 校验（情况 2），设计评审后再改 `validate_grid_spatial`，仍用规则 Quadtree。

本分析不授权直接改 TreeBuilder。
