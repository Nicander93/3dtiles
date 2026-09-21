# R08.2 Spatial Quality Checklist

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r08-2-spatial-quality-7ab0

## 任务目标

1. 定义空间质量检查项（machine-checkable 优先）
2. 扩展 run-info.json 和 status.json schema
3. D1 fixture 定义和获取说明
4. Layer B / GE·REPLACE 实数 placeholders

## 空间质量检查项

### Layer A - Machine Checkable（当前实现）

| 检查项 | 描述 | 实现 | 输出字段 |
|--------|------|------|----------|
| **Tileset 结构** | 基础 JSON schema 正确性 | ✅ Layer A validator | `validation.layerA` |
| **URI 解析** | 所有 content URI 可解析，无 cycle | ✅ Layer A validator | `validation.errors` |
| **BoundingVolume 存在** | 所有 tile 有 BV | ✅ Layer A validator | `validation.errors` |
| **Transform 合法性** | Transform 矩阵可解析 | ✅ Layer A validator | `validation.errors` |
| **Content 可读** | B3DM/GLB header 有效 | ✅ Layer A validator | `validation.errors` |

### Layer A+ - Structural Hooks（本 PR）

| 检查项 | 描述 | 实现 | 输出字段 |
|--------|------|------|----------|
| **Frontier coverage** | L0 blocks 覆盖 frontier | ⚠️ Hook 定义 | `spatialQuality.frontierCoverage` |
| **Subtree retention** | 外部 block tileset 保留 | ⚠️ Hook 定义 | `spatialQuality.subtreeRetention` |
| **Transform consistency** | World-space transform 一致性 | ⚠️ Hook 定义 | `spatialQuality.transformConsistency` |

### Layer B - Real-Number Quality（Placeholders）

| 检查项 | 描述 | 实现 | 输出字段 |
|--------|------|------|----------|
| **GE 单调性** | 父 GE ≥ 所有子 GE | ❌ 未实现 | `spatialQuality.geMonotonicity` |
| **GE 合理性** | GE 与实际几何误差接近 | ❌ 未实现 | `spatialQuality.geReasonableness` |
| **BV 紧密度** | BV 与实际几何紧密 | ❌ 未实现 | `spatialQuality.bvTightness` |
| **REPLACE 切换正确性** | REPLACE refine 在合理 GE | ❌ 未实现 | `spatialQuality.replaceCorrectness` |

### Visual Quality（R09+）

| 检查项 | 描述 | 实现 | 工具 |
|--------|------|------|------|
| **纹理保真度** | 无明显失真 | ❌ R09 | CesiumJS 截图对比 |
| **接缝检测** | 无明显接缝 | ❌ R09 | 视觉对比 |
| **坐标系正确性** | 地理位置正确 | ❌ R09 | 关键点对比 |

## 扩展的 Schema

### run-info.json

添加 `spatialQualityChecks` 字段：

```json
{
  "runId": "20260921-162900",
  "timestamp": "2026-09-21T16:29:00Z",
  "tipSha": "a489436",
  "binaries": { ... },
  "parameters": { ... },
  "environment": { ... },
  "spatialQualityChecks": {
    "enabled": true,
    "layerAEnabled": true,
    "layerAStructuralHooksEnabled": true,
    "layerBEnabled": false,
    "layerBNote": "Real-number quality checks not yet implemented (R09+)"
  }
}
```

### status.json (per-fixture)

添加 `spatialQuality` 字段：

