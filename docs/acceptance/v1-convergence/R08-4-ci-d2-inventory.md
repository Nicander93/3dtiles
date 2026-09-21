# R08.4 CI Integration + D2 Data Inventory

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r08-4-ci-integration-7ab0

## 任务目标

1. CI 集成 D0 smoke test
2. LandsD/PlanD (D2) 数据 inventory 文档
3. 评估 R08 代码完成状态

## CI 集成 D0

### 实施

在 `.github/workflows/linux.yml` 中添加 D0 acceptance test job：

```yaml
- name: Run D0 acceptance tests
  run: |
    cargo build --release -p processor
    cd docs/acceptance/v1-convergence
    ./scripts/run-acceptance.sh --level d0
```

### 可行性分析

✅ **D0 非常轻量**:
- Fixture size: ~140 bytes
- 运行时间: <1s
- 构建时间: 已在前面 steps 完成 debug build
- Release build: ~13s (本 job 新增)

✅ **CI runner 容量足够**:
- GitHub Actions Ubuntu runner: 2 cores, 7 GB RAM
- D0 fixture 处理需要: <50 MB RAM, <1s CPU
- 不会显著增加 CI 时间

✅ **无外部依赖**:
- D0 只测试 processor + top_rebuild
- 不需要 converter (OSG/GDAL)
- 不需要 texture 工具 (basisu)

### 预期 CI 时间

| Step | Time |
|------|------|
| Existing steps | ~40s |
| Build processor (release) | ~13s |
| Run D0 | <1s |
| **Total** | **~54s** |

增加 ~14s，可接受。

### 退出策略

如果 CI D0 失败或太慢：
1. 检查日志识别问题
2. 如果是资源问题：移除 CI job，保留 script-only
3. 如果是代码问题：修复 processor/top_rebuild

当前评估：**可行，集成到 CI**。

## D2 Real Data Inventory

### 概述

D2 (Real datasets) 数据**不在 repo 中**，也**不在 CI 中运行**。

本节记录已知或计划的 D2 数据集，供**本地验收**使用。

### D2 数据梯度定义

| 特征 | 描述 |
|------|------|
| **规模** | 1GB+ |
| **来源** | 客户私有数据 或 大型公开数据集 |
| **许可** | 私有 或 严格限制 |
| **位置** | 仅本地，不上传 |
| **用途** | 生产级验收，性能测试 |

### LandsD/PlanD 候选数据集

#### 1. LandsD - 大型城市 OSGB 数据

**描述**: 大型城市倾斜摄影 OSGB 数据集

**预期规格**:
- 格式: OSGB
- 大小: 1-10 GB
- 覆盖: 城市街区，多栋建筑
- 纹理: 真实航拍纹理
- 层级: Tile blocks with LOD

**状态**: ⚠️ **Pending-local on LM**

**获取**:
- 路径: 待提供（本地 LM 环境）
- 许可: 客户私有或内部测试数据
- 不可分发

**预期 hashes**: 
- 未知（待本地运行后记录）
- 本文档将在首次运行后更新

**运行说明**:
```bash
# 本地 LM 环境
export D2_LANDSD_PATH=/path/to/landsd/data
cd docs/acceptance/v1-convergence
./scripts/run-acceptance.sh --level d2 --fixture landsd --input $D2_LANDSD_PATH
```

**预期输出**:
- 运行时间: 10-60 分钟
- 输出大小: ~输入的 0.5-2x（取决于 rebuild 和 texture）
- 性能指标: 记录在 `acceptance-results/`

#### 2. PlanD - 规划数据集

**描述**: 城市规划或 GIS 数据转换的 3D Tiles

**预期规格**:
- 格式: 3D Tiles 或 OSGB
- 大小: 500MB-5GB
- 覆盖: 城市规划区域
- 特征: 可能包含 I3DM (instances)

**状态**: ⚠️ **Pending-local on LM**

**获取**:
- 路径: 待提供（本地 LM 环境）
- 许可: 待确认
- 不可分发

**预期 hashes**: 
- 未知（待本地运行后记录）

**运行说明**:
```bash
# 本地 LM 环境
export D2_PLAND_PATH=/path/to/pland/data
cd docs/acceptance/v1-convergence
./scripts/run-acceptance.sh --level d2 --fixture pland --input $D2_PLAND_PATH
```

### D2 运行要求

#### 硬件要求
- CPU: 8+ cores
- RAM: 16+ GB
- 磁盘: 50+ GB 可用空间（输入 + 输出 + 临时）

#### 软件要求
- Processor (release build)
- top_rebuild (release build)
- 可选: converter (如果是 OSGB)
- 可选: texture 工具 (如果需要 texture processing)

#### 时间要求
- 1 GB 数据: ~10 分钟
- 5 GB 数据: ~30-60 分钟
- 10 GB 数据: ~1-2 小时

### D2 数据不在 CI 的原因

❌ **不可行原因**:
1. **数据太大**: GitHub Actions runner 磁盘和时间限制
2. **许可限制**: 私有或客户数据不能上传
3. **运行时间**: 超过 CI job 6 小时限制的 10% budget
4. **非 regression**: D2 是验收，不是每次 push 的 regression test

