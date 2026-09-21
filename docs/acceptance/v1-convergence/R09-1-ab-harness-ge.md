# R09.1 Cesium A/B Harness + GE Monotonicity

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r09-1-ab-harness-ge-7ab0

## 任务目标

1. 定义 Cesium A/B harness (convert-flat vs rebuild HLOD)
2. 扩展 schema 添加 A/B metric fields
3. 实现 GE 单调性机器检查（填充 Layer B placeholder）
4. 明确 blocked 状态如果缺少真实数据

## R09 Gate 要求

### Same-tip Far-view A/B Comparison

**对比方法**:
- **Baseline (A)**: Cesium 3D Tiling Pipeline (convert-flat, no HLOD rebuild)
- **Candidate (B)**: GeoForge processor (rebuild HLOD with top_rebuild)
- **Same tip**: 两者使用相同的 input data 和 相同的代码 SHA

**Far-view 场景**:
- 相机距离足够远，只加载 top-level tiles
- 测量 initial load performance
- 不测试 drill-down（近景）

### Pass Thresholds

**必须满足 ≥2/3 metrics**:

| Metric | Threshold | 描述 |
|--------|----------|------|
| **Requests** | −50% | HTTP requests 减少 50% |
| **Bytes** | −40% | Network bytes 减少 40% |
| **tOverview** | −25% | Overview load time 减少 25% |

**示例**:
- Baseline: 100 requests, 10 MB, 2.0s
- Candidate pass: ≤50 requests, ≤6 MB, ≤1.5s (需要 ≥2/3)

### Evidence 非继承

❌ **不允许**:
- 使用旧的 A/B 报告
- 声称 "SHA abc 通过，所以 SHA def 也通过"
- 从其他 tip 继承 far-view 数据

✅ **正确做法**:
- 每个 tip SHA 需要新的 A/B run
- Blocked 标记如果缺少真实数据
- 不虚假声称通过

### Blocked 状态

如果缺少真实数据（D2 LandsD/PlanD 不可用）:
- ❌ 不虚造 pass
- ❌ 不用 D0/D1 代替（不够大，无法测试 far-view HLOD 效果）
- ✅ 标记 **blocked** 等待真实数据
- ✅ 文档化 harness，等待数据可用

## R09.1 Scope

### 本 PR 包含

1. **Cesium A/B harness 定义文档**
   - 对比方法（convert-flat vs rebuild HLOD）
   - Far-view 场景定义
   - Metric 收集方法
   - Pass threshold 验证逻辑

2. **Schema 扩展**
   - `run-info.json` 添加 `cesiumAB` 字段
   - `status.json` 添加 A/B metrics
   - Placeholder 示例

3. **GE 单调性实数检查**
   - 实现 `check_ge_monotonicity` 函数
   - 遍历 tileset tree
   - 验证 parent.GE ≥ child.GE
   - 填充 `spatialQuality.geMonotonicity` (移除 placeholder)

4. **Blocked 状态文档**
   - 明确 D2 数据依赖
   - 不虚造 A/B 结果

### 本 PR 不包含

❌ **推迟到 R09.2+**:
- Cesium A/B 实际运行脚本（需要 Cesium CLI/viewer）
- 真实 D2 数据 A/B 对比
- 其他 Layer B 检查（BV tightness, frontier, subtree）

## Cesium A/B Harness 定义

### Baseline: Cesium 3D Tiling Pipeline

**工具**: `3d-tiles-tools` (Cesium 官方)

**方法**: Convert-flat (no HLOD rebuild)
```bash
# 假设命令（实际需要验证）
npx 3d-tiles-tools tileset-to-tileset \
  --input-tileset input/tileset.json \
  --output-tileset baseline/tileset.json \
  --gzip false
```

**特征**:
- 保留原始 tileset 结构
- 不做 HLOD rebuild
- 可能做基本优化（gltf-pipeline）

### Candidate: GeoForge Processor

**工具**: `processor` + `top_rebuild`

**方法**: Rebuild HLOD
```bash
cargo run --release -p processor -- process-tileset \
  --input input/tileset.json \
  --output candidate/tileset.json \
  --rebuild-top \
  --levels 0  # full pyramid
```

**特征**:
- Top-level HLOD rebuild
- Quadtree pyramid
- 减少 top-level tile 数量

### Far-view 场景定义

**相机设置**:
- 距离: 足够远，只看到 root + L0/L1
- 视角: 俯视 (top-down)
- 视锥: 包含整个 tileset bounding volume

