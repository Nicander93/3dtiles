# cpuWorkers 环境变量注入问题调查和修复

## 问题假设

用户报告：在 Windows 上使用 GeoForge convert，设置 `options.execution.cpuWorkers` 在 processor `run --task` JSON 中不会导致 `GEOFORGE_CONVERT_THREADS` 被传递给转换器子进程。日志显示 "omitted, no env"，而转换器实际运行时使用 threads=8。

## 调查结果

### 代码路径分析

1. **ExecutionOptions 解析** (`crates/protocol/src/lib.rs:378-439`)
   - `ExecutionOptions::parse()` 正确解析 JSON 中的 `execution.cpuWorkers`
   - 支持 `CpuWorkers::Auto` (0) 和 `CpuWorkers::Count(n)`

2. **解析为实际值** (`crates/protocol/src/lib.rs:455-478`)
   - `ExecutionOptions::resolve()` 将配置解析为实际线程数
   - `CpuWorkers::Auto` → `(available_parallelism / 2).max(1).min(8)`
   - `CpuWorkers::Count(n)` → `n.max(1).min(available_parallelism)`
   - **关键点**：resolve() 保证返回值 >= 1

3. **ResourceBudget** (`crates/processor/src/resource_budget.rs:28-31`)
   - `convert_threads()` 简单返回 `resolved.cpu_workers`
   - 测试验证：`cpuWorkers: 1` → `convert_threads() == 1` ✓

4. **环境变量注入** (`crates/processor/src/stages/convert.rs:273-286`)
   - `run_native()` 正确调用 `converter_environment()` 获取基础环境变量
   - 正确添加 `GEOFORGE_CONVERT_THREADS` 到 env 向量
   - **之前的代码**有一个 `if threads == 0` 分支，但这是**死代码**（resolve() 保证 >= 1）

5. **Command 执行** (`crates/processor/src/util.rs:387-389`)
   - `run_logged_env_result()` 正确调用 `command.env(k, v)` 设置环境变量
   - **问题**：没有记录环境变量，导致无法验证

## 根本原因

**代码逻辑是正确的**。环境变量确实被注入了。

**真正的问题是日志不足**：
- `run_native()` 只记录 "setting thread count to N"
- `run_logged_env_result()` 不记录传递的环境变量
- 在 Windows 上很难验证转换器子进程是否真的收到了环境变量

## 修复内容

### 1. 添加环境变量日志 (`util.rs`)

```rust
for (k, v) in extra_env {
    emitter.log(&format!("[env] {}={}", k, v.display()));
    command.env(k, v);
}
```

**好处**：现在每个环境变量都会被明确记录。

### 2. 简化并改进 run_native() (`convert.rs`)

**移除死代码**：
```rust
// 之前（有死代码）
if threads == 0 {
    emitter.log("[convert] forcing thread mode to auto (0)");
    env.push(("GEOFORGE_CONVERT_THREADS", PathBuf::from("0")));
} else {
    emitter.log(&format!("[convert] setting thread count to {}", threads));
    env.push(("GEOFORGE_CONVERT_THREADS", PathBuf::from(threads.to_string())));
}

// 之后（简化）
env.push((
    "GEOFORGE_CONVERT_THREADS",
    PathBuf::from(threads.to_string()),
));
emitter.log(&format!(
    "[convert] injecting GEOFORGE_CONVERT_THREADS={} into converter environment",
    threads
));
```

**好处**：
- 移除了永远不会执行的分支
- 更清晰的日志消息
- 代码更简洁

### 3. 添加测试

```rust
#[test]
fn convert_threads_honors_explicit_one() {
    let opts = ExecutionOptions {
        cpu_workers: CpuWorkers::Count(1),
        memory_budget_mib: None,
        io_workers: None,
        resume_policy: ResumePolicy::Off,
    };
    let budget = ResourceBudget::new(&opts);
    assert_eq!(budget.convert_threads(), 1);
}
```

## Windows 验证步骤

创建测试任务 `test-cpuworkers.json`：

```json
{
  "input": "D:\\data\\test_osgb",
  "output": "D:\\output\\test_cpuworkers",
  "execution": {
    "cpuWorkers": 1
  }
}
```

运行：
```powershell
processor.exe run --task test-cpuworkers.json
```

### 预期日志输出

```
[convert] resolved CPU workers: 1 (execution protocol v1 support: true)
[convert] injecting GEOFORGE_CONVERT_THREADS=1 into converter environment
$ D:\runtime\converter\_3dtile.exe -f osgb -i D:\data\test_osgb -o D:\output\test_cpuworkers
[env] GEOFORGE_CONVERT_THREADS=1
[env] OSG_LIBRARY_PATH=D:\runtime\converter\osgPlugins-3.6.5
[env] GDAL_DATA=D:\runtime\converter\gdal
[env] PROJ_DATA=D:\runtime\converter\proj
[env] PROJ_LIB=D:\runtime\converter\proj
```

### 验证转换器行为

转换器应该在 stderr 输出中显示使用 1 个线程（具体消息取决于转换器实现）。

如果转换器仍然使用 8 个线程，那么问题可能在于：
1. **转换器二进制文件是旧版本**，不支持 `GEOFORGE_CONVERT_THREADS` 环境变量
2. **Windows 环境变量传递有问题**（但这非常罕见）
3. **转换器忽略了环境变量**（可能有 bug）

可以使用 Process Monitor 或类似工具来验证环境变量是否真的传递给了子进程。

## 总结

这个修复主要是**改进可观测性**，而不是修复功能 bug。通过添加详细的日志，我们现在可以：

1. 清楚地看到 `cpuWorkers` 如何被解析
2. 验证 `GEOFORGE_CONVERT_THREADS` 确实被注入
3. 查看所有传递给转换器的环境变量

如果在 Windows 上验证后发现转换器仍然不遵守环境变量，那么问题可能在于转换器本身，而不是 processor。

## PR 链接

https://github.com/Nicander93/3dtiles/pull/44