✅ **正确做法**:
- D0 in CI: smoke test, <1s, 每次 push
- D1 optional in CI: 如果需要，~1s
- D2 local-only: 生产验收，手动运行

## 无虚假运行声明

### Evidence 原则

❌ **不做的事**:
- 不声称 "D2 通过" 如果没有真实运行
- 不提交虚假的 `acceptance-results/d2-*/`
- 不在文档中声称 "LandsD 验证通过" 如果只是计划

✅ **做的事**:
- 明确标注: "Pending-local on LM"
- 记录预期规格和运行方式
- 首次真实运行后更新 hashes 和结果
- 保持 `acceptance-results/` gitignored

### 首次 D2 运行流程

1. **准备数据**: 在本地 LM 获取 LandsD/PlanD
2. **运行验收**: 
   ```bash
   ./scripts/run-acceptance.sh --level d2 --fixture landsd --input $D2_PATH
   ```
3. **记录结果**:
   - 复制 `acceptance-results/<run-id>/` 到安全位置
   - 记录 input data hash (如果可以)
   - 记录性能指标
4. **更新文档**:
   - 本文档添加 "实际运行结果" 章节
   - 记录 tip SHA, binary hashes, output hashes
   - 不提交 bulky output，只提交 summary

## run-acceptance.sh D2 支持

### 当前状态

D2 level 尚未实现在 `run-acceptance.sh` 中。

### 需要添加的功能 (R08.5 或 R09)

1. **`--fixture` 参数**: 允许指定 fixture 名称
2. **`--input` 参数**: 允许指定外部数据路径
3. **D2 处理逻辑**: 类似 D0/D1，但支持外部路径
4. **性能指标收集**: CPU time, memory peak, disk usage

示例：
```bash
./scripts/run-acceptance.sh \
  --level d2 \
  --fixture landsd \
  --input /local/data/landsd-city-2024 \
  --output-dir acceptance-results/v1-convergence/d2-landsd-$(date +%Y%m%d)
```

### 暂不实现 D2 logic 的原因

- R08 目标: 建立框架，D0/D1 smoke
- D2 需要真实数据才能测试
- 当前无 LM 环境访问
- R09+ 实现时有真实数据

## R08 代码完成状态评估

### 已完成

✅ **R08.1**: Acceptance harness + D0 fixture
✅ **R08.2**: Spatial quality checklist + D1 定义 + Layer B hooks
✅ **R08.3**: D1 synthetic fixture 实现
✅ **R08.4**: CI 集成 D0 + D2 inventory 文档

### R08 Scaffolding 完整性

| 组件 | 状态 | 备注 |
|------|------|------|
| **Data gradient 定义** | ✅ | D0/D1/D2 明确 |
| **D0 fixture** | ✅ | 140 bytes, single-tile |
| **D1 fixture** | ✅ | 6 KB, 2x2 grid |
| **D2 inventory** | ✅ | LandsD/PlanD 文档化 |
| **run-acceptance.sh** | ✅ | D0/D1 支持 |
| **run-info.json schema** | ✅ | 包含 spatialQuality hooks |
| **status.json schema** | ✅ | 包含 Layer B placeholders |
| **CI 集成** | ✅ | D0 in Linux workflow |
| **Evidence 原则** | ✅ | 明确非继承，无虚假 pass |

### R08 可以标记 "代码通过"

✅ **原因**:
1. 框架完整：D0/D1 可运行，D2 已文档化
2. Schema 就绪：Layer B hooks 定义完整
3. CI 集成：D0 每次 push 验证
4. Evidence 严格：无虚假 pass，placeholders 清晰
5. Tip re-run 可用：任何 SHA 可以运行 D0/D1

### 剩余工作推迟到 R09

⚠️ **Layer B 实数检查实现**:
- GE 单调性验证
- BV tightness 分析
- Frontier coverage 检查
- Subtree retention 检查
- Cesium A/B 对比

⚠️ **D2 run-acceptance.sh 支持**:
- 外部数据路径参数
- 性能指标收集
- 大数据集鲁棒性

### 开始 R09 的前提条件

✅ **R08 scaffolding 足够**:
- Tip re-run 可用 (D0/D1)
- Schema 定义完整
- Placeholders 就绪

✅ **可以开始 R09**:
- 实现 GE 单调性检查（移除 placeholder）
- 实现其他 Layer B 检查
- Cesium baseline 对比工具
- 真实 D2 数据运行（如果可用）

## 总结

### R08.4 完成内容

1. ✅ **CI 集成 D0**: 添加到 Linux workflow，<15s overhead
2. ✅ **LandsD/PlanD inventory**: 文档化规格、路径、pending-local
3. ✅ **无虚假运行**: 明确标注待本地运行，不声称通过

### R08 整体状态

**R08: 代码通过**

- Scaffolding 完整
- D0/D1 可运行
- D2 已文档化
- CI 集成完成
- Evidence 规则严格

**下一步**: R09 Layer B 实数检查实现

---

**状态**: R08.4 完成，R08 可标记 "代码通过"
**CI**: D0 集成到 Linux workflow
**D2**: Pending-local on LM，无虚假声称