```json
{
  "fixture": "d0-tiny/single-tile",
  "passed": true,
  "duration": "1.2s",
  "validation": {
    "layerA": "PASS",
    "errors": 0,
    "warnings": 0
  },
  "spatialQuality": {
    "frontierCoverage": {
      "checked": false,
      "note": "D0 too small for frontier analysis"
    },
    "subtreeRetention": {
      "checked": true,
      "passed": true,
      "blocksExpected": 1,
      "blocksRetained": 1
    },
    "transformConsistency": {
      "checked": false,
      "note": "Single block, no transform chain"
    },
    "geMonotonicity": {
      "checked": false,
      "note": "Layer B not implemented (R09+)"
    },
    "geReasonableness": {
      "checked": false,
      "note": "Layer B not implemented (R09+)"
    },
    "bvTightness": {
      "checked": false,
      "note": "Layer B not implemented (R09+)"
    },
    "replaceCorrectness": {
      "checked": false,
      "note": "Layer B not implemented (R09+)"
    }
  },
  "outputSize": "1234 bytes"
}
```

## Frontier Coverage

**定义**: L0 source blocks 的 grid 覆盖是否完整，无空洞。

**检查方法**:
1. 扫描 tileset root children，提取所有 `Tile_+xxx_+yyy`
2. 构建 (grid_x, grid_y) 集合
3. 检查是否形成连续矩形或已知模式

**实现状态**: ⚠️ Hook 定义，暂不实现代码

**Placeholder 输出**:
```json
{
  "frontierCoverage": {
    "checked": false,
    "note": "Frontier analysis requires grid topology (R09+)",
    "gridSize": "1x1",
    "blocksCount": 1
  }
}
```

## Subtree Retention

**定义**: 外部 block tileset.json 是否完整保留在输出中。

**检查方法**:
1. 对比输入 `Tile_xxx_yyy/tileset.json` 和输出对应文件
2. 验证 content URIs 保持不变
3. 验证 boundingVolume 保持不变

**实现状态**: ⚠️ Hook 定义，暂不实现代码

**Placeholder 输出**:
```json
{
  "subtreeRetention": {
    "checked": true,
    "passed": true,
    "blocksExpected": 1,
    "blocksRetained": 1,
    "details": "All external block tilesets preserved"
  }
}
```

## Transform Consistency

**定义**: World-space transform 链是否一致，ECEF 坐标系是否合理。

**检查方法**:
1. 追踪从 root 到 leaf 的 transform 链
2. 验证复合 transform 合理性
3. 检查 ECEF bounds 是否在地球表面附近

**实现状态**: ⚠️ Hook 定义，暂不实现代码

**ECEF 注意**:
- 合理的地表点：sqrt(x²+y²+z²) ≈ 6.3-6.4 million meters
- 不合理值（如全零、超大值）应 warn

**Placeholder 输出**:
```json
{
  "transformConsistency": {
    "checked": false,
    "note": "Transform chain analysis not implemented (R09+)",
    "ecefNote": "ECEF bounds validation pending"
  }
}
```

## GE Monotonicity (Layer B Placeholder)

**定义**: 父 tile 的 geometricError ≥ 所有子 tile 的 geometricError。

**检查方法**:
1. 遍历 tileset 树
2. 对每个 parent-child 对，验证 parent.GE ≥ child.GE
3. 记录违反点

**实现状态**: ❌ 未实现（Layer B，推迟到 R09）

**Placeholder 输出**:
```json
{
  "geMonotonicity": {
    "checked": false,
    "note": "Layer B real-number checks not implemented (R09+)",
    "reason": "Requires full tileset tree traversal + GE comparison"
  }
}
```

## GE Reasonableness (Layer B Placeholder)

**定义**: geometricError 与实际几何误差（SSE 投影）接近。

**检查方法**:
1. 加载 tile content (B3DM/GLB)
2. 计算实际几何范围和细节
3. 对比 GE 值是否合理（例如：GE 不应小于最小三角形边长）

**实现状态**: ❌ 未实现（需要几何分析）

**Placeholder 输出**:
```json
{
  "geReasonableness": {
    "checked": false,
    "note": "Requires GLB geometry analysis (R10+)"
  }
}
```

## BV Tightness (Layer B Placeholder)

**定义**: BoundingVolume 与实际几何紧密，无过度膨胀。

**检查方法**:
1. 加载 tile content
2. 计算实际几何 AABB/OBB
3. 对比 tileset 中的 BV
4. 计算 tightness ratio（BV 体积 / 实际几何体积）

