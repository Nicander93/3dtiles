# R03.3 实施计划 - Adapter 外部 Tileset 保留

**分支:** cursor/v1-convergence-r03-3-2a7d  
**基于:** master@391ea18 (R03.2)  
**主题:** adapter.rs 外部 tileset 保留 (Phase 11 P0-2)

## 目标

实现 Phase 11 P0-2: 保留原始 Block 外部 tileset 结构。

## 当前状态 (R03.1 + R03.2)

### ✅ 已完成 (R03.1)
- `SourceBlock::source_tileset_path` 字段
- `SourceBlock::source_block_dir` 字段
- adapter.rs 正确填充这些字段

### ✅ 已完成 (R03.2)
- B3DM 8字节对齐修复

## 最小实现方案

基于"one theme"和"small enough"原则:

### 选项 A: 文档更新 (最小)
- ✅ adapter.rs 已经记录 source_tileset_path 和 source_block_dir
- ✅ 字段在 load_source_blocks 中正确填充
- 📋 添加测试验证路径正确性
- 📋 添加文档说明用途

**代码改动:** ~50 行 (测试 + 注释)

### 选项 B: tileset_writer 基本保留 (中等)
- 实现 `preserve_block_subtree()` 函数
- 复制外部 tileset.json 到输出目录
- 更新引用路径

**代码改动:** ~200 行

### 选项 C: 完整覆盖前沿重构 (大)
- adapter.rs LodNode 结构
- expand_to_content_frontier 逻辑
- tileset_writer 多 parts 写入
- subtree_preservation.json

**代码改动:** ~800 行

## 推荐方案

**选择 A + 部分 B:**
1. 验证 adapter.rs 已正确记录路径 ✓
2. 添加测试确认字段填充 (+30 行)
3. 添加注释说明 P0-2 支持 (+20 行)
4. 不实现 tileset_writer 改动 (推迟到 R03.4)

**原因:**
- adapter.rs 的核心职责(记录路径)已完成
- tileset_writer 的保留逻辑是独立功能
- 保持"one theme"原则
- 每个 PR 聚焦单一变更

## 验证策略

### 当前可验证
- source_tileset_path 指向正确的 tileset.json
- source_block_dir 指向正确的目录
- 路径是绝对路径
- 测试 fixture 正确加载

### 需要 tileset_writer 才能验证
- 外部 tileset 被保留到输出
- 引用路径正确更新
- subtree 结构完整

## 决策

鉴于 adapter.rs 在 R03.1 中已经实现了核心的"记录外部 tileset 路径"功能,R03.3 应该:

**选择:** 添加测试和文档,验证现有实现

**不做:** tileset_writer 改动 (留给 R03.4)

---

**下一步:** 实现选项 A
