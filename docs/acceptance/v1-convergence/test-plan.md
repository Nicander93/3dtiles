# R08 Acceptance Test Plan

**版本**: R08.1 (初版)  
**日期**: 2026-09-21  
**状态**: D0 完成，D1/D2 待实现

## 目标

建立分层验收测试体系，从快速 smoke test 到完整生产验证：
1. **D0 Tiny**: CI smoke test，快速反馈
2. **D1 Medium**: 空间质量 spot check
3. **D2 Real**: 生产级验收

## 数据梯度

| 级别 | 数据规模 | 运行时间 | CI 可行 | 主要目的 |
|------|---------|---------|---------|----------|
| **D0** | <1MB | <10s | ✅ | Smoke test, 格式正确性 |
| **D1** | 10-100MB | ~1min | ⚠️ | 空间质量, 真实几何 |
| **D2** | 1GB+ | 10min+ | ❌ | 生产验收, 大规模性能 |

## 测试维度

### 1. 格式正确性 (D0+)
- ✅ 有效的 3D Tiles 1.0 格式
- ✅ Layer A 验证通过
- ✅ 输出可被标准查看器读取

### 2. 空间质量 (D1+)
- ⚠️ Bounding volume 准确性
- ⚠️ Geometric error 单调性
- ⚠️ 层级合理性
- ⚠️ 无几何退化

### 3. 视觉质量 (D1+)
- ⚠️ 纹理无明显失真
- ⚠️ 坐标系转换正确
- ⚠️ 无明显接缝

### 4. 性能 (D2)
- ❌ 大数据集处理时间
- ❌ 内存峰值
- ❌ 输出大小优化

### 5. 鲁棒性 (所有级别)
- ✅ 不 crash
- ✅ 错误信息有用
- ⚠️ 边界条件处理

## D0 Tiny - Smoke Test

**目标**: 快速验证基本功能，CI 每次 push 运行

### 包含的 Fixtures

#### single-tile
- **内容**: 单个 B3DM tile
- **大小**: 104 bytes
- **特征**: 最小有效格式，无真实几何
- **测试**: processor 可运行，Layer A 通过

### 覆盖的检查项

| 检查项 | 方法 | 预期 |
|-------|------|------|
| Processor 运行 | 退出码 | 0 |
| Layer A 验证 | logs 中无 ERROR | 通过 |
| 输出存在 | tileset.json 存在 | ✅ |
| 输出格式 | JSON 可解析 | ✅ |

### 限制

D0 **不测试**：
- 空间质量（无真实几何）
- 视觉质量（无纹理）
- 性能（数据过小）
- 复杂层级（单 tile）

### 运行

```bash
cd docs/acceptance/v1-convergence
./scripts/run-acceptance.sh --level d0
```

**预期时间**: <10 秒

## D1 Medium - Quality Spot Check

**目标**: 验证空间和视觉质量，真实几何

**状态**: R08.2+ 实现

### 计划的 Fixtures

#### small-building (待定)
- **内容**: 单栋建筑的 OSGB 或 3D Tiles
- **大小**: ~10MB
- **来源**: 开源数据集（待识别，license 待验证）
- **特征**: 真实几何、纹理、多 LOD

#### urban-block (待定)
- **内容**: 城市街区
- **大小**: ~50MB
- **特征**: 多建筑、空间层级、真实坐标系

### 覆盖的检查项

| 检查项 | 方法 | 预期 |
|-------|------|------|
| Bounding volume 准确性 | 计算实际范围 vs BV | <5% 误差 |
| GE 单调性 | 父 GE ≥ 子 GE | 严格满足 |
| 层级合理性 | 深度/分支因子 | 符合 3D Tiles 最佳实践 |
| 纹理保真度 | 目视检查或 PSNR | 无明显失真 |
| 坐标系正确性 | 关键点对比 | <1m 误差 |

### 工具

- **空间检查**: 自定义脚本（R08.2）
- **视觉检查**: CesiumJS viewer + 截图
- **对比基准**: Cesium 3D Tiling Pipeline (R09)

### 运行

```bash
# 下载 D1 fixtures (一次性)
./scripts/download-d1-fixtures.sh

# 运行 D1
./scripts/run-acceptance.sh --level d1
```

**预期时间**: ~1 分钟

## D2 Real - Production Validation

**目标**: 完整生产级验收，大规模数据

**状态**: R08.3+ 定义

### 计划的数据集

#### customer-dataset-A (待定)
- **内容**: 客户实际数据
- **大小**: 1-10GB
- **许可**: 客户私有，不上传
- **特征**: 生产真实场景

