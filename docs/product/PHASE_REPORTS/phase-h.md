## Phase H

Date: **2026-09-12** Asia/Shanghai  
Scope: 真实数据 geometricError 标定、离线 Cesium 与调度观测

### 结果

- 转换器写死的 Block 根 `geometricError=1000` 会先按块空间尺度校正。
- Proxy 父节点同时按 child GE、空间对角线和简化误差形成清晰递增阶梯。
- 预览的 `debug=1` 模式可切换近景、中景、远景、超远景，并显示可见代理、原始块和 FPS。
- Cesium 1.125 运行时随前端构建复制，默认不依赖公网脚本或在线底图。

### 真实 5×4 验证

输入：`data/real/hk_11-NW-10B/sheet_3dtiles`  
输出：`data/real/hk_11-NW-10B/sheet_rebuild_ge_v1`（gitignored）

```text
release rebuild: 28.9 s
tree: 20 → 6 → 2 → 1
leaf GE: 163.7–198.0
L1 GE: 339.0–447.0
L2 GE: 745.1–893.9
L3 GE: 1787.9
```

Cesium 1.125 / WebGL 实测：

| 视距 | 可见代理 | 可见原始块 | 结果 |
|---|---:|---:|---|
| 中景（10×半径） | 0 | 20 | 原始 Block LOD |
| 远景（40×半径） | 6（L1） | 0 | 代理生效 |
| 超远景（100×半径） | 2（L2） | 0 | 更粗代理生效 |

这证明代理 HLOD 会随视距接管渲染。当前观测是在本机浏览器完成，不把约 56–57 FPS 当成跨设备性能结论。

### 验证

```text
cargo test -p top_rebuild -p processor --offline: 62 passed
npm run build: passed
offline Cesium runtime: script / Widgets / Workers / Assets / LICENSE present
real tileset: tilesLoaded; lon=114.18573 lat=22.34087; texture visible
```

Processor Windows 全链路也已复跑：

```text
20-block OSGB → Docker converter: 21.16 s
convert validate + atomic commit: passed
process-tileset → Rust TopRebuild: passed
rebuild validate + atomic commit: passed
Cesium: tilesLoaded at lon=114.18573 lat=22.34087
cancel during rebuild: CANCELLED; final output absent; temp directory removed
```

### 仍未完成

- gap 指标仍受不同 child 局部坐标影响，当前数值只作警告。
- LockBorder 下部分代理不能压到三角形预算。
- Windows 安装包仍需补齐 converter 等 sidecar 并做安装后冒烟。
