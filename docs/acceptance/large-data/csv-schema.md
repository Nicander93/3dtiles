# Benchmark CSV Schema

CSV 格式用于简单的表格化基线对比，每行代表一次完整的 benchmark 运行。

## 文件格式

文件名建议格式：`benchmark-results-YYYYMMDD.csv`

编码：UTF-8 with BOM（确保 Excel 正确识别）

分隔符：逗号 `,`

## 列定义

### 基础信息

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `timestamp` | ISO 8601 datetime | 是 | 测试时间 | `2026-09-27T15:30:00Z` |
| `os` | string | 是 | 操作系统 | `Windows 11`, `Linux 6.12.94+` |
| `cpu` | string | 是 | CPU 型号 | `Intel(R) Xeon(R) Processor` |
| `cpu_cores` | integer | 否 | 物理核心数 | `8` |
| `memory_mb` | integer | 是 | 系统内存 MB | `16384` |
| `disk_type` | string | 是 | 磁盘类型 | `SSD`, `HDD`, `NVMe`, `Unknown` |

### 代码版本

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `main_commit` | string | 是 | 主仓库提交 SHA | `2cd60d8` |
| `main_branch` | string | 否 | 主仓库分支 | `master` |
| `converter_version` | string | 是 | 转换器版本 | `v0.2.2` |
| `converter_commit` | string | 是 | 转换器提交 SHA | `c1400d5` |

### 配置

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `threads` | integer | 是 | 线程数（0=自动） | `4` |
| `thread_source` | string | 是 | 线程数来源 | `explicit`, `env`, `default` |
| `texture_mode` | string | 是 | 纹理模式 | `keep`, `ktx2`, `both` |

### 样本数据

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `sample_name` | string | 是 | 样本名称 | `small-test-1gb` |
| `sample_type` | string | 是 | 数据类型 | `osgb`, `fbx`, `obj` |
| `input_size_mb` | number | 否 | 输入大小 MB | `1024.5` |
| `block_count` | integer | 否 | Block 数量 | `25` |
| `max_block_size_mb` | number | 否 | 最大 Block MB | `128.3` |

### 执行状态

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `status` | string | 是 | 执行状态 | `success`, `partial_success`, `failed`, `cancelled` |
| `blocks_succeeded` | integer | 否 | 成功 Block 数 | `24` |
| `blocks_failed` | integer | 否 | 失败 Block 数 | `1` |

### 计时（秒）

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `total_seconds` | number | 是 | 总耗时 | `3600.5` |
| `scan_seconds` | number | 否 | 扫描耗时 | `5.2` |
| `convert_seconds` | number | 否 | 转换耗时 | `2800.3` |
| `rebuild_seconds` | number | 否 | 重建耗时 | `450.1` |
| `texture_seconds` | number | 否 | 纹理处理耗时 | `320.8` |
| `validate_seconds` | number | 否 | 校验耗时 | `15.3` |
| `commit_seconds` | number | 否 | 提交耗时 | `8.8` |

### 内存（MB）

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `memory_method` | string | 是 | 测量方法 | `windows_private_bytes`, `linux_rss` |
| `peak_total_mb` | number | 否 | 进程树峰值内存 | `4096.5` |
| `peak_main_mb` | number | 否 | 主进程峰值 | `512.2` |
| `peak_converter_mb` | number | 否 | 转换器峰值 | `3500.8` |

### CPU 利用率（%）

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `cpu_avg_percent` | number | 否 | 平均 CPU 利用率 | `75.5` |
| `cpu_convert_percent` | number | 否 | 转换阶段 CPU | `85.2` |

### I/O

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `read_mb` | number | 否 | 读取数据 MB | `10240` |
| `write_mb` | number | 否 | 写入数据 MB | `8192` |
| `temp_peak_mb` | number | 否 | 临时盘峰值 MB | `15360` |
| `output_size_mb` | number | 否 | 输出大小 MB | `7680` |

### 备注

| 列名 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `notes` | string | 否 | 备注信息 | `One block failed due to corrupted texture` |

## 示例 CSV

```csv
timestamp,os,cpu,cpu_cores,memory_mb,disk_type,main_commit,converter_version,threads,texture_mode,sample_name,sample_type,input_size_mb,block_count,status,total_seconds,convert_seconds,rebuild_seconds,peak_total_mb,notes
2026-09-27T15:30:00Z,Windows 11,Intel i9-12900K,16,32768,NVMe,2cd60d8,v0.2.2,4,keep,small-test,osgb,1024,25,success,3600.5,2800.3,450.1,4096.5,Baseline measurement
2026-09-27T16:45:00Z,Windows 11,Intel i9-12900K,16,32768,NVMe,2cd60d8,v0.2.2,8,keep,small-test,osgb,1024,25,success,2100.2,1500.8,320.5,7200.3,Increased threads
```

## 使用说明

### 导出为 CSV

使用提供的 `scripts/benchmark-large-dataset.sh` 脚本，它会：
1. 从 JSON 结果文件提取数据
2. 按 CSV schema 格式化
3. 追加到现有 CSV 文件（如果存在）
4. 保持列顺序一致

### Excel 分析

1. 用 Excel 打开 CSV 文件
2. 创建数据透视表分析不同配置的性能
3. 按 `threads` 分组对比加速比
4. 按 `sample_name` 对比不同数据集

### 自动化对比

```bash
# 对比两次运行
scripts/benchmark-compare.sh results1.json results2.json

# 生成趋势报告
scripts/benchmark-trend.sh benchmark-results-*.csv
```

## 数据完整性

### UNMEASURED 标记

如果某个指标无法测量，在 CSV 中：
- 数值列：留空
- 字符串列：填写 `UNMEASURED`
- 备注列：说明原因

示例：
```csv
timestamp,sample_name,peak_total_mb,notes
2026-09-27T15:30:00Z,no-sample,,UNMEASURED - No sample data available
```

### 必填列验证

`scripts/benchmark-validate.sh` 会检查：
- 所有必填列是否存在
- 数值类型是否正确
- 枚举值是否合法
- 不一致的数据（如 `blocks_succeeded + blocks_failed ≠ block_count`）

## 版本历史

- **1.0.0** (2026-09-27): 初始 schema，P0 基线版本
