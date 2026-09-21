# Windows CI Blocker Fix - R01 Test Flake

**问题:** Windows CI 测试失败 - `thread_config_from_env`  
**修复时间:** 2026-09-21 17:27 CST  
**Commit:** cb307ea

## 根本原因

### 失败的测试
```
stages::convert::tests::thread_config_from_env
Expected: override_value = Some(4)
Actual: None
```

### 问题分析

**并发测试竞争条件:**

Cargo 默认并行运行测试。4 个测试同时操作环境变量 `GEOFORGE_CONVERT_THREADS`：

1. `thread_config_omitted_no_env` → `remove_var`
2. `thread_config_from_env` → `set_var("4")` → 读取
3. `thread_config_env_invalid` → `set_var("999")`
4. `thread_config_explicit_wins_over_env` → `set_var("8")`

**时间序列（失败场景）:**
```
T1: thread_config_from_env        set_var("4")
T2: thread_config_env_invalid     set_var("999")  ← 覆盖
T3: thread_config_from_env        读取 env       ← 得到 "999"
T4: thread_config_omitted_no_env  remove_var()   ← 或得到 None
```

**Windows 特殊性:**
- Windows 环境变量操作可能比 Unix 更慢
- 竞争窗口更大，更容易触发失败

### 为什么本地测试通过？

- 本地运行可能只运行单个测试
- CPU 核心少时并发度低
- 随机性：有时竞争不发生

## 修复方案

### 使用 serial_test Crate

**添加依赖 (Cargo.toml):**
```toml
[dev-dependencies]
serial_test = "3.0"
```

**标记测试为串行:**
```rust
use serial_test::serial;

#[test]
#[serial]
fn thread_config_from_env() {
    std::env::set_var("GEOFORGE_CONVERT_THREADS", "4");
    // ...
    std::env::remove_var("GEOFORGE_CONVERT_THREADS");
}
```

### 修改的测试 (4 个)

1. `thread_config_omitted_no_env` → `#[serial]`
2. `thread_config_from_env` → `#[serial]`
3. `thread_config_env_invalid` → `#[serial]`
4. `thread_config_explicit_wins_over_env` → `#[serial]`

**不影响的测试 (9 个):**
- 不使用环境变量的测试保持并行
- 重试逻辑测试不受影响

## 验证

### 本地测试 (Linux)
```bash
cargo test --package processor
```
**结果:** 42/42 passed ✓

### CI 测试
- Windows: 运行中
- Ubuntu: 运行中
- macOS: 运行中

## 技术细节

### serial_test 工作原理

```rust
#[test]
#[serial]
fn test_a() { /* ... */ }

#[test]
#[serial]
fn test_b() { /* ... */ }
```

1. `serial_test` 使用全局互斥锁
2. 标记 `#[serial]` 的测试按顺序执行
3. 未标记的测试仍然并行
4. 跨平台支持 (Windows/Linux/macOS)

### 替代方案（未采用）

**方案 A: 手动互斥锁**
```rust
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test() {
    let _guard = ENV_LOCK.lock().unwrap();
    // test code
}
```
缺点：需要手写样板代码

**方案 B: 唯一环境变量名**
```rust
std::env::set_var("GEOFORGE_TEST_1", "4");
```
缺点：不测试真实环境变量名

**方案 C: `-- --test-threads=1`**
```bash
cargo test -- --test-threads=1
```
缺点：全局串行，显著降低测试速度

### 选择 serial_test 的理由

- ✅ 最小侵入性
- ✅ 只串行化需要的测试
- ✅ 清晰的意图表达
- ✅ 社区标准解决方案
- ✅ 零运行时开销（仅 dev-dependency）

## 合约保证

**R01 合约仍然成立:**

当 `options.convert.threads` 省略时：
1. `resolve_thread_config` 读取 `GEOFORGE_CONVERT_THREADS`
2. 值 `"4"` → 解析为 `Some(4)`
3. 传递给 `run_native` 作为 `thread_override`
4. 设置为子进程环境变量

**测试现在可靠地验证:**
- ✅ 环境变量正确读取
- ✅ 解析为正确的 override 值
- ✅ 无竞争，无 flake

## 影响范围

**变更文件 (3):**
1. `crates/processor/Cargo.toml` — 添加 serial_test
2. `crates/processor/src/stages/convert.rs` — 4 个测试加 `#[serial]`
3. `Cargo.lock` — 依赖更新

**无逻辑变更:**
- ❌ `resolve_thread_config` 实现未变
- ❌ 线程配置逻辑未变
- ✅ 仅测试隔离改进

## 后续监控

### CI 绿色条件

1. **Windows tests pass** ← 本次修复目标
2. Ubuntu tests pass
3. macOS tests pass
4. Product Core pass

### 如果仍然失败

**Debug 步骤:**
1. 检查 CI 日志中的具体错误
2. 验证 serial_test 在 Windows 上正确工作
3. 考虑增加测试清理的健壮性

**额外防护（如需要）:**
```rust
#[test]
#[serial]
fn thread_config_from_env() {
    // 清理可能的残留
    std::env::remove_var("GEOFORGE_CONVERT_THREADS");
    
    std::env::set_var("GEOFORGE_CONVERT_THREADS", "4");
    let config = resolve_thread_config(&json!({"convert": {}}));
    assert_eq!(config.to_override(), Some(4));
    
    // 显式清理
    std::env::remove_var("GEOFORGE_CONVERT_THREADS");
}
```

## 提交信息

```
fix(R01): serialize env var tests to prevent Windows flakes

Root Cause: Cargo parallel tests racing on GEOFORGE_CONVERT_THREADS
Fix: serial_test crate with #[serial] on 4 env-dependent tests
Result: 42/42 tests pass, Windows CI should be green
```

## 总结

- **问题:** Windows CI 环境变量竞争导致测试 flake
- **根因:** 并行测试同时操作全局环境变量
- **修复:** 使用 `serial_test` 串行化相关测试
- **影响:** 最小 (仅测试代码)
- **验证:** 本地通过，CI 运行中
