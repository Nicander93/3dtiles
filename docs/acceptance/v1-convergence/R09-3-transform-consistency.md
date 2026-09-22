# R09.3 Transform Consistency (Layer A+)

**日期:** 2026-09-21  
**状态:** 进行中  
**分支:** cursor/r09-3-transform-consistency-7ab0

## 任务目标

实现 Transform Consistency 检查 (Layer A+ structural hook)，验证 tileset 的 transform chain 正确性。

## Transform Consistency 定义

### 什么是 Transform Consistency

**Transform**: 3D Tiles 中的 `transform` 矩阵（4x4）将 tile 内容从 local 坐标转换到 parent 坐标系。

**Consistency**: 
1. Transform 矩阵格式正确（16 个数字）
2. Transform chain 不断裂（child 继承 parent transform）
3. Transform 应用后 bounding volume 合理（不会导致极端坐标）

### D1 Small-Grid Transform Chain

```
Root tile (no transform, or identity)
  └─ Child Tile_+000_+000 (transform: [...])
      └─ LOD1 tile (transform: [...])
```

**检查内容:**
- Root transform 存在或为 identity
- Child transforms 存在且格式正确
- 没有 NaN / Infinity 值
- Transform chain depth 记录

## R09.3 Scope

### 包含的内容

1. **Transform Format 检查**
   - 16 个 float64 值
   - 无 NaN / Infinity
   - Matrix 行主序或列主序一致性

2. **Transform Chain 深度**
   - 记录最大 chain depth
   - 验证 chain 不断裂

3. **集成到 run-acceptance.sh**
   - D1 运行检查
   - 更新 `status.json`

### 不包含的内容（超出 Layer A+ scope）

❌ **推迟到后续或不做:**
- ECEF 坐标验证（需要地理库）
- Transform 矩阵数学验证（行列式、正交性等）
- Bounding volume transform 后的精确计算
- 视觉质量影响分析

## 实现

### transform_consistency.rs

```rust
pub struct TransformConsistencyResult {
    pub checked: bool,
    pub passed: bool,
    pub max_chain_depth: usize,
    pub tiles_with_transform: usize,
    pub tiles_without_transform: usize,
    pub invalid_transforms: Vec<String>,
}

pub fn check_transform_consistency(tileset_path: &Path) -> Result<TransformConsistencyResult> {
    let tileset = load_tileset(tileset_path)?;
    
    let mut max_depth = 0;
    let mut tiles_with = 0;
    let mut tiles_without = 0;
    let mut invalid = Vec::new();
    
    traverse_and_check(&tileset.root, 0, &mut max_depth, &mut tiles_with, 
                       &mut tiles_without, &mut invalid)?;
    
    Ok(TransformConsistencyResult {
        checked: true,
        passed: invalid.is_empty(),
        max_chain_depth: max_depth,
        tiles_with_transform: tiles_with,
        tiles_without_transform: tiles_without,
        invalid_transforms: invalid,
    })
}

fn validate_transform_matrix(transform: &[f64; 16], tile_uri: &str) -> Option<String> {
    for (i, &val) in transform.iter().enumerate() {
        if val.is_nan() {
            return Some(format!("{}: transform[{}] is NaN", tile_uri, i));
        }
        if val.is_infinite() {
            return Some(format!("{}: transform[{}] is Infinity", tile_uri, i));
        }
    }
    None
}
```

## 集成到 run-acceptance.sh

### D1 部分

```bash
# Run transform consistency check (Layer A+)
if TRANSFORM_RESULT=$("$PROCESSOR_PATH" check-transform --tileset "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null); then
    TRANSFORM_CHECKED=$(echo "$TRANSFORM_RESULT" | jq -r '.checked')
    TRANSFORM_PASSED=$(echo "$TRANSFORM_RESULT" | jq -r '.passed')
    TRANSFORM_DEPTH=$(echo "$TRANSFORM_RESULT" | jq -r '.max_chain_depth')
    TRANSFORM_WITH=$(echo "$TRANSFORM_RESULT" | jq -r '.tiles_with_transform')
else
    TRANSFORM_CHECKED="false"
    TRANSFORM_PASSED="false"
    TRANSFORM_DEPTH=0
    TRANSFORM_WITH=0
fi
```

### status.json

```json
{
  "spatialQuality": {
    "transformConsistency": {
      "checked": true,
      "passed": true,
      "maxChainDepth": 2,
      "tilesWithTransform": 8,
      "tilesWithoutTransform": 0,
      "invalidTransforms": 0,
      "note": "Transform consistency check (R09.3)"
    }
  }
}
```

## D0 不运行此检查

**原因:** D0 single-tile 结构太简单，transform chain depth = 1，不具代表性

## 测试计划

### Unit Tests

1. **test_transform_valid** - 正常 transform 矩阵
2. **test_transform_nan** - 包含 NaN 值
3. **test_transform_infinity** - 包含 Infinity 值
4. **test_no_transform** - 无 transform（使用 identity）

### Integration Tests

- D1 small-grid: transform check 应该 PASS

## R09.3 限制

**简化实现原因:**
- ECEF 验证需要 geospatial 库（proj, geodesy），增加依赖
- Transform 矩阵数学验证（正交性、行列式）复杂且非必需
- 当前 focus: format 正确性和 chain 完整性

**足够用于:**
- 检测明显错误（NaN, Infinity, missing）
- 验证 transform chain 不断裂
- Layer A+ structural hook（不涉及实数质量）

## A/B Status

- ⚠️ **仍然 blocked** (R09.3 不影响 A/B)
- 等待 parent 在 LM 上执行 D2 checklist
- 不虚造 A/B metrics

## 下一步

**R09.4+ 候选:**
- BV tightness (需要 GLB 几何解析)
- GE reasonableness (实数范围检查)
- 或者转向 R10 (fixed runtime install)

**等待 D2:**
- Parent 完成 LM checklist
- D2 sample 验证
- Cesium A/B baseline 可运行

---

**Status:** R09.3 实现中，简化版 transform consistency  
**Evidence:** 严格，A/B blocked 待 D2  
**D2 Checklist:** 已提供给 parent (D2-runnable-checklist-LM.md)
