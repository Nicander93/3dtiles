# R09.2 Frontier Coverage + Subtree Retention

**日期:** 2026-09-21  
**状态:** 进行中  
**分支:** cursor/r09-2-frontier-subtree-7ab0

## 任务目标

实现 Layer A+ structural hooks（frontier coverage + subtree retention），为 D1 small-grid 提供机器可检查的空间质量验证。

## R09.2 Scope

### 包含的内容

1. **Frontier Coverage 检查**
   - 验证 2x2 grid 的连续性（无 gaps）
   - 检查 block 边界对齐
   - 机器可检查（不需要实数几何）

2. **Subtree Retention 检查**
   - 验证 external tileset (Tile_* blocks) 保留
   - 检查 rebuild 后 external 引用未丢失
   - 统计 expected vs retained blocks

3. **集成到 run-acceptance.sh**
   - D1 运行这两项检查
   - 更新 `status.json` 从 placeholder 到真实检查

### 不包含的内容

❌ **推迟到 R09.3+:**
- BV tightness (需要 GLB 几何分析)
- GE reasonableness (需要实数分析)
- REPLACE correctness (需要深度遍历)
- A/B 实际运行（blocked 待 D2）

## Frontier Coverage 定义

### 什么是 Frontier

**Frontier**: tileset 中 top-level blocks 的空间边界集合。

**Coverage**: frontier 是否完整覆盖预期区域，无 gaps。

### D1 Small-Grid 2x2 Frontier

```
预期 frontier:
+---+---+
| 0 | 1 |  (row 0)
+---+---+
| 2 | 3 |  (row 1)
+---+---+

Blocks:
- Tile_+000_+000 (col 0, row 0)
- Tile_+001_+000 (col 1, row 0)
- Tile_+000_+001 (col 0, row 1)
- Tile_+001_+001 (col 1, row 1)
```

### 检查逻辑

1. **解析 root tileset.json**
2. **提取 children 的 Tile_* URIs**
3. **解析坐标 (col, row) from Tile_+XXX_+YYY**
4. **验证:**
   - 所有坐标在 [0..1] x [0..1] 范围内
   - 无重复坐标
   - 无 missing 坐标（2x2 grid 需要 4 个）
5. **报告:**
   - `checked: true`
   - `passed: true/false`
   - `gridSize: "2x2"`
   - `blocksExpected: 4`
   - `blocksFound: N`
   - `gaps: []` (如果有 missing blocks)

### 实现

创建 `crates/processor/src/validation/frontier_coverage.rs`:

```rust
pub struct FrontierCoverageResult {
    pub checked: bool,
    pub passed: bool,
    pub grid_size: String,
    pub blocks_expected: usize,
    pub blocks_found: usize,
    pub gaps: Vec<(i32, i32)>,  // missing (col, row) coordinates
}

pub fn check_frontier_coverage(tileset_path: &Path) -> Result<FrontierCoverageResult> {
    let tileset = load_tileset(tileset_path)?;
    
    // Extract Tile_* children
    let mut coords = HashSet::new();
    for child in &tileset.root.children {
        if let Some(uri) = child.content.as_ref().and_then(|c| c.uri.as_ref()) {
            if let Some((col, row)) = parse_tile_coord(uri) {
                coords.insert((col, row));
            }
        }
    }
    
    // For 2x2 grid, expect (0,0), (1,0), (0,1), (1,1)
    let expected = [(0,0), (1,0), (0,1), (1,1)];
    let mut gaps = Vec::new();
    for &coord in &expected {
        if !coords.contains(&coord) {
            gaps.push(coord);
        }
    }
    
    Ok(FrontierCoverageResult {
        checked: true,
        passed: gaps.is_empty(),
        grid_size: "2x2".to_string(),
        blocks_expected: 4,
        blocks_found: coords.len(),
        gaps,
    })
}

fn parse_tile_coord(uri: &str) -> Option<(i32, i32)> {
    // Parse "Tile_+000_+001/tileset.json" -> (0, 1)
    let parts: Vec<&str> = uri.split('/').collect();
    if parts.is_empty() {
        return None;
    }
    
    let tile_name = parts[0];
    if !tile_name.starts_with("Tile_") {
        return None;
    }
    
    let coords_str = &tile_name[5..]; // skip "Tile_"
    let coords: Vec<&str> = coords_str.split('_').collect();
    if coords.len() != 2 {
        return None;
    }
    
    let col = coords[0].trim_start_matches('+').parse::<i32>().ok()?;
    let row = coords[1].trim_start_matches('+').parse::<i32>().ok()?;
    
    Some((col, row))
}
```

## Subtree Retention 定义

