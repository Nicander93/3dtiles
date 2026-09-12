## Phase F

Date: **2026-09-12** Asia/Shanghai  
Scope: 质量参数标定（有限）  
**未按城区数据重算预算。未改 ProxyBuilder。**

### 本阶段目标

计划写明：真实数据跑通之前不要花大量时间调参；E 至少一个城区之后再标定。  
E 停在 5×4，所以本阶段只做两件事：把用户面对的旋钮收成三档，并把未标定的项写清楚。

### 发现的问题

1. 转换页仍是 `rebuildTop.levels` / `texture.mode` 开发参数。
2. 真数据上 GE 阶梯、gap 1 m、L2/L3 三角形预算都不合适，但改公式属于标定/算法，没有城区样本就动会骗过 synthetic。

### 修改

- OSGB 转换、Tiles 处理增加「质量优先 / 均衡 / 性能优先」
- 映射：质量 → levels=2 + keep；均衡 → levels=1 + keep；性能 → levels=1 + 若有 basisu 则 ktx2-etc1s
- 开发用的层数、纹理下拉仍保留

未改：

```text
sourceErrorRatio
targetErrorMeters
L1/L2/L3+ maxTriangles
maxTextureSize / maxTextureBytes / maxGlbBytes
gap warning threshold
geometric_error_proxy
```

### 关键文件

```text
apps/desktop/src/pages/OsgbConvert.tsx
apps/desktop/src/pages/ProcessTiles.tsx
```

### Tests

无新 Rust 测试。桌面 TypeScript 未单独加单测。

```text
cargo test -p top_rebuild --offline
57 passed
```

### Real Data Evidence

三档没有在 20 块上分别重跑。默认（全金字塔、keep）即 Phase C 那次。

建议以后用同一份 5×4 标定，而不是先做 32×32 synthetic：

| 现象 | 可能方向（未实现） |
|------|-------------------|
| Block 根 GE=1000，代理 1010/1020/1030 | 父节点 GE 按空间尺度，或压低 L0 引用 GE |
| maxGap 471 m、阈值 1 m | 按块尺寸给警告阈值 |
| L3 `after=20977 max=2000` + LockBorder | 提高上层预算或接受 LockBorder 保边 |

### 验收

- [x] UI 有三档，并写明未按城区标定
- [ ] sourceErrorRatio 等内部参数已用城区数据标定
- [ ] 用户可以完全不看开发参数（层数/纹理仍在）

### 未解决

- GE / gap / 三角形预算仍是 Phase 8 默认值
- 性能优先在没有 basisu 时与均衡相同

### 下一步

计划 Phase G：sidecar 查找与安装包。本轮只做查找规则，不做完整 Windows 安装包冒烟。
