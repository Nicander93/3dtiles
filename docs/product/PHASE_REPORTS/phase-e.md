## Phase E

Date: **2026-09-12** Asia/Shanghai  
Scope: 真实数据逐级放大  
**16×16 真数据已跑通。到此停止：没有代表性城区、没有百平方公里。**

### 本阶段目标

C 通过后按 4×4/8×8 → 16×16 → 城区 → 百平方公里放大。一层失败或没有数据就停。

### 发现的问题

1. 绝对 `Tile_+x_+y` 当下标做 `floor(/2)` 时，起点不整除 2^k 会卡在 4 个节点合不掉（`NO_FURTHER_REDUCTION`）。HK 窗口 `5567–5582 × 5469–5484` 第一次重建就是这样。8×8 窗口碰巧最后一层对齐，所以当时没暴露。
2. GE 阶梯仍是 1040 / 1030 / 1020 / 1010 / 1000。远景 Cesium 仍会走进原 Block 的 L14+。

### 修改

`tree_builder::l0_nodes` 用本批 Block 的最小 `grid_x/grid_y` 做原点，再聚合。空间校验仍用文件名格子和 world center，没有改 TreeBuilder 的 2×2 规则。

### 关键文件

```text
crates/top_rebuild/src/tree_builder.rs
data/real/hk_8x8/
data/real/hk_16x16/
docs/product/REAL_DATA_VALIDATION.md
docs/product/PHASE_REPORTS/phase-e.md
```

### Tests

`cargo test -p top_rebuild --offline`：**58 passed**（含 `offset_16x16_reaches_root`）。

```text
docker run winner1/3dtiles:1.0 _3dtile -f osgb -i /data/hk_16x16/osgb -o /data/hk_16x16/3dtiles
target/release/top_rebuild -i data/real/hk_16x16/3dtiles -o data/real/hk_16x16/rebuild
```

### Real Data Evidence

| 规模 | 数据 | 结果 |
|------|------|------|
| 5×4 real（C） | HK 11-NW-10B | convert 20 s，rebuild 39 s，L0–L3 |
| 8×8 real | 10A/B/C/D 子集，x=5575–5582 y=5477–5484 | convert 50.25 s，rebuild 72.35 s（改原点前的树：64→25→9→4→1） |
| 16×16 real | 9/10/14/15 图幅，x=5567–5582 y=5469–5484 | convert 205.74 s，rebuild 373.5 s，**256→64→16→4→1**，无 `GRID_SPATIAL_MISMATCH` |
| 代表性城区 | 无 | **停止** |
| 百平方公里 | 无 | **停止** |

16×16 构建：

| 项 | 值 |
|----|-----|
| 输入 Block | 256 |
| 输入 OSGB | 10183.89 MB |
| 转换输出 | 13447.53 MB |
| 重建输出 | 13580.82 MB |
| 树 | L0 256 / L1 64 / L2 16 / L3 4 / L4 1 |
| probe | proxies=85 leaf_external=256 replace=341 max_depth=4 |
| proxy 三角形 | 1217794 |
| proxy 纹理 | 62.25 MB |
| convert | 205.74 s |
| rebuild | 373.5 s |
| maxGap / P95Gap | 1232.93 m / 1144.00 m |
| L4 预算 | LockBorder 下 tris 300076 / 目标 2000；GLB 22.4 MB / 16 MB |

Cesium（`cesium-preview.html` + 本地 8765）：

| 观察 | 8×8 | 16×16 |
|------|-----|--------|
| 定位 | lon=114.18361 lat=22.33832，r≈981 m | lon=114.17764 lat=22.33273，r≈1953 m |
| 与底图 | 压在黄大仙 / 横头磡 / 乐富 | 同一带向西、南扩满约 2.4 km 矩形，北侧狮子山 |
| 翻转 / 整块消失 | 未见 | 未见，256 块矩形完整 |
| tilesLoaded | true | true；远景 selected=256，85 个 Proxy 已加载 |

### 验收

- [x] 5×4 真实构建数字已记录
- [x] 8×8 real convert + rebuild + Cesium
- [x] 16×16 real convert + rebuild + Cesium
- [ ] 城区 — 无数据，停止
- [ ] 百平方公里 — 无数据，停止
- [ ] baseline vs rebuild 调度 / 网络对比

文档仍不得写成「支持大范围倾斜摄影顶层重建」。16×16 真数据约 2.4 km × 2.4 km，不是城区，更不是百平方公里。

### 未解决

- GE 未标定，放大后同样会跳过代理独占远景
- LockBorder 下 gap / budget 随尺度变差；L4 已明显超预算
- 没有再往上的连续真 OSGB

### 下一步

计划 Phase F：质量参数标定。本轮只做 UI 三档映射，数值不按城区数据标定。
