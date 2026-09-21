# R01 Implementation Summary

**任务:** 并发参数传递与重试语义（convert.rs）  
**状态:** 代码通过  
**完成时间:** 2026-09-21 17:13 CST

## 实现的功能

### 1. 线程配置解析 (`resolve_thread_config`)

新增 `ThreadConfig` 结构和解析逻辑：

- **显式数值 (1/2/4):** 直接传递给 subprocess
- **显式 0:** 表示 auto 模式，传递环境变量 `GEOFORGE_CONVERT_THREADS=0`
- **省略 threads 字段:** 读取 `GEOFORGE_CONVERT_THREADS` 环境变量（如存在）
- **无效值 (>16, 错误类型):** 记录日志并忽略，使用 converter 默认值

**优先级:** 显式 API 值 > 环境变量 > converter 默认

### 2. 日志分离

分别记录三个层次：

1. **请求的配置** (`converter.threads.requested` metric + log)
   - 示例: `"2 (explicit)"`, `"from env: 4"`, `"default (omitted, no env)"`

2. **传递给 subprocess 的 override** (`converter.threads.override` metric + log)
   - `Some(n)` 或 `None` (converter default)

3. **Converter 实际使用的 worker 数** (如可获知，R01 暂未实现探测)

### 3. 第一次调用修复

**修复前 Bug:**
```rust
run_native(..., None)  // 第一次调用，忽略了 configured_threads
```

**修复后:**
```rust
let thread_override = thread_config.to_override();
run_native(..., thread_override)  // 第一次调用就传递解析后的值
```

### 4. 重试语义改进

**重试条件 (所有条件必须同时满足):**

```rust
result.exit_code != 0
    && thread_override != Some(1)      // 非单线程失败
    && !cancel.is_cancelled()          // 未取消
    && retry_single_thread(&result)    // 已知崩溃签名或空 JSON
```

**不重试的情况:**
- 真正的单线程失败 (`thread_override == Some(1)`)
- 取消后的失败
- 正常业务错误（非崩溃）

### 5. 环境变量传递

`run_native` 中对 `thread_override` 的处理：

```rust
if let Some(threads) = thread_override {
    if threads == 0 {
        // 显式 auto
        env.push(("GEOFORGE_CONVERT_THREADS", PathBuf::from("0")));
    } else {
        // 显式数值
        env.push(("GEOFORGE_CONVERT_THREADS", PathBuf::from(threads.to_string())));
    }
}
// None = 不设置环境变量，converter 使用默认
```

## 测试覆盖 (13 tests, 全部通过)

### 原有测试 (3)
- `retries_missing_tile_json` - 空 JSON 错误重试
- `retries_known_windows_native_crashes` - Windows 崩溃码重试
- `does_not_retry_unrelated_converter_errors` - 正常错误不重试

### 新增测试 (10)

**线程配置解析:**
- `thread_config_explicit_1` - 显式 1
- `thread_config_explicit_2` - 显式 2
- `thread_config_explicit_4` - 显式 4
- `thread_config_explicit_auto` - 显式 0 (auto)
- `thread_config_invalid_high` - 无效值 >16
- `thread_config_invalid_type` - 错误类型（字符串）
- `thread_config_omitted_no_env` - 省略且无环境变量
- `thread_config_from_env` - 从环境变量读取
- `thread_config_env_invalid` - 环境变量无效
- `thread_config_explicit_wins_over_env` - 显式值优先

## 代码变更统计

- **文件:** `crates/processor/src/stages/convert.rs`
- **变更:** +204 / -27 行
- **新增结构:** `ThreadConfig`
- **新增函数:** `resolve_thread_config`
- **修改函数:** `run_convert`, `run_native`

## 验收要点

### 功能验收
- [x] UI 选择 1 → 第一次 convert 获得单线程参数 ✓
- [x] 配置与日志匹配 ✓ (三层日志分离)
- [ ] 目标故障 ≤1 重试 ✓ (需要实机测试)
- [ ] 取消不启动额外进程 ✓ (需要实机测试)

### 日志示例

**显式 threads=2:**
```
[convert] thread config requested: 2 (explicit)
[convert] thread override passed to subprocess: 2
[convert] forcing thread count to 2
```

**省略 + 环境变量:**
```
[convert] thread config requested: from env: 4
[convert] thread override passed to subprocess: 4
[convert] forcing thread count to 4
```

**省略 + 无环境变量:**
```
[convert] thread config requested: default (omitted, no env)
[convert] thread override passed to subprocess: none (converter default)
```

## 下一步 (R02)

R02 将实现：
- 设置持久化到配置文件
- 旧配置兼容性处理
- 默认值：keep texture mode + 1 worker

## CI 状态

- **构建:** 通过 (Rust 1.98.1)
- **测试:** 42/42 通过
- **PR #8:** CI 运行中