**加载测量**:
1. 清空 cache
2. 加载 tileset.json
3. 渲染一帧（触发 tile selection）
4. 等待 initial tiles 加载完成
5. 记录 metrics

### Metric 收集方法

#### 1. Requests (HTTP 请求数)

**Cesium 方法**:
- Hook `Cesium.Resource` or network inspector
- 计数 GET requests
- 只计算 initial load (before user interaction)

**测量**:
```javascript
let requestCount = 0;
// Hook Cesium network requests
Cesium.Resource.prototype._originalFetch = Cesium.Resource.prototype.fetch;
Cesium.Resource.prototype.fetch = function(...args) {
  requestCount++;
  return this._originalFetch(...args);
};

// Load tileset...
// Wait for ready...
console.log(`Requests: ${requestCount}`);
```

#### 2. Bytes (网络传输字节)

**Cesium 方法**:
- 累加 response body sizes
- 只计算 initial load

**测量**:
```javascript
let totalBytes = 0;
// Hook and measure response sizes
// (实现细节待 R09.2)
```

#### 3. tOverview (Overview 加载时间)

**定义**: 从开始加载到 initial tiles ready 的时间

**Cesium 方法**:
```javascript
const startTime = performance.now();
const viewer = new Cesium.Viewer('cesiumContainer');
const tileset = await Cesium.Cesium3DTileset.fromUrl('tileset.json');
viewer.scene.primitives.add(tileset);

await tileset.readyPromise;
// Wait for initial tiles loaded
await tileset.allTilesLoaded;

const tOverview = performance.now() - startTime;
console.log(`tOverview: ${tOverview}ms`);
```

### Pass Threshold 验证逻辑

```python
def check_ab_pass(baseline, candidate):
    metrics_pass = 0
    
    # Requests
    if candidate.requests <= baseline.requests * 0.5:
        metrics_pass += 1
    
    # Bytes
    if candidate.bytes <= baseline.bytes * 0.6:
        metrics_pass += 1
    
    # tOverview
    if candidate.tOverview <= baseline.tOverview * 0.75:
        metrics_pass += 1
    
    return metrics_pass >= 2  # Need ≥2/3
```

## Schema 扩展

### run-info.json 添加 cesiumAB

```json
{
  "runId": "20260921-172600",
  "timestamp": "2026-09-21T17:26:00Z",
  "tipSha": "8a8cb82",
  "level": "d2",
  "binaries": { ... },
  "parameters": { ... },
  "environment": { ... },
  "spatialQualityChecks": { ... },
  "cesiumAB": {
    "enabled": true,
    "status": "blocked",
    "blockedReason": "D2 real data not available (pending LandsD on LM)",
    "baseline": {
      "tool": "3d-tiles-tools",
      "version": "unknown",
      "method": "convert-flat"
    },
    "candidate": {
      "tool": "geoforge-processor",
      "version": "0.1.0",
      "method": "rebuild-hlod",
      "tipSha": "8a8cb82"
    },
    "farViewScenario": {
      "cameraDistance": "far",
      "viewport": "1920x1080",
      "description": "Top-down view covering entire tileset"
    },
    "note": "A/B comparison blocked pending D2 data availability"
  }
}
```

### status.json 添加 cesiumAB metrics

```json
{
  "fixture": "d2-landsd/city-2024",
  "passed": false,
  "cesiumAB": {
    "enabled": false,
    "status": "blocked",
    "blockedReason": "D2 data not available",
    "baseline": {
      "requests": null,
      "bytes": null,
      "tOverview": null,
      "note": "Baseline not run (blocked)"
    },
    "candidate": {
      "requests": null,
      "bytes": null,
      "tOverview": null,
      "note": "Candidate not run (blocked)"
    },
    "comparison": {
      "requestsReduction": null,
      "bytesReduction": null,
      "tOverviewReduction": null,
      "metricsPassed": 0,
      "overallPass": false,
      "note": "A/B blocked pending D2 data"
    }
  }
}
```

## GE 单调性实数检查

### 实现

创建 `crates/processor/src/validation/ge_monotonicity.rs`:

```rust
/// Check GE monotonicity: parent.geometricError >= all children.geometricError
pub fn check_ge_monotonicity(tileset_path: &Path) -> Result<GeMonotonicityResult> {
    let tileset = load_tileset(tileset_path)?;
    let violations = traverse_and_check(&tileset.root, None)?;
    
    Ok(GeMonotonicityResult {
        checked: true,
        passed: violations.is_empty(),
        violations,
        total_tiles: count_tiles(&tileset.root),
    })
}

struct Violation {
    parent_tile: String,
    parent_ge: f64,
    child_tile: String,
    child_ge: f64,
}
```

