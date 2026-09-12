## Phase C

Date: **2026-09-12** Asia/Shanghai  
Scope: 真实连续 4×4～8×8 倾斜摄影全链路  
**未改 TreeBuilder。未实现 Adaptive Quadtree。**

### 本阶段目标

A/B 只在 synthetic 和 OSGBny 稀疏集上验证过。本阶段用一份连续真 OSGB 走用户路径：转换 → 重建 → Cesium，确认坐标、原 Block、代理覆盖，而不是只打开 `tileset.json`。

### 发现的问题

1. 用户从 `3d.map.gov.hk` 下的 `11-NW-10B.zip`（839 MB）是 **单体建筑 FBX**，不能进 TopRebuild。
2. 同图号的 Tile-based **OSGB** 分 20 个约 20–45 MB 的 zip，拼起来是连续 5×4。
3. 转换器给每个 Block 根写 `geometricError=1000`，代理层只比它大 1%～3%，Cesium 几乎不会停在 Proxy 上。
4. `KHR_techniques_webgl` 在两块预检时已兼容；20 块重建未再因材质失败。

### 修改

本阶段没有改算法。沿用 A/B 的 transform、LOD 整目录复制、默认不 synthesize。  
Processor sidecar 查找补了 Windows `.exe`（给 Phase G），与 C 验收无关。

### 关键文件

```text
data/real/hk_11-NW-10B/osgb
data/real/hk_11-NW-10B/sheet_3dtiles
data/real/hk_11-NW-10B/sheet_rebuild
docs/product/REAL_DATA_VALIDATION.md
```

### Tests

```powershell
cd d:\code\3dtiles
cargo test -p top_rebuild --offline
cargo test -p processor --offline
```

```text
cargo test -p top_rebuild --offline
cargo test: 57 passed (11 suites, 0.58s)

cargo test -p processor --offline
cargo test: 0 passed (3 suites, 0.00s)   # 编译通过
```

真实链路（不是 fixture）：

```powershell
docker run --rm -v "D:\code\3dtiles\data\real:/data" winner1/3dtiles:1.0 `
  /bin/bash -lc "cd /3dtiles && export LD_LIBRARY_PATH=./lib && ./target/release/_3dtile -f osgb -i /data/hk_11-NW-10B/osgb -o /data/hk_11-NW-10B/sheet_3dtiles -v"
# 20.03 s, exit 0

.\target\release\top_rebuild.exe `
  -i data\real\hk_11-NW-10B\sheet_3dtiles `
  -o data\real\hk_11-NW-10B\sheet_rebuild
# 39.13 s, exit 0, L0 20 / L1 9 / L2 4 / L3 1
```

### Real Data Evidence

见 [`../REAL_DATA_VALIDATION.md`](../REAL_DATA_VALIDATION.md)。

| dataset | blocks | input OSGB | baseline | rebuild | runtime | 树 |
|---------|--------|------------|----------|---------|---------|-----|
| HK LandsD `11-NW-10B` OSGB | 20（5×4） | 888.67 MB | 1172.48 MB | 1180.04 MB | convert 20 s + rebuild 39 s | 20→9→4→1 |

### 验收

- [x] 坐标正确（ECEF，黄大仙一带，未落到地心）
- [x] 原 Block LOD 完整（L14–L22 仍在外部 tileset）
- [x] Proxy 节点写全（14 个，覆盖 20 叶）
- [x] 无整块消失、无明显翻转 / 错比例
- [x] 近景纹理可辨
- [x] metrics 文件完整
- [~] Cesium 里 REPLACE **写进了 tileset**（34 个节点）；因 GE 阶梯过窄，远景统计不到 `Proxy_L*` 被选中，原 Block 自己的 L14+ 在切

### 未解决

- geometricError 阶梯 1000/1010/1020/1030，远景代理调度弱
- gap 阈值 1 m、LockBorder 下预算警告——留给 Phase F 标定
- 没有 16×16 真数据，不能进城区 / 百平方公里
- 未做 baseline vs rebuild 的请求数 / 下载量 / FPS

### 下一步

计划 Phase D：OSGBny / 稀疏网格只出分析结论，不改 TreeBuilder。