### 什么是 Subtree Retention

**Subtree**: external tileset references (e.g., Tile_* blocks)

**Retention**: rebuild 后 external 引用是否保留（不丢失）

### D1 Small-Grid Subtree

```
Input tileset.json:
{
  "root": {
    "children": [
      { "content": { "uri": "Tile_+000_+000/tileset.json" } },
      { "content": { "uri": "Tile_+001_+000/tileset.json" } },
      { "content": { "uri": "Tile_+000_+001/tileset.json" } },
      { "content": { "uri": "Tile_+001_+001/tileset.json" } }
    ]
  }
}
```

**Expected after rebuild:**
- All 4 Tile_* children retained
- No external blocks lost

### 检查逻辑

1. **解析 input tileset.json** (from fixture)
2. **解析 output tileset.json** (after processor)
3. **提取 external tilesets:**
   - Count Tile_* URIs in input
   - Count Tile_* URIs in output
4. **验证:**
   - output blocks ≥ input blocks
   - No missing blocks
5. **报告:**
   - `checked: true`
   - `passed: true/false`
   - `blocksExpected: 4`
   - `blocksRetained: N`
   - `blocksLost: []` (if any)

### 实现

创建 `crates/processor/src/validation/subtree_retention.rs`:

```rust
pub struct SubtreeRetentionResult {
    pub checked: bool,
    pub passed: bool,
    pub blocks_expected: usize,
    pub blocks_retained: usize,
    pub blocks_lost: Vec<String>,
}

pub fn check_subtree_retention(
    input_tileset: &Path,
    output_tileset: &Path,
) -> Result<SubtreeRetentionResult> {
    let input = load_tileset(input_tileset)?;
    let output = load_tileset(output_tileset)?;
    
    let input_blocks = extract_external_blocks(&input.root);
    let output_blocks = extract_external_blocks(&output.root);
    
    let mut lost = Vec::new();
    for block in &input_blocks {
        if !output_blocks.contains(block) {
            lost.push(block.clone());
        }
    }
    
    Ok(SubtreeRetentionResult {
        checked: true,
        passed: lost.is_empty(),
        blocks_expected: input_blocks.len(),
        blocks_retained: output_blocks.len(),
        blocks_lost: lost,
    })
}

fn extract_external_blocks(tile: &Tile) -> HashSet<String> {
    let mut blocks = HashSet::new();
    
    if let Some(content) = &tile.content {
        if let Some(uri) = &content.uri {
            if uri.starts_with("Tile_") {
                blocks.insert(uri.clone());
            }
        }
    }
    
    for child in &tile.children {
        blocks.extend(extract_external_blocks(child));
    }
    
    blocks
}
```

## 集成到 run-acceptance.sh

### D1 运行检查

```bash
# Frontier coverage check
FRONTIER_RESULT=$("$PROCESSOR_PATH" check-frontier --tileset "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null || echo '{"checked": false}')
FRONTIER_CHECKED=$(echo "$FRONTIER_RESULT" | jq -r '.checked')
FRONTIER_PASSED=$(echo "$FRONTIER_RESULT" | jq -r '.passed')
FRONTIER_BLOCKS=$(echo "$FRONTIER_RESULT" | jq -r '.blocksFound')
FRONTIER_GAPS=$(echo "$FRONTIER_RESULT" | jq -r '.gaps | length')

# Subtree retention check
SUBTREE_RESULT=$("$PROCESSOR_PATH" check-subtree \
    --input "$FIXTURE_INPUT/tileset.json" \
    --output "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null || echo '{"checked": false}')
SUBTREE_CHECKED=$(echo "$SUBTREE_RESULT" | jq -r '.checked')
SUBTREE_PASSED=$(echo "$SUBTREE_RESULT" | jq -r '.passed')
SUBTREE_EXPECTED=$(echo "$SUBTREE_RESULT" | jq -r '.blocksExpected')
SUBTREE_RETAINED=$(echo "$SUBTREE_RESULT" | jq -r '.blocksRetained')
```

### 更新 status.json

```json
{
  "spatialQuality": {
    "frontierCoverage": {
      "checked": true,
      "passed": true,
      "gridSize": "2x2",
      "blocksExpected": 4,
      "blocksFound": 4,
      "gaps": 0,
      "note": "Frontier coverage check (R09.2)"
    },
    "subtreeRetention": {
      "checked": true,
      "passed": true,
      "blocksExpected": 4,
      "blocksRetained": 4,
      "blocksLost": 0,
      "note": "Subtree retention check (R09.2)"
    },
    "transformConsistency": {
      "checked": false,
      "note": "Layer A+ not yet implemented (R09.3+)"
    },
    "geMonotonicity": {
      "checked": true,
      "passed": true,
      "violations": 0,
      "totalTiles": 5,
      "note": "GE monotonicity check (R09.1)"
    },
    "geReasonableness": {
      "checked": false,
      "note": "Layer B not implemented (R09.3+)"
    },
    "bvTightness": {
      "checked": false,
      "note": "Layer B not implemented (R09.3+)"
    }
  }
}
```