### 集成到 run-acceptance.sh

在 status.json 生成时，调用 GE 检查：

```bash
# 调用 processor 的 validate 子命令（如果实现）
# 或者 单独的 check-ge 工具

GE_CHECK=$(processor check-ge --tileset "$FIXTURE_OUTPUT/output/tileset.json")
GE_PASSED=$(echo "$GE_CHECK" | jq -r '.passed')
GE_VIOLATIONS=$(echo "$GE_CHECK" | jq -r '.violations | length')
```

更新 `spatialQuality.geMonotonicity`:
```json
{
  "geMonotonicity": {
    "checked": true,
    "passed": true,
    "violations": 0,
    "totalTiles": 12,
    "note": "All parent GE >= children GE"
  }
}
```

## Blocked 状态文档

### 当前状态

**D2 数据状态**: ⚠️ **Blocked - Pending LandsD/PlanD on LM**

**原因**:
- LandsD/PlanD 数据路径未提供
- 无法访问本地 LM 环境
- D0/D1 太小，无法测试 far-view HLOD 效果

### D0/D1 不适用于 A/B

**为什么 D0/D1 不能用于 A/B**:
- D0: 140 bytes, 单 tile - 无 HLOD hierarchy
- D1: 6 KB, 2x2 grid - HLOD 效果不明显
- Far-view 需要: 大规模数据 (1GB+), 深层 hierarchy, 才能体现 HLOD 优势

**D0/D1 用途**:
- ✅ Smoke test (processor 不 crash)
- ✅ Schema 验证
- ❌ 不适合 far-view A/B 对比

### 如何解除 Blocked

1. **获取 D2 数据**: LandsD 或 PlanD 在本地 LM 可用
2. **设置 Cesium baseline**: 安装 3d-tiles-tools, 运行 convert-flat
3. **运行 A/B harness**: 使用真实数据
4. **记录 metrics**: requests, bytes, tOverview
5. **验证 thresholds**: ≥2/3 pass
6. **更新文档**: 移除 blocked 状态，记录真实结果

### 不虚造结果

❌ **绝不做**:
- 编造 A/B metrics
- 用 D0/D1 假装 A/B pass
- 声称 "blocked 但应该会 pass"

✅ **正确做法**:
- 明确 "blocked"
- 等待真实数据
- 首次运行后更新文档

## 本 PR 可测试内容

### GE 单调性 on D0/D1

可以在 D0/D1 上测试 GE 单调性检查：
- ✅ D0 single-tile: 应该 pass (无 children)
- ✅ D1 small-grid: 应该 pass (手工构造 GE 单调)

这验证了 GE 检查代码正确性，但不是 A/B 对比。

### Schema 验证

可以验证 schema 格式：
- ✅ run-info.json 包含 cesiumAB 字段
- ✅ status.json 包含 blocked 标记
- ✅ 不虚假声称 pass

## R09.1 限制

### 不包含的内容

❌ **R09.2+ 实现**:
1. Cesium A/B 实际运行脚本（需要 Cesium viewer 集成）
2. Baseline (3d-tiles-tools) 自动化
3. Metric 自动收集（需要 browser automation）
4. 真实 D2 数据 A/B 运行
5. 其他 Layer B 检查（BV, frontier, subtree）

### Blocked 直到

- D2 LandsD/PlanD 数据可用
- Cesium viewer 自动化就绪
- Network metric 收集实现

## 总结

### R09.1 交付

1. ✅ **Cesium A/B harness 定义文档**
   - 对比方法
   - Far-view 场景
   - Metric 定义
   - Pass threshold

2. ✅ **Schema 扩展**
   - cesiumAB 字段
   - Blocked 状态支持

3. ✅ **GE 单调性检查**
   - 实现并集成
   - 填充 Layer B placeholder

4. ✅ **Blocked 状态明确**
   - 不虚造结果
   - 等待 D2 数据

### R09 Gate 合规

- ✅ Same-tip A/B 定义
- ✅ Pass threshold 明确 (≥2/3)
- ✅ Evidence 非继承原则
- ✅ Blocked 标记（不虚造）

### 下一步 (R09.2+)

- 实现 Cesium A/B 运行脚本
- 获取 D2 数据
- 运行真实 A/B
- 解除 blocked 状态

---

**状态**: R09.1 harness 定义完成，GE 检查实现，A/B blocked 待数据
**Evidence**: 严格，无虚造，blocked 明确
