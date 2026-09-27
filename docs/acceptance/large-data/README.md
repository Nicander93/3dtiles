# 大规模数据处理基线与性能测量

此目录包含 GeoForge 大规模倾斜摄影数据处理的基线版本记录、性能测量工具和验收标准。

## 文件结构

```
docs/acceptance/large-data/
├── README.md                    # 本文件
├── baseline-version.md          # 代码基线和环境配置记录
├── benchmark-schema.json        # JSON 结果格式 schema
├── csv-schema.md                # CSV 结果格式说明
├── results.csv                  # 性能基线数据（自动生成）
└── samples/                     # 样本数据元信息（不包含实际数据）
    └── README.md

scripts/
├── benchmark-large-dataset.sh   # 基准测试执行脚本
└── (future scripts...)
```

## 快速开始

### 查看当前基线

```bash
cat docs/acceptance/large-data/baseline-version.md
```

此文档记录了：
- 主程序和转换器的 git 提交 SHA
- 转换器运行时版本和配置
- 操作系统和硬件配置
- 当前并行配置和资源控制状态

### 运行基准测试

**重要**: P0 阶段只提供脚本框架和 schema 定义。实际性能测量需要样本数据和真实执行。

```bash
# 查看帮助
./scripts/benchmark-large-dataset.sh --help

# 空运行（无样本数据）- 生成 UNMEASURED 结果
./scripts/benchmark-large-dataset.sh -i /path/to/dummy/Data -s dummy-sample --dry-run

# 真实样本运行（需要真实 OSGB 数据）
./scripts/benchmark-large-dataset.sh \
    -i /path/to/real/osgb/Data \
    -s small-test-1gb \
    -t 4 \
    -m keep \
    -o ./benchmark-results/run-001

# 对比不同线程数
for threads in 1 2 4 8; do
    ./scripts/benchmark-large-dataset.sh \
        -i /path/to/osgb/Data \
        -s my-sample \
        -t $threads \
        -o ./benchmark-results/threads-$threads
done
```

### 结果文件

每次运行生成：

1. **JSON 结果** (`result.json`): 完整的结构化数据，符合 `benchmark-schema.json`
2. **CSV 追加**: 自动追加到 `docs/acceptance/large-data/results.csv`

JSON 格式包含：
- 环境配置（OS、CPU、内存）
- 代码版本（双仓 commit SHA）
- 配置参数（线程数、纹理模式）
- 样本信息（大小、Block 数）
- 详细结果：
  - 各阶段耗时
  - 峰值内存（进程树）
  - CPU 利用率
  - I/O 统计
  - Block 成功/失败数
  - 最慢 Block 和最大 Block

### 查看历史结果

```bash
# 查看所有基线数据
cat docs/acceptance/large-data/results.csv

# 用 Excel 打开分析
# Windows: start excel docs/acceptance/large-data/results.csv
# Mac: open -a "Microsoft Excel" docs/acceptance/large-data/results.csv
# Linux: libreoffice --calc docs/acceptance/large-data/results.csv
```

## P0 状态：UNMEASURED

### 已完成

- [x] 记录双仓 SHA 和代码基线
- [x] 记录转换器版本（v0.2.2）和运行时配置
- [x] 记录当前操作系统和硬件配置（VM 环境）
- [x] 定义性能测量指标和 schema（JSON 和 CSV）
- [x] 创建 benchmark 脚本框架
- [x] 明确内存指标口径（Windows/Linux 差异）
- [x] 明确标记 UNMEASURED 状态

### 待补充（需要资源）

- [ ] **样本数据**: 获取 1-5 GB、20-50 GB、100-200 GB 真实 OSGB 样本
- [ ] **Windows 环境**: 在目标 Windows 桌面环境运行完整测量
- [ ] **实际执行**: 集成 processor 执行，采集真实性能数据
- [ ] **多线程对比**: 测量 threads=1/2/4/8 的性能差异
- [ ] **纹理模式对比**: 测量 keep vs ktx2 的性能差异
- [ ] **瓶颈识别**: 确定 CPU/内存/I/O 哪个是主要瓶颈

## 测量指标说明

### 阶段耗时

| 阶段 | 说明 |
|------|------|
| Scan | 扫描 OSGB 目录，建立 Block 清单 |
| Convert | 转换 OSGB → 3D Tiles (Block 并行) |
| Rebuild | 顶层重建，生成 Proxy HLOD |
| Texture | 纹理后处理（KTX2 压缩） |
| Validate | 校验结构、坐标、覆盖、纹理 |
| Commit | 提交最终成果 |

### 内存峰值

**重要**: 多进程场景下，必须采集**同一时刻**所有子进程的内存之和。

- **Windows**: 推荐 Process Explorer 的 "Private Bytes" 或 PerfMon
- **Linux**: 推荐 `smem -t -k` 或遍历 `/proc/[pid]/status` 的 VmRSS

