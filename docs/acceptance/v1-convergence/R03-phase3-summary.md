# R03.3 实施总结 - Adapter 外部 Tileset 保留

**分支:** cursor/v1-convergence-r03-3-2a7d  
**基于:** master@391ea18 (R03.2)  
**提交:** 4ccee60  
**主题:** Phase 11 P0-2 adapter.rs 外部 tileset 保留验证

## 实现内容

### 1. 测试验证 ✓

**新增测试:** `external_tileset_paths_recorded`

位置: `crates/top_rebuild/src/adapter.rs:518-605`

测试覆盖:
- 创建临时目录结构模拟真实 3D Tiles 布局
- 根 tileset.json 引用外部 Tile_+000_+000/tileset.json
- 外部 tileset 包含一个 .b3dm 内容
- 验证 `load_source_blocks` 正确记录:
  - `source_tileset_path` 指向外部 tileset.json 绝对路径
  - `source_block_dir` 指向外部 tileset 目录
  - `grid_x` 和 `grid_y` 正确解析
  - 至少加载一个 Representation

**测试结果:** ✓ 通过

### 2. 文档注释 ✓

**更新位置:** `crates/top_rebuild/src/adapter.rs:245-258`

添加了详细的文档注释说明:
- Phase 11 P0-2 设计意图
- `source_tileset_path` 和 `source_block_dir` 用途
- 与 tileset_writer 的集成点
- 保留外部 tileset 结构的目标

### 3. 依赖更新 ✓

**Cargo.toml 变更:**
```toml
[dev-dependencies]
tempfile = "3"
```

用于测试中创建临时目录结构。

## 功能状态

### ✅ 已完成 (R03.1)

adapter.rs 的核心职责已在 R03.1 中完成:
- `SourceBlock` 结构包含 `source_tileset_path` 和 `source_block_dir` 字段
- `load_source_blocks` 正确填充这些字段 (line 356-357)
- 路径是绝对路径,指向正确的外部 tileset

### ✅ 已完成 (R03.3)

- 测试验证路径记录正确性
- 文档说明设计意图和集成点
- 完整的测试覆盖

### 🔜 待完成 (R03.4+)

tileset_writer 的保留逻辑 (独立功能):
- `preserve_block_subtree()` 函数
- 复制外部 tileset.json 到输出目录
- 更新引用路径
- 多 parts 覆盖前沿选择

## 测试结果

### 单元测试
```
test adapter::tests::external_tileset_paths_recorded ... ok
```

### 回归测试
完整测试套件通过,无破坏性变更:
```
test result: ok. 46 passed; 0 failed; 0 ignored
```

## 代码统计

| 类型 | 行数 |
|------|------|
| 测试代码 | 88 |
| 文档注释 | 14 |
| Cargo 配置 | 1 |
| **总计** | **103** |

## 设计决策

### 为什么不在 R03.3 实现 tileset_writer?

1. **单一主题原则:** "one theme" - R03.3 聚焦 adapter
2. **小增量原则:** "small enough" - 保持 PR 小而专注
3. **功能独立性:** adapter 记录路径 vs tileset_writer 使用路径是两个独立职责
4. **已有基础:** R03.1 已完成核心数据结构,R03.3 只需验证

### 为什么选择验证而非实现?

adapter.rs 在 R03.1 已经正确实现了:
- 解析 Tile_+X_+Y 模式
- 记录外部 tileset 绝对路径
- 填充 source_block_dir

R03.3 的价值:
- 测试确认功能正确
- 文档说明设计意图
- 为后续 tileset_writer 集成提供清晰接口

## 后续任务

### R03.4: tileset_writer preserve_block_subtree

预计改动: ~200 行
- 实现 `preserve_block_subtree()` 函数
- 复制外部 tileset.json 结构
- 更新 uri 引用路径
- 集成到 `write_tileset` 流程

### R03.5: phase11_correctness 测试

从 feat/v1-prod-align 移植:
- `tests/phase11_correctness.rs`
- 验证 external tileset 保留完整性
- 验证 transform 不变性

## 验收标准

- [x] 测试通过
- [x] 文档完整
- [x] 无回归
- [x] 代码审查就绪

## 注意事项

### 兼容性
向后兼容: 所有测试 fixture 可以使用空 PathBuf,不影响现有测试。

### 性能
无性能影响: 只是记录路径,不增加 I/O 或计算。

### 安全性
路径验证: `load_source_blocks` 已验证外部 tileset 存在性。

---

**结论:** R03.3 成功验证 adapter.rs 已正确实现 Phase 11 P0-2 的路径记录职责,为后续 tileset_writer 集成奠定坚实基础。
