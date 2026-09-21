# R06.1 实施总结 - TempGuard Late-Cancel Semantics

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r06-1-tempguard-late-cancel-7ab0

## 任务目标

澄清 TempGuard 和 commit 的语义，确保：
1. 成功提交后，迟到的取消不会重写输出
2. 取消前失败保留诊断目录
3. 原子性保证明确

## 背景问题

当前的 `commit_rename` + `TempGuard` 实现存在时序问题：

```rust
// OLD: 可能的竞态
commit::commit_rename(emitter, &temp, &output)?;  // 成功
// <-- 如果取消在这里到达
temp_guard.mark_committed();  // 未执行
// TempGuard drop 时会尝试删除 temp（但已被重命名）
```

虽然实际上安全（temp 已被重命名，删除不存在的路径是 no-op），但语义不清晰。

## 实现内容

### 1. 修改 `commit_rename` 签名

接受可选的 `TempGuard` 参数，在原子 rename 成功后立即标记：

```rust
pub fn commit_rename(
    emitter: &Emitter,
    temp_dir: &Path,
    final_output: &Path,
    temp_guard: Option<&mut TempGuard<'_>>,  // 新增
) -> Result<(), String>
```

### 2. 确保原子性顺序

```rust
// 1. Atomic rename: once this succeeds, output is committed
path_policy::rename_no_replace(temp_dir, final_output)?;

// 2. Mark committed IMMEDIATELY after successful rename
if let Some(guard) = temp_guard {
    guard.mark_committed();
}

// 3. Best-effort manifest write (non-critical)
let manifest = final_output.join(".geoforge-manifest.json");
let _ = fs::write(&manifest, ...);
```

**关键改进:**
- Rename 成功后立即标记 committed（无窗口）
- Manifest 写入移到标记之后（非关键，失败不影响提交）
- 迟到取消无法影响已提交的输出

### 3. 增强文档

为 `TempGuard` 添加详细的语义说明：

```rust
/// # Semantics
///
/// - **Cancel before commit**: cleanup removes temp directory if cancellation is detected
/// - **Success after commit**: cleanup is suppressed by `mark_committed()`; output remains
/// - **Late cancel after commit**: even if cancel arrives after `commit_rename` succeeds
///   but before `mark_committed()`, the temp dir has already been renamed, so cleanup
///   attempts to remove a non-existent path (safe no-op)
///
/// The guard does NOT clean up on normal success without cancellation, allowing
/// diagnostic inspection of temp dirs when processing fails naturally.
```

### 4. 新增测试

`late_cancel_after_commit_does_not_remove_output`:
- 模拟成功提交（rename + mark）
- 迟到取消到达
- 验证输出未被删除

## 代码统计

| 文件 | 改动 | 说明 |
|------|------|------|
| `stages/commit.rs` | +43 行 | 修改签名、顺序、文档、测试 |
| `pipeline.rs` | -4 行 | 传递 temp_guard，移除冗余 mark_committed |
| **总计** | **~47 行** | |

## 测试结果

- ✅ 新增测试通过: `late_cancel_after_commit_does_not_remove_output`
- ✅ 现有测试通过: 75/75 processor 测试
- ✅ 编译通过

## 语义保证

### 场景 1: 正常成功（无取消）
```
prepare_temp() → work → commit_rename() → output exists
                                       ↓ mark_committed
TempGuard drop: 已标记，不删除
结果: temp 消失，output 存在 ✅
```

### 场景 2: 失败（无取消）
```
prepare_temp() → work → error!
TempGuard drop: 未取消，不删除
结果: temp 保留用于诊断 ✅
```

### 场景 3: 取消前失败
```
prepare_temp() → work → cancel arrives → error
TempGuard drop: 已取消且未提交，删除 temp
结果: temp 被清理 ✅
```

### 场景 4: 提交后迟到取消
```
prepare_temp() → work → commit_rename() → cancel arrives (late)
                                       ↓ atomic rename + immediate mark
TempGuard drop: 已标记，不删除
结果: output 安全保留 ✅
```

## 技术亮点

1. **原子性窗口最小化**: rename 和 mark_committed 紧邻，无其他操作
2. **非关键操作后置**: manifest 写入失败不影响提交
3. **清晰的文档**: 所有场景的预期行为都有文档说明
4. **向后兼容**: 调用方只需传递 `Some(&mut guard)` 即可

## R06 后续计划

R06 的其他主题（每个独立 PR）：
- R06.2: 独占 temp dir 创建增强（已有基础，可能不需要）
- R06.3: 所有权标记验证增强
- R06.4: rename_no_replace Windows/Unix 行为统一
- R06.5: cancel-cleanup 压力测试

## 下一步

- [ ] CI 验证
- [ ] Code review
- [ ] 合并到 master
- [ ] 开始 R06.2（或根据需要调整）

---

**状态:** 代码完成，等待 PR 和 CI
