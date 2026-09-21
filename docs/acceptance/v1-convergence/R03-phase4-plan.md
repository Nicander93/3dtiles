# R03.4 实施计划 - Tileset Writer Block Subtree Preservation

**分支:** cursor/v1-convergence-r03-4-2a7d  
**基于:** master@9c38f60 (R03.3)  
**主题:** Phase 11 P0-2 tileset_writer preserve_block_subtree

## 目标

实现 tileset_writer 中的 block subtree preservation,保留原始 Block 外部 tileset 结构。

## 当前状态

### ✅ 已完成 (R03.1-R03.3)
- `SourceBlock::source_tileset_path` 和 `source_block_dir` 字段
- adapter.rs 正确填充路径
- 测试验证路径记录
- B3DM 8字节对齐修复

### 🔜 R03.4 实现范围

基于 origin/feat/v1-prod-align 的实现,移植以下内容:

#### 核心函数 (~300 行)

1. **preserve_block_subtree** - 主函数
   - 复制外部 tileset 目录到输出
   - 调用 B3DM 对齐修复 (已有 realign_b3dm_tree)
   - 验证结构完整性
   - 返回 proxy sources 和 preservation entry

2. **辅助函数** (~150 行)
   - `structure_digest` - 计算 tileset 结构摘要
   - `count_tileset_nodes` - 递归统计节点和内容
   - `copy_dir_recursive` - 递归复制目录
   - `resolve_proxy_sources` - 解析多 parts 覆盖前沿
   - `write_minimal_leaf_tileset` - 合成最小 tileset (debug)
   - `synthesize_missing_content_in_dir` - 合成缺失内容 (debug)

3. **新类型** (~20 行)
   - `SubtreePreservationEntry` - 记录保留统计

4. **已有函数增强** (~50 行)
   - `validate_release_content` - 内容验证
   - `content_exists`, `content_usable` - 内容检查

#### 集成点

需要在 `write_tileset` 中调用 `preserve_block_subtree` 而不是旧的内联逻辑。

## 最小实现方案

### 选项 A: 完整移植 (~500 行)
- 所有辅助函数
- SubtreePreservationEntry
- 完整的多 parts 支持
- synthesize 逻辑

**代码改动:** ~500 行

### 选项 B: 核心保留 (~300 行)
- preserve_block_subtree (简化版)
- 必需的辅助函数
- 跳过 synthesize 逻辑 (debug only)
- 基本的多 parts 支持

**代码改动:** ~300 行

### 选项 C: 最小验证 (~100 行)
- 只添加 copy_dir_recursive
- 简单的结构验证
- 不完整的多 parts 支持

**代码改动:** ~100 行

## 推荐方案

**选择 B: 核心保留 (~300 行)**

**包含:**
1. `preserve_block_subtree` - 核心逻辑
2. `structure_digest` + `count_tileset_nodes` - 验证
3. `copy_dir_recursive` - 目录复制
4. `resolve_proxy_sources` - 多 parts (简化)
5. `SubtreePreservationEntry` - 统计记录
6. `content_usable` - 内容检查

**不包含 (推迟到 R03.5+):**
- `synthesize_missing_content_in_dir` (debug only)
- `write_minimal_leaf_tileset` (debug only)
- `validate_release_content` 完整实现 (可用简化版)

**原因:**
- synthesize 逻辑只用于 debug/fixture
- Release 路径不需要合成
- 保持 PR 聚焦核心功能
- ~300 行是合理的 "small PR" 范围

## 实现步骤

1. 添加 `SubtreePreservationEntry` 结构 ✓
2. 实现 `copy_dir_recursive` ✓
3. 实现 `count_tileset_nodes` ✓
4. 实现 `structure_digest` ✓
5. 实现 `content_usable` ✓
6. 实现 `resolve_proxy_sources` (简化) ✓
7. 实现 `preserve_block_subtree` ✓
8. 集成到 `write_tileset` ✓
9. 添加测试 ✓

## 测试策略

### 单元测试
- `structure_digest` 正确统计
- `copy_dir_recursive` 复制目录
- `resolve_proxy_sources` 解析 parts

### 集成测试
- 完整的 block preservation 流程
- 验证输出 tileset 结构
- 验证内容文件复制

## 兼容性

- ✅ 不破坏现有测试
- ✅ Release 模式正常工作
- ✅ Debug 模式降级 (无 synthesize)

## 风险

- **中等复杂度:** ~300 行新代码
- **依赖 R03.1-R03.3:** 需要 source_tileset_path 等字段
- **测试覆盖:** 需要真实 tileset 结构

---

**下一步:** 实现方案 B