**实现状态**: ❌ 未实现（需要几何分析）

**Placeholder 输出**:
```json
{
  "bvTightness": {
    "checked": false,
    "note": "Requires GLB geometry analysis (R10+)"
  }
}
```

## REPLACE Correctness (Layer B Placeholder)

**定义**: 使用 REPLACE refine 的 tile 在合理的 GE 切换。

**检查方法**:
1. 识别 REPLACE tiles
2. 验证 parent/child GE 差异合理
3. 验证 REPLACE 不在叶子 tile

**实现状态**: ❌ 未实现（Phase 11 REPLACE 支持有限）

**Placeholder 输出**:
```json
{
  "replaceCorrectness": {
    "checked": false,
    "note": "REPLACE switching analysis not implemented (R11+)"
  }
}
```

## D1 Fixture 定义

### 选项 1: Tiny Synthetic D1（推荐）

**方案**: 手工构造一个更大但仍然 license-safe 的 fixture。

**内容**:
- 4 个 Tile blocks (2x2 grid)
- 每个 block 包含 2-3 个 tiles (简单层级)
- 总大小: ~1-5MB
- License: MIT (repository-created)

**优点**:
- ✅ 可提交到 repo
- ✅ CI 可运行
- ✅ 完全控制内容

**缺点**:
- ⚠️ 不是真实几何（无法测试视觉质量）
- ⚠️ 需要手工构造较多文件

**状态**: 本 PR 定义，R08.3 实现

### 选项 2: 开源数据集

**候选数据集**:
1. **Cesium 示例数据**: 
   - URL: https://github.com/CesiumGS/3d-tiles-samples
   - License: Apache 2.0 或 CC-BY
   - 状态: 需要验证许可和大小

2. **Open Tileset**:
   - 一些开源城市数据
   - License: 待确认
   - 状态: 需要调研

**实现**: R08.3+ 调研和获取

### D1 Fixture 结构（计划）

```
fixtures/d1-medium/
├── README.md           # 获取说明
├── small-grid/         # Synthetic 2x2 grid (本 PR 定义)
│   ├── tileset.json
│   ├── Tile_+000_+000/
│   ├── Tile_+000_+001/
│   ├── Tile_+001_+000/
│   └── Tile_+001_+001/
└── cesium-sample/      # 外部数据（R08.3+，如果可用）
    └── download.sh
```

## 本 PR 实现

### 代码改动

❌ **无代码改动**

本 PR 仅定义 schema 和 checklist，不实现实际检查逻辑。

### 文档改动

✅ **文档改动**:
1. 本文档 (`R08-2-spatial-quality.md`)
2. 更新 `test-plan.md` 添加 spatial quality 章节
3. 创建 `fixtures/d1-medium/README.md` 占位
4. 更新 `status.md`

### Schema 示例

创建 `docs/acceptance/v1-convergence/schemas/` 目录，提供示例：
- `run-info-example.json`
- `status-example.json`

## 下一步 (R08.3+)

1. **R08.3**: 实现 D1 synthetic fixture 生成
   - 2x2 grid, 每个 block 2-3 tiles
   - 或者获取 Cesium 示例数据
2. **R09**: Layer B 实数检查实现
   - GE 单调性验证
   - BV tightness 分析
3. **R10**: 几何分析工具
   - GLB 加载和分析
   - GE reasonableness 检查

## 与 Cesium A/B 对比集成（R09）

Layer B 实数检查需要与 Cesium baseline 对比：
1. 使用 Cesium 3D Tiling Pipeline 处理相同输入
2. 对比 GE、BV、transform
3. 视觉截图对比

这需要：
- Cesium tooling 集成
- 自动化截图对比工具
- Baseline 数据存储

**推迟到 R09**。

---

**状态**: Schema 定义完成，Layer B placeholders 就绪
**代码**: 无改动（仅文档）
**下一步**: R08.3 D1 fixture 实现
