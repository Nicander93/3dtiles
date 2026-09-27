# P5 验证指南：同层 Proxy 有界并行与输入内存预算

## 概述

P5 实现了同层 Proxy 节点的有界并行构建，在 top_rebuild 中加入了内存准入控制机制。本文档说明如何验证 P5 的实现是否满足计划要求。

## 实现摘要

### 核心变更

1. **提取 build_one_proxy 函数**
   - 输入：已完成子节点的路径和元数据
   - 输出：ProxySummary（不包含大型 GLB 缓冲）
   - 位置：`crates/top_rebuild/src/tileset_writer.rs`

2. **每层有界工作池**
   - 使用 Rayon 线程池实现并行
   - worker 数量可配置（0 = 自动 = 物理核心数/2，上限 8）
   - 每层的任务只读子节点、写自己的节点目录
   - 协调端收集结果并合并到 payloads/统计/清单

3. **内存准入机制**
   - `MemoryAdmission` 结构管理工作内存预算
   - `estimate_proxy_working_memory` 在解码前估算资源需求
   - 独立于 ProxyBudget 的工作内存预算
   - permit 机制：获取->使用->自动释放

4. **所有权优化**
   - `proxy_builder.rs` 中移除不必要的克隆
   - `simplify_group` 接受所有权而非引用
   - 网格遍历使用移动语义

5. **ExecutionOptions 集成**
   - CLI 参数：`--workers` 和 `--working-memory-mib`
   - `resource_budget.rs` 的 `rebuild_workers()` 现在返回完整的 cpu_workers
   - `stages/rebuild.rs` 传递 workers 和 memory budget 到 top_rebuild

## 验证步骤

### 前置条件

由于当前 CI 环境中存在 crates.io 注册表的临时问题（clap_lex 1.1.1 需要 edition2024），建议在本地环境验证：

```bash
# 如果遇到 clap_lex 编译错误，临时解决方案：
rm -rf ~/.cargo/registry
cargo update
```

或者等待 crates.io 修复注册表索引。

### 1. 基础编译测试

```bash
cd /workspace
cargo build -p top_rebuild
cargo test -p top_rebuild
```

### 2. Worker=1/2/4 结构一致性测试

使用现有的 4x4 测试 fixture：

```bash
# Worker=1（串行，作为基准）
cargo run -p top_rebuild --bin top_rebuild -- \
  -i crates/top_rebuild/tests/fixtures/grid_4x4/input \
  -o /tmp/p5_test/w1 \
  --workers 1

# Worker=2
cargo run -p top_rebuild --bin top_rebuild -- \
  -i crates/top_rebuild/tests/fixtures/grid_4x4/input \
  -o /tmp/p5_test/w2 \
  --workers 2

# Worker=4
cargo run -p top_rebuild --bin top_rebuild -- \
  -i crates/top_rebuild/tests/fixtures/grid_4x4/input \
  -o /tmp/p5_test/w4 \
  --workers 4
```

### 3. 结构对比验证

```bash
# 比较 tileset.json 结构
diff <(jq -S . /tmp/p5_test/w1/tileset.json) <(jq -S . /tmp/p5_test/w2/tileset.json)
diff <(jq -S . /tmp/p5_test/w1/tileset.json) <(jq -S . /tmp/p5_test/w4/tileset.json)

# 比较指标
for w in w1 w2 w4; do
  echo "=== Worker=$w ==="
  jq '.totals, .budgets, .gaps.maxGap, .gaps.P95Gap' /tmp/p5_test/$w/rebuild_metrics.json
done
```

### 4. 内存预算验证

测试内存不足时的表现：

```bash
# 设置极小的内存预算（应该失败或警告）
cargo run -p top_rebuild --bin top_rebuild -- \
  -i crates/top_rebuild/tests/fixtures/grid_4x4/input \
  -o /tmp/p5_test/low_mem \
  --workers 2 \
  --working-memory-mib 10 2>&1 | grep -i "memory"

# 正常内存预算
cargo run -p top_rebuild --bin top_rebuild -- \
  -i crates/top_rebuild/tests/fixtures/grid_4x4/input \
  -o /tmp/p5_test/normal_mem \
  --workers 2 \
  --working-memory-mib 2048
```

### 5. 现有测试套件