#### public-large-city (待定)
- **内容**: 大型城市开源数据
- **大小**: 5-20GB
- **许可**: 待确认
- **特征**: 大规模、复杂层级

### 覆盖的检查项

| 检查项 | 方法 | 预期 |
|-------|------|------|
| 处理时间 | wall clock | 记录基准 |
| 内存峰值 | `/usr/bin/time -v` | <16GB |
| 输出大小 | du -sh | 合理压缩率 |
| 大规模鲁棒性 | 无 OOM/crash | 完整运行 |

### 运行

```bash
# D2 数据在本地，路径配置在环境变量或 config
export D2_DATA_ROOT=/path/to/large/datasets

./scripts/run-acceptance.sh --level d2
```

**预期时间**: 10-60 分钟

## Evidence 非继承原则

### 关键原则

**每个 git SHA 需要独立验收运行。**

历史结果**不继承**：
- ❌ 错误: "SHA abc 通过了，所以 SHA abc+patch 也应该通过"
- ✅ 正确: "SHA abc 通过了。SHA def 需要新运行。"

### 实现

1. **run-info.json 绑定 SHA**
   ```json
   {
     "tipSha": "aac679d",
     "binaries": {
       "processor": {
         "sha256": "abc123..."
       }
     }
   }
   ```

2. **结果目录按 run-id 隔离**
   ```
   acceptance-results/v1-convergence/
   ├── 20260921-150423/  # SHA aac679d
   └── 20260921-160530/  # SHA def4567
   ```

3. **文档明确声明**
   每个 summary.md 包含：
   > This result is valid ONLY for git SHA aac679d.

4. **CI 每次重新运行 D0**
   GitHub Actions 每次 push 都运行完整 D0 suite。

### 为什么重要

- **代码变更可能破坏已通过的测试**: 即使是小 patch
- **不同 binary 产生不同结果**: 必须绑定 binary hash
- **避免虚假信心**: "曾经通过" ≠ "现在通过"

## 测试组织

```
docs/acceptance/v1-convergence/
├── test-plan.md               # 本文档
├── R08-1-harness.md           # R08.1 实现文档
├── fixtures/
│   ├── d0-tiny/
│   │   ├── single-tile/       # D0 fixture
│   │   └── README.md
│   ├── d1-medium/             # D1 fixtures (R08.2+)
│   │   └── README.md
│   └── d2-real/               # D2 fixtures (R08.3+)
│       └── README.md
└── scripts/
    ├── run-acceptance.sh      # 主运行脚本
    ├── download-d1-fixtures.sh # D1 下载脚本 (R08.2+)
    └── README.md

acceptance-results/            # Gitignored
└── v1-convergence/
    └── <run-id>/
        ├── run-info.json
        ├── d0-tiny/
        ├── d1-medium/
        ├── d2-real/
        └── summary.md
```

## CI 集成

### R08.1 (当前)
- ✅ D0 在 GitHub Actions 中运行
- ✅ PR 必须通过 D0 才能 merge

### R08.2+ (Future)
- ⚠️ D1 可选运行（如果 fixture 已授权）
- ⚠️ Spatial quality 检查集成
- ⚠️ Visual diff 对比 baseline

### R08.3+ (Future)
- ❌ D2 仅本地运行（数据过大或私有）
- ❌ 生成性能报告
- ❌ 对比 Cesium baseline (R09)

## License 和数据来源

| 级别 | License | 来源 | 可分发 | 状态 |
|------|---------|------|-------|------|
| D0 | MIT | Repository-created | ✅ | 完成 |
| D1 | 待定 (目标 CC-BY) | 开源数据集 | ⚠️ | R08.2+ |
| D2 | 私有/严格限制 | 客户或授权 | ❌ | R08.3+ |

## 实施路线

### R08.1 (本 PR) ✅
- [x] D0 fixture: single-tile
- [x] 运行脚本 harness
- [x] run-info.json 格式
- [x] Evidence 非继承声明
- [x] 文档和 README

### R08.2 (下一步)
- [ ] 空间质量检查脚本
- [ ] D1 fixture 识别和获取
- [ ] CI 集成 D0
- [ ] Spatial report 生成

### R08.3
- [ ] D2 数据集定义
- [ ] 性能指标收集
- [ ] 大规模鲁棒性测试

### R08.4 (与 R09 重叠)
- [ ] Cesium baseline 对比
- [ ] Visual diff 工具
- [ ] 完整验收报告

---

**状态**: R08.1 完成，框架可运行
**下一步**: R08.2 空间质量 checklist