不能把各子进程在不同时刻的峰值直接相加。

### CPU 利用率

平均 CPU 占用率 = (所有进程的 CPU 时间总和) / (wall-clock 时间 × CPU 核心数) × 100%

- 单核 100% = 完全占用一个核心
- 多核场景下，最高可达 (核心数 × 100%)

### Block 统计

- **总数**: 扫描发现的 Block 数量
- **成功**: 完整转换成功的 Block
- **失败**: 转换失败的 Block（记录原因）
- **最慢 Block**: 耗时最长的单个 Block（用于识别异常）
- **最大 Block**: 磁盘占用最大的 Block（用于识别内存风险）

## 样本数据要求

### 不提交样本数据

**重要**: 真实倾斜摄影数据通常几十到上百 GB，不能提交到 Git。

只记录样本的**元信息**：
- 名称和描述
- 来源（项目、地区）
- 大小（MB/GB）
- Block 数量
- 纹理数量和最大分辨率
- 是否稀疏网格

元信息放在 `docs/acceptance/large-data/samples/` 目录。

### 样本获取

如果有真实样本数据，放置在项目外的目录：

```
/data/
├── osgb-samples/
│   ├── small-1gb/
│   │   └── Data/
│   ├── medium-20gb/
│   │   └── Data/
│   └── large-100gb/
│       └── Data/
```

然后在 benchmark 脚本中引用：

```bash
./scripts/benchmark-large-dataset.sh \
    -i /data/osgb-samples/small-1gb/Data \
    -s small-1gb
```

### 合成测试数据

对于自动化测试，可以使用合成的元数据：

```bash
# 生成 1000 个空 Block 目录用于元数据测试
./scripts/generate-synthetic-blocks.sh 1000 /tmp/synthetic-osgb/Data
```

这只能测试**元数据处理**（扫描、网格校验、清单管理），不能测试**实际转换**性能。

## Schema 版本

当前 schema 版本: **1.0.0**

### JSON Schema

完整定义见 `benchmark-schema.json`，包含：
- 所有字段的类型、约束、枚举值
- 必填字段和可选字段
- 嵌套对象结构
- 描述和示例

使用 JSON schema 验证器验证结果：

```bash
# 使用 ajv-cli 验证
npm install -g ajv-cli
ajv validate -s docs/acceptance/large-data/benchmark-schema.json \
             -d benchmark-results/run-001/result.json
```

### CSV Schema

说明见 `csv-schema.md`，包含：
- 列名、类型、必填状态
- 列顺序（用于 Excel 分析）
- UNMEASURED 标记规则
- 示例 CSV

## 后续计划

### P1: 统一资源参数

- 引入 `ExecutionOptions`（`execution.cpuWorkers`, `execution.memoryBudgetMiB`）
- 修正 C++ 线程安全小缺口
- 更新 benchmark 脚本支持新参数

### P2: 流式结果收取

- Block 原子提交
- 边转换边落盘
- 减少内存累计

### P3: 恢复机制

- 工作清单持久化
- 失败后只重跑未完成 Block
- 更新 benchmark 脚本支持恢复测量

### P4-P8

见 [执行计划](../../../uploads/geoforge-large-data-plan-v1_19a4.md)。

每个阶段完成后，重新运行基准测试，更新基线数据。

## 验收标准

P0 验收通过条件：

1. **文档完整**
   - [x] baseline-version.md 记录代码和环境
   - [x] benchmark-schema.json 定义完整
   - [x] csv-schema.md 说明清晰

2. **脚本可用**
   - [x] benchmark-large-dataset.sh 可执行
   - [x] 支持必要的参数（input, threads, texture-mode）
   - [x] 生成符合 schema 的 JSON 和 CSV
   - [x] 明确标记 UNMEASURED 状态

3. **无假数据**
   - [x] 没有样本时不生成估算成绩
   - [x] UNMEASURED 字段留空或明确标记

4. **测试兼容**
   - [ ] 现有回归测试仍然通过（待验证）

## 相关文档

- [执行计划](../../../uploads/geoforge-large-data-plan-v1_19a4.md): 完整的 P0-P8 实施计划
- [仓库目录结构](../../REPOSITORY_LAYOUT.md): 项目结构说明
- [V1 补齐进度](../../product/V1_COMPLETION_REPORT.md): 当前开发状态
- [转换器依赖](../../dependencies/3dtiles-converter.md): 转换器集成说明
- [验收测试框架](../v1-convergence/README.md): 现有验收测试

## 问题和反馈

如果运行 benchmark 遇到问题：

1. 检查 `baseline-version.md` 确认环境要求
2. 确认样本数据路径和格式正确
3. 查看 benchmark 输出的 `result.json` 中的 `failures` 数组
4. 查看 processor 日志（如果有）

将问题报告到项目 issue tracker，附上：
- 完整的 `result.json`
- 样本元信息（不要上传样本数据本身）
- 系统环境信息