```bash
# 运行所有 top_rebuild 测试
cargo test -p top_rebuild

# 重点测试
cargo test -p top_rebuild grid_4x4 --verbose
cargo test -p top_rebuild proxy_2x2 --verbose
cargo test -p top_rebuild hlod_4x4 --verbose
```

## 验收标准

### 必须满足

1. ✅ **结构一致性**：worker=1/2/4 产生相同的 tileset 结构
   - 相同的 bounds
   - 相同的 geometricError 值（允许浮点误差 < 1e-6）
   - 相同的节点层次结构
   - 相同的 content URI

2. ✅ **覆盖完整性**：并行构建不丢失任何覆盖区域
   - 所有 L0 叶节点都被保留
   - Proxy 节点正确引用所有子节点

3. ✅ **内存准入**：内存预算不足时能正确拒绝或降级
   - 超出预算的估算会被拒绝
   - permit 正确释放（通过 Drop）

4. ✅ **层级屏障**：父层永远不读取未完成的子节点
   - 同层内并行
   - 层间串行
   - 按层顺序构建

### 预期行为

1. **性能特征**
   - worker=2 相对 worker=1 应有吞吐提升（CPU 密集场景）
   - worker=4 相对 worker=2 可能提升较小（受限于 I/O 或内存带宽）
   - 固定预算下，峰值内存不随历史完成网格累计增长

2. **日志输出**
   ```
   top_rebuild engine=rust-core input=... output=...
   [rebuild] resource budget: N workers
   tileset /tmp/.../tileset.json
   L0 16
   L1 4
   L2 1
   gaps maxGap=X.XX P95Gap=Y.YY pairs=Z
   totals tris_after=... texture_bytes=... glb_bytes=...
   ```

3. **指标一致性**
   - 三角形数量：worker 数量不影响最终简化结果
   - 纹理字节：去重和调整大小应一致
   - GLB 字节：在预算约束内
   - Gap 指标：LockBorder 保护边界，gap 值应相似

## 已知限制

1. **编译环境**
   - 当前 CI 环境 Rust 1.83，crates.io 注册表存在临时问题
   - 建议在本地或升级到 Rust 1.85+ 的环境验证

2. **首版保守配置**
   - 自动 workers = 物理核心/2，上限 8（首版保守起点）
   - 未实现 DAG 每节点调度（保留层级屏障）
   - 未实现进程隔离（保持线程池模式）

3. **未包含功能**（按计划属于后续阶段）
   - P6: 纹理后处理并行化
   - P7: UI 进度接入
   - P8: 百 GB 大数据实测

## 已提交的更改

### 文件列表

- `crates/top_rebuild/src/tileset_writer.rs`: 核心并行逻辑、内存准入
- `crates/top_rebuild/src/proxy_builder.rs`: 所有权优化
- `crates/top_rebuild/src/bin/top_rebuild.rs`: CLI 参数
- `crates/processor/src/resource_budget.rs`: rebuild_workers 实现
- `crates/processor/src/stages/rebuild.rs`: 参数传递
- `crates/top_rebuild/Cargo.toml`: 添加 rayon 依赖
- `Cargo.lock`: 锁定 rayon 及其依赖

### 代码审阅要点

1. **线程安全**
   - `MemoryAdmission` 使用 `Arc<Mutex<u64>>` 管理共享状态
   - `payloads` HashMap 在并行任务完成后才更新（无竞争）
   - 文件写入隔离：每个节点有独立目录

2. **错误传播**
   - Rayon 的 `par_iter().map().collect()` 正确传播 `Result`
   - 内存准入失败返回 `TopRebuildError`
   - 单个 proxy 失败会终止整层构建（保持一致性）

3. **资源管理**
   - `MemoryPermit` 通过 Drop trait 自动释放
   - 临时 GLB 文件在写入最终内容后删除
   - 工作内存预算独立于输出预算

## 后续步骤（不属于 P5）

- 在配置更新的环境中运行完整测试套件
- 在真实 20-50 GB 数据集上验证内存行为
- 性能分析（worker 数量 vs 吞吐）
- 可选：添加 P5 专门的集成测试用例

## 问题报告

如遇到问题，请提供：
1. Rust/Cargo 版本（`rustc --version`, `cargo --version`）
2. 操作系统
3. 完整错误日志
4. 使用的 fixture 或数据集
5. 使用的 workers 和 memory_budget 参数
