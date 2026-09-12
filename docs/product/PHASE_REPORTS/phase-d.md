## Phase D

Date: **2026-09-12** Asia/Shanghai  
Scope: OSGBny `GRID_SPATIAL_MISMATCH` 决策  
**不改 TreeBuilder。不做 nearest-neighbor。不关校验。**

### 本阶段目标

Phase A 之后 OSGBny 仍失败。计划要求先分析再选情况 1/2/3，禁止为了跑通而静默分组。

分析正文已经在 [`../OSGBNY_GRID_ANALYSIS.md`](../OSGBNY_GRID_ANALYSIS.md)，本报告只记录决策。

### 发现的问题

`Tile_x_y` 是空间网格，但 OSGBny 是 6 块离散抽样，边块尺寸不均。origin=`min(grid)` 再按中位间距外推，必然 `GRID_SPATIAL_MISMATCH`。这不是 transform bug。

同一天的 HK `11-NW-10B` 连续 5×4 **通过** 同一套校验。说明规则 Quadtree + 当前 validation 对连续规则格网可用。

### 修改

无代码修改。

### 关键文件

```text
docs/product/OSGBNY_GRID_ANALYSIS.md
docs/product/PHASE_REPORTS/phase-d.md
```

### Tests

未改代码。回归仍是：

```text
cargo test -p top_rebuild --offline
57 passed
```

OSGBny 复跑期望不变：

```text
GRID_SPATIAL_MISMATCH: block Tile_+006_+005 grid=(6,5)
center=(325.003,284.942) expected≈(343.924,275.613) dist=21.095 tol=17.286
```

### Real Data Evidence

| 数据 | 结论 |
|------|------|
| OSGBny 6 块 | 稀疏 + 边块不均，应继续失败 |
| HK 11-NW-10B 20 块 | 连续规则格网，通过 |

### 验收

- [x] 分析问题已回答（见 OSGBNY_GRID_ANALYSIS.md）
- [x] 决策：**情况 2 部分成立，但本轮选「保持诚实失败」**（分析文档选项 1）
- [x] 未关闭 `GRID_SPATIAL_MISMATCH`
- [x] 未按文件顺序强制分组
- [x] 未做 adaptive / bounds-derived quadtree

不选「扩展 sparse-regular validation」的原因：缺块 + 非均匀格子还没有书面规则（容差、origin、是否允许细长边块）。没有设计评审就改校验，容易把错误空间关系收成成功。

### 未解决

- 若产品以后要吃「缺几块的规则格网」，需要单独设计 sparse-regular 校验，再改 `validate_grid_spatial`
- 情况 3 不成立，不要改成按 bounds 自适应建树

### 下一步

计划 Phase E：只有更大真实连续网格才放大。当前只有 5×4，没有 16×16 真数据。
