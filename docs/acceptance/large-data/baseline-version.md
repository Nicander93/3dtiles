# 大规模数据处理基线版本记录

此文档记录 GeoForge 大规模倾斜摄影数据处理的代码基线、运行环境和性能测量起点。

## 代码基线

### 主程序仓库 (Nicander93/3dtiles)

- **仓库**: https://github.com/Nicander93/3dtiles
- **基线分支**: master
- **基线提交**: `2cd60d8` (Merge PR#33 feat/model-conversion-v1-integration)
- **包含关键提交**: `574595e798a85cccbcfdb733db4e7371c35e9fb8`
- **日期**: 2026-09-27

关键能力：
- Block 级并行转换支持
- 顶层重建 (top_rebuild)
- 阶段 checkpoint
- 进程树取消
- 受保护临时目录
- 特定失败条件下的单线程重试

### 转换器仓库 (Nicander93/geoforge-converter)

- **仓库**: https://github.com/Nicander93/geoforge-converter
- **基线分支**: feat/model-conversion-v1
- **基线提交**: `c1400d5068e9d4e8354d529e0c867aeb11b6ca08`
- **标签**: v0.2.2
- **日期**: 2026-09-27

关键能力：
- 独立 Rayon ThreadPool
- GEOFORGE_CONVERT_THREADS 环境变量支持
- 默认并行度: available_parallelism / 2, 限制在 1-8
- Block 级并行（Data 下的 Block）
- C++ FFI 异常边界处理
- 空 JSON、UTF-8 和空成果检查

### 转换器运行时锁定

**配置文件**: `apps/desktop/config/converter-runtime.json`

```json
{
  "name": "geoforge-converter",
  "version": "0.2.2",
  "source": "https://github.com/Nicander93/geoforge-converter",
  "upstream": "https://github.com/fanvanzh/3dtiles",
  "upstreamCommit": "GeoForge engines/3dtiles-converter snapshot bdb8b7fd31c1eaa7ef65ad9f3419c6ab8c56c149",
  "windowsX64": {
    "url": "https://github.com/Nicander93/geoforge-converter/releases/download/v0.2.2/geoforge-converter-0.2.2-windows-x64.zip",
    "sha256": "228c75a09d07c6c6c5c76ac1b57812206bd229656fae5907a4716545fe7a797d"
  }
}
```

**实际二进制验证**: UNMEASURED
- [ ] 确认 Release 资产与标签源码一致
- [ ] 验证 SHA256 匹配
- [ ] 确认运行时可执行文件位置

## 运行环境

### 操作系统

- **OS**: Linux 6.12.94+ x86_64
- **发行版**: Ubuntu/Debian (推测)
- **内核**: 6.12.94+ #1 SMP PREEMPT_DYNAMIC

**Windows 环境验收**: REQUIRED (待后续补充)

### 硬件配置

**当前测量环境 (VM)**:
- **CPU**: Intel(R) Xeon(R) Processor
- **CPU 核心数**: UNMEASURED (待 `lscpu` 或 `/proc/cpuinfo` 详细解析)
- **内存**: 16 GB (16398384 kB)
- **磁盘**: 254 GB (SSD 或 HDD: UNMEASURED)
- **磁盘类型**: 虚拟磁盘 `/dev/vdc`

**生产目标环境 (Windows 桌面)**:
- **CPU**: PLACEHOLDER (待填写目标桌面 CPU)
- **CPU 核心数**: PLACEHOLDER
- **内存**: PLACEHOLDER (建议 16GB+ 用于大规模数据)
- **磁盘**: PLACEHOLDER (建议 SSD 用于 I/O 密集型处理)

### 内存指标口径说明

不同操作系统对内存占用的报告方式不同，需要明确统计口径：

**Windows**:
- **工作集 (Working Set)**: 进程当前驻留在物理内存中的页面
- **私有提交内存 (Private Bytes)**: 进程独占的已提交内存
- **峰值推荐**: 使用 **私有工作集峰值** 或 **私有提交内存峰值**

**Linux**:
- **RSS (Resident Set Size)**: 驻留在物理内存中的页面
- **PSS (Proportional Set Size)**: 共享页面按比例分摊
- **峰值推荐**: 使用 `/proc/[pid]/status` 中的 **VmHWM (Peak RSS)** 或 **VmRSS**

**多进程场景**:
- **不能直接相加**: 各子进程在不同时刻的峰值不能当作同一时刻峰值
- **进程树峰值**: 需要同一时间点采样所有活跃子进程的内存占用之和
- **工具推荐**: 
  - Windows: Process Explorer, PerfMon
  - Linux: `smem`, `ps`, `/proc/[pid]/smaps_rollup`

## 并行配置基线

### 当前线程数控制

**转换器 (Converter)**:
- 配置路径: `src/osgb.rs::convert_threads()`
- 环境变量: `GEOFORGE_CONVERT_THREADS`
- 默认值: `available_parallelism / 2`, 限制在 1-8
- 上限: 8 线程

**主程序 (Processor)**:
- 配置路径: `crates/processor/src/stages/convert.rs::resolve_thread_config()`
- 配置字段: `options.convert.threads`
- 0 表示自动
- 显式值限制: 1-16
- 传递方式: 通过 `GEOFORGE_CONVERT_THREADS` 环境变量

**重建 (TopRebuild)**:
- 当前: 串行执行各层
- 同层 Proxy: 串行循环
- P5 后将改为有界并行

**纹理处理**:
- 当前: `tools/texture_ktx2/texture_ktx2.py` 串行处理
- P6 后将改为有界并行

### 资源准入控制

**当前状态**: NONE
- 无内存预算检查
- 无工作队列容量限制
- 存在多个高纹理成本 Block 同时处理的 OOM 风险

**P1+ 计划**: 
- 引入 `ExecutionOptions` 统一资源参数
- 内存预算准入控制
- 有界工作队列
- 加权 permit 机制

## 性能测量基线

### 测量指标

**必须采集**:
1. **阶段耗时**
   - 扫描 (Scan)
   - 转换 (Convert)
   - 重建 (Rebuild)
   - 纹理处理 (Texture)
   - 校验 (Validate)
   - 提交 (Commit)
   - 总耗时

2. **内存峰值**
   - 进程树峰值 (同一时刻所有子进程之和)
   - 主进程峰值
   - 转换器子进程峰值
   - 口径: Windows 私有提交内存 / Linux RSS

3. **CPU 利用率**
   - 平均 CPU 占用率 (%)
   - 各阶段 CPU 占用率
   - 线程数 vs CPU 核心数

4. **I/O 指标**
   - 读取字节总量
   - 写入字节总量
   - 临时盘峰值占用

5. **工作单元统计**
   - Block 总数
   - 成功转换 Block 数
   - 失败 Block 数
   - 最慢 Block 及其耗时
   - 最大 Block 及其大小

6. **失败信息**
   - 失败原因分类
   - 错误堆栈或诊断信息

### 基线测试矩阵

**状态**: UNMEASURED (无可用样本数据)

| 样本类型 | 大小 | Block 数 | 线程数 | 转换耗时 | 重建耗时 | 纹理耗时 | 峰值内存 | 状态 |
|---------|------|---------|--------|---------|---------|---------|---------|------|
| 小型样本 | 1-5 GB | TBD | 1/2/4/8 | UNMEASURED | UNMEASURED | UNMEASURED | UNMEASURED | 待测 |
| 中型样本 | 20-50 GB | TBD | 1/2/4/8 | UNMEASURED | UNMEASURED | UNMEASURED | UNMEASURED | 待测 |
| 大型样本 | 100-200 GB | TBD | 1/2/4/8 | UNMEASURED | UNMEASURED | UNMEASURED | UNMEASURED | 待测 |

**纹理处理模式**:
- **keep**: 保留原始纹理
- **KTX2**: 转换为 KTX2 格式

**样本要求**:
- 真实倾斜摄影数据 (OSGB 格式)
- 包含多 Block 结构
- 包含纹理数据
- 不提交样本数据进 Git

## 验收标准

### P0 最小交付

- [x] 记录双仓 SHA 和提交
- [x] 记录转换器版本和能力
- [x] 记录运行包配置文件
- [ ] 记录实际可执行文件位置和哈希 (需 Windows 环境)
- [x] 记录操作系统信息
- [x] 记录 CPU/内存/磁盘配置
- [x] 明确内存指标口径
- [ ] 真实小样本跑 threads 1/2/4/8 (需样本数据)
- [ ] 采集阶段耗时 (需样本数据)
- [ ] 采集进程树峰值内存 (需样本数据)
- [ ] 采集 CPU 利用率 (需样本数据)
- [ ] 采集读写字节 (需样本数据)
- [ ] 采集临时盘峰值 (需样本数据)
- [ ] 采集 Block 数量和失败原因 (需样本数据)
- [x] 增加 scripts/benchmark-large-dataset.* 脚本
- [x] 增加 docs/acceptance/large-data/ 文档框架
- [x] 定义 CSV/JSON schema
- [ ] 保持现有回归测试通过

### 待补充信息

1. **Windows 环境验证**
   - 实际运行环境的详细硬件配置
   - 转换器二进制文件验证
   - Windows 内存指标采集工具配置

2. **样本数据获取**
   - 小型样本 (1-5 GB)
   - 中型样本 (20-50 GB)
   - 大型样本 (100-200 GB)
   - 样本元数据 (Block 数、纹理数、LOD 层级)

3. **性能基线测量**
   - 各样本在不同线程数下的完整性能数据
   - 瓶颈识别 (CPU、内存、I/O)
   - 并行加速比分析

## 参考文档

- [执行计划](../../../uploads/geoforge-large-data-plan-v1_19a4.md)
- [转换器依赖说明](../../dependencies/3dtiles-converter.md)
- [仓库目录结构](../../REPOSITORY_LAYOUT.md)
- [V1 补齐进度](../../product/V1_COMPLETION_REPORT.md)