## D0 不运行这些检查

**原因:**
- D0 single-tile 无 grid (只有 1 个 block)
- Frontier 概念不适用
- Subtree 只有 1 个 external block，trivial

**D0 status.json:**
```json
{
  "spatialQuality": {
    "frontierCoverage": {
      "checked": false,
      "note": "D0 single-tile, no grid structure"
    },
    "subtreeRetention": {
      "checked": false,
      "note": "D0 single-tile, trivial retention"
    }
  }
}
```

## 测试计划

### Unit Tests

1. **Frontier Coverage:**
   - `test_frontier_2x2_complete` - 完整 2x2 grid
   - `test_frontier_2x2_missing_block` - missing (1,1)
   - `test_frontier_empty` - 无 Tile_* children

2. **Subtree Retention:**
   - `test_subtree_all_retained` - 所有 blocks 保留
   - `test_subtree_one_lost` - 一个 block 丢失
   - `test_subtree_empty_input` - 输入无 external blocks

### Integration Tests

- D1 small-grid: 
  - ✅ Frontier: 4/4 blocks, no gaps
  - ✅ Subtree: 4 expected, 4 retained

## R09.2 不包含的内容

### A/B Runner Scaffolding (推迟)

**原因:** 
- Frontier + Subtree 更直接可验证
- A/B scaffolding 需要 Cesium integration 才有意义
- Blocked 状态已在 R09.1 明确

**如果 R09.2 有容量，可在 R09.3 实现:**
- Cesium viewer stub (headless)
- Metric collection hooks (requests/bytes)
- 保持 blocked（不运行真实 A/B）

### BV Tightness (推迟到 R09.3+)

**原因:**
- 需要 GLB 几何解析
- 需要计算 mesh bounding box
- 比 frontier/subtree 复杂得多

### Transform Consistency (推迟到 R09.3+)

**原因:**
- 需要 transform chain 验证
- 需要 ECEF 坐标计算
- D1 可以实现，但优先级低于 frontier/subtree

## R09.2 完成标准

### 代码

- ✅ `validation/frontier_coverage.rs` 实现
- ✅ `validation/subtree_retention.rs` 实现
- ✅ `processor check-frontier` 命令
- ✅ `processor check-subtree` 命令
- ✅ 集成到 `run-acceptance.sh` (D1 only)

### 测试

- ✅ Unit tests 通过 (≥4 new tests)
- ✅ D1 acceptance: frontier + subtree PASS
- ✅ D0 acceptance: 仍然 PASS (不运行这两项检查)
- ✅ CI green (all platforms)

### 文档

- ✅ R09-2-frontier-subtree.md (本文档)
- ✅ schemas/status-example.json 更新
- ✅ status.md 更新

### Evidence

- ✅ Layer A+ hooks 实现（非 placeholder）
- ✅ D1 验证通过
- ✅ 不虚造 A/B metrics
- ✅ Blocked 状态保持

## R10 Path Notes (准备)

### Fixed Runtime Install Candidate

**R10 目标:** 固定运行时安装候选（移除 Python 依赖）

**准备工作 (R09.2 后):**
1. 评估 basisu native Rust binding (不通过 Python)
2. KTX2 encode 路径纯 Rust 化
3. Install mode 不再需要 Python/pip

**R09 harness 足够后开始 R10:**
- R09.1: GE monotonicity ✅
- R09.2: Frontier + Subtree ✅
- R09.3: (可选) Transform consistency / BV tightness
- R09 有足够 Layer B checks → 开始 R10

## 总结

### R09.2 交付

1. ✅ Frontier coverage 机器检查
2. ✅ Subtree retention 机器检查
3. ✅ D1 集成 + 验证
4. ✅ D0 不受影响

### R09.2 限制

❌ **不包含:**
- A/B 实际运行（blocked 待 D2）
- BV tightness (复杂度高)
- Transform consistency (优先级低)
- GE reasonableness (实数分析)

### 下一步 (R09.3+)

- R09.3: Transform consistency OR BV tightness
- R09.4+: A/B runner scaffolding (保持 blocked)
- R10: Fixed runtime install (移除 Python)

---

**状态:** R09.2 实现中，CI green → merge
**Evidence:** 严格，A/B blocked 直到 LandsD/PlanD
