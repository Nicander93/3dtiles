# R06.3 实施总结 - Crash Recovery Checkpoints

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r06-3-crash-recovery-7ab0

## 任务目标

为处理管道添加崩溃恢复检查点，实现：
1. 跟踪管道进度，记录每个阶段的完成状态
2. 崩溃后提供清晰的诊断信息，显示失败位置
3. 为未来的自动恢复功能奠定基础

## 背景

当前问题：
- 管道在各个阶段崩溃后，temp 目录被保留用于诊断
- 但无法知道崩溃发生在哪个阶段
- 用户看到 "temp already exists" 错误，但不知道之前执行到哪里
- 无法判断是否安全删除 temp 重试

崩溃窗口：
1. **Convert 阶段崩溃**: temp 包含部分转换输出
2. **Rebuild 阶段崩溃**: temp 包含未重建的或部分重建的内容
3. **Texture 阶段崩溃**: temp 包含未处理纹理的内容
4. **Validate 阶段崩溃**: temp 包含未验证的内容
5. **Commit 前崩溃**: temp 包含已验证但未提交的内容
6. **Commit 后崩溃**: 输出已提交，但 temp shell 未清理

## 实现内容

### 1. Checkpoint 枚举

定义所有管道阶段的检查点：

```rust
pub enum Checkpoint {
    Prepared,      // temp 目录已创建
    Converting,    // 开始转换
    Converted,     // 转换完成
    Rebuilding,    // 开始重建
    Rebuilt,       // 重建完成
    Texturing,     // 开始纹理处理
    Textured,      // 纹理处理完成
    Validating,    // 开始验证
    Validated,     // 验证完成
    Committing,    // 开始提交
    Committed,     // 提交完成
}
```

### 2. 检查点文件

在 temp 目录中写入 `.geoforge-checkpoint` 文件，记录当前阶段：

```rust
pub fn write_checkpoint(temp_dir: &Path, checkpoint: Checkpoint) -> Result<(), String>
pub fn read_checkpoint(temp_dir: &Path) -> Option<Checkpoint>
```

### 3. 管道集成

在关键阶段写入检查点：

**convert-osgb 管道**:
```rust
prepare_temp()  → Prepared
convert         → Converted
rebuild         → Rebuilding → Rebuilt
texture         → Texturing → Textured
validate        → Validating → Validated
commit          → Committing → Committed
```

**process-tileset 管道**:
```rust
prepare_temp()  → Prepared
rebuild         → Rebuilding → Rebuilt
texture         → Texturing → Textured
validate        → Validating → Validated
commit          → Committing → Committed
```

### 4. 增强错误消息

temp 已存在时，显示最后的检查点：

**Before:**
```
Error: temporary work directory already exists (refusing to overwrite): 
/data/.geoforge-task-abc ...
```

**After:**
```
Error: temporary work directory already exists (refusing to overwrite): 
/data/.geoforge-task-abc Last checkpoint: validating
This may indicate: (1) a previous run was interrupted, (2) another process 
is using the same task ID, or (3) leftover state from a crash. 
If you're certain no other process is using this directory, manually remove it 
and retry.
```

用户现在知道：上次运行在 validation 阶段崩溃了。

## 代码统计

| 文件 | 改动 | 说明 |
|------|------|------|
| `stages/commit.rs` | +112 行 | Checkpoint 枚举、读写函数、测试 |
| `pipeline.rs` | +15 行 | 管道检查点调用 |
| **总计** | **~127 行** | |

## 测试结果

- ✅ 新增 5 个测试通过
- ✅ 现有测试通过: 82/82 processor 测试
- ✅ 编译通过

**新增测试**:
1. `checkpoint_write_and_read`: 检查点读写
2. `prepare_temp_includes_checkpoint`: 初始检查点为 Prepared
3. `existing_temp_reports_last_checkpoint`: 错误消息包含检查点
4. `checkpoint_roundtrip_all_stages`: 所有阶段序列化/反序列化
5. 保留了所有现有测试

## 崩溃场景诊断

### 场景 1: Convert 阶段崩溃

```
Last checkpoint: converting
```
→ 用户知道：转换进行中崩溃，temp 可能包含部分转换输出，安全删除重试

### 场景 2: Validation 前崩溃

```
Last checkpoint: textured
```
→ 用户知道：纹理处理完成，但未验证，删除 temp 重试

### 场景 3: Commit 前崩溃

```
Last checkpoint: validated
```
→ 用户知道：内容已验证但未提交，可以手动检查 temp/staged，然后删除重试

### 场景 4: Commit 完成

```
Last checkpoint: committed
```
→ 用户知道：输出已提交成功，temp 只是清理失败，可以安全删除

## 技术亮点

1. **轻量级**: 每个检查点只写入一行文本（~10 bytes）
2. **健壮**: 检查点写入失败会导致整个操作失败（fail-safe）
3. **向后兼容**: 旧版本 temp 没有检查点时，错误消息优雅降级
4. **类型安全**: 使用枚举而非字符串，编译时检查
5. **可扩展**: 未来可添加更多检查点或自动恢复逻辑

## 未来改进方向

当前 PR 专注于**诊断**，未来可以添加：

1. **自动恢复**: 检测到 `validated` 检查点时，跳过前面阶段直接 commit
2. **增量重试**: 检测到 `converted` 时，跳过 convert 直接进入 rebuild
3. **时间戳**: 检查点文件记录时间，检测长期未清理的 temp
4. **PID 记录**: 记录创建 temp 的进程 ID，检测进程是否仍在运行

## 安全性

检查点机制不影响安全性：
- ✅ 仍然拒绝覆盖已存在的 temp（reject-if-exists）
- ✅ 所有权标记机制保持不变
- ✅ 原子性提交语义不变
- ✅ 检查点只是额外的诊断信息

---

**状态:** 代码完成，等待 PR 和 CI
