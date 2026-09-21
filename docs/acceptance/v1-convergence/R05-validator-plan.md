# R05 Validator 对比与实施计划

**分析日期:** 2026-09-21  
**Master:** validate.rs (26765 行, 复杂实现)  
**Feat:** validator.rs (927 行, 新 Layer A/B 实现)

## 对比概要

### Master validate.rs (当前)
- 26765 行代码
- 复杂的验证逻辑
- 可能包含冗余或旧代码
- 需要简化和整合

### Feat validator.rs (目标)
- 927 行代码
- 清晰的 Layer A/B 分层
- 稳定错误码 (ValidationCode enum)
- 规范化报告 (ValidationReport struct)

## 核心改进点

1. **稳定错误码体系**
   - ValidationCode enum (18 种错误类型)
   - SCREAMING_SNAKE_CASE 命名
   - 统一错误处理

2. **Layer A 结构检查**
   - tileset.json 存在和解析
   - asset/root/geometricError 必需字段
   - boundingVolume 结构
   - transform 有效性
   - content URI 解析和路径转义
   - cycle 检测

3. **Layer B 内容验证**
   - 文件存在性
   - B3DM/GLB magic 和头部
   - 最小文件大小
   - GLB 提取和解析

## R05 实施策略

### 第一个小 PR: ValidationCode + ValidationReport 结构

**范围:**
1. 创建 `crates/processor/src/validator.rs` 基础文件
2. 定义 ValidationCode enum (18 种错误)
3. 定义 ValidationReport 结构
4. 定义 ValidationIssue 结构
5. 添加基础导出到 lib.rs

**代码量:** ~150 行
**影响:** 无破坏性,纯新增

**不包含:**
- 完整验证逻辑
- 与现有 validate.rs 集成
- 测试 (推迟到后续 PR)

### 后续 PR 计划

1. **R05.2:** Layer A 基础检查
   - tileset.json 解析
   - asset/root 验证
   - ~200 行

2. **R05.3:** Layer A URI 和路径
   - content URI 解析
   - 路径转义检查
   - cycle 检测
   - ~200 行

3. **R05.4:** Layer B 内容检查
   - 文件存在性
   - B3DM/GLB 头部
   - ~150 行

4. **R05.5:** 集成到 processor
   - 替换现有 validate.rs 调用
   - 向后兼容
   - ~100 行

5. **R05.6:** 测试和文档
   - 单元测试
   - 集成测试
   - ~300 行

## 技术依赖

- ✅ serde/serde_json (已有)
- ✅ std::collections::HashSet (cycle 检测)
- ✅ std::path (URI 解析)
- ❌ 新依赖: 无

## 风险评估

**低风险:**
- 新文件,不影响现有代码
- 增量集成,可逐步验证
- 稳定错误码,向后兼容

**中等风险:**
- 最终需要替换现有 validate.rs (26K 行)
- 需要仔细测试所有边缘情况

## 下一步: R05.1 实施

创建基础结构 PR:
1. 新建 validator.rs
2. ValidationCode enum
3. ValidationReport/ValidationIssue struct
4. lib.rs 导出
5. 编译验证

**预计代码量:** ~150 行  
**预计 CI 时间:** ~5 分钟  
**风险:** 极低 (纯新增,不影响现有功能)

---

**总结:** R05 将分 6 个小 PR 完成 validator 整合,首个 PR 只建立基础结构。
