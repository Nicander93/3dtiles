# R08.1 Acceptance Harness & D0 Fixtures

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r08-1-acceptance-harness-7ab0

## 任务目标

建立 R08 验收测试框架：
1. 数据梯度定义（D0/D1/D2）
2. 测试 harness 结构
3. D0 tiny fixture 运行说明
4. 结果存储约定（gitignored）
5. Evidence 不继承声明

## 数据梯度定义

| 级别 | 描述 | 规模 | CI 可行 | 用途 |
|------|------|------|---------|------|
| **D0** | Tiny fixtures | <1MB | ✅ 是 | Smoke test, CI regression |
| **D1** | Medium samples | 10-100MB | ⚠️ 可能 | Integration test, quality spot check |
| **D2** | Real datasets | 1GB+ | ❌ 否 | Production validation, full quality |

### D0 Tiny Fixtures
- **目标**: 快速 smoke test，CI 可执行
- **数据**: 手工构造的最小 3D Tiles
- **覆盖**: 基本 tileset 结构、单个 content
- **许可**: MIT (repo-created)
- **位置**: `docs/acceptance/v1-convergence/fixtures/d0-tiny/`

### D1 Medium Samples (文档化，数据未打包)
- **目标**: 空间质量 spot check
- **数据**: 来自开源数据集的子集
- **覆盖**: 真实几何、纹理、层级
- **许可**: 需要验证（CC-BY 或类似）
- **位置**: 外部下载或本地路径
- **状态**: **R08.2+ 定义和获取**

### D2 Real Datasets (文档化，数据未打包)
- **目标**: 生产级验收
- **数据**: 客户数据或大规模公开数据
- **覆盖**: 完整工作流、大规模性能
- **许可**: 客户私有或明确许可
- **位置**: 仅本地，不上传 CI
- **状态**: **R08.3+ 或后续定义**

## 目录结构

```
docs/acceptance/v1-convergence/
├── fixtures/
│   ├── d0-tiny/                # D0 fixtures (committed)
│   │   ├── single-tile/        # 单个 B3DM tile
│   │   │   ├── tileset.json
│   │   │   └── tile.b3dm
│   │   └── README.md           # D0 fixture 说明
│   ├── d1-medium/              # D1 fixtures (文档，数据外部)
│   │   └── README.md           # D1 下载/设置说明
│   └── d2-real/                # D2 fixtures (文档，数据本地)
│       └── README.md           # D2 数据来源说明
├── scripts/
│   ├── run-acceptance.sh       # 主运行脚本
│   └── README.md               # 脚本使用说明
└── test-plan.md                # 完整测试计划

acceptance-results/             # Gitignored
└── v1-convergence/
    └── <run-id>/               # e.g. 20260921-150423
        ├── run-info.json       # Tip SHA, params, hashes
        ├── d0-tiny/            # D0 results
        ├── d1-medium/          # D1 results (if run)
        └── spatial-report.md   # Quality report
```

## Evidence 不继承原则

**明确声明**: 历史 evidence 和验收结果**不继承**。

### 原则
1. **每次运行独立**: 新的 tip SHA 需要新的验收运行
2. **不假设向后兼容**: 代码改进可能打破之前通过的测试
3. **结果绑定 SHA**: `run-info.json` 记录确切的 git SHA 和二进制 hash
4. **No false inheritance**: 我们不声称 "X 通过了，所以 X+patch 也通过"

### 实现
- `run-info.json` 包含 `tipSha` 字段
- 结果存储在 `<run-id>/` 下，与 timestamp 绑定
- 文档明确："本结果仅对 SHA abc123 有效"
- CI 每次 push 都重新运行 D0

## run-info.json 格式

```json
{
  "runId": "20260921-150423",
  "timestamp": "2026-09-21T15:04:23Z",
  "tipSha": "aac679d",
  "binaries": {
    "processor": {
      "path": "target/release/processor",
      "sha256": "abc123..."
    },
    "converter": {
      "path": "runtime/converter/3dtile.exe",
      "sha256": "def456..."
    }
  },
  "parameters": {
    "d0Enabled": true,
    "d1Enabled": false,
    "d2Enabled": false,
    "textureMode": "keep"
  },
  "environment": {
    "os": "Linux",
    "rustVersion": "1.98.1"
  }
}
```

## D0 Fixture: single-tile

**描述**: 最小可运行的 3D Tiles tileset

**内容**:
- `tileset.json`: 根 tileset，单个 tile
- `tile.b3dm`: 最小 B3DM（12 byte header + 空 GLB）

**用途**:
- Smoke test: processor 可以运行
- Validator: Layer A 验证通过
- 输出: 生成有效的 tileset

**限制**:
- 无真实几何（空 GLB）
- 无纹理
- 无层级（单 tile）

这是**最小可行 fixture**，不代表生产质量。

## 运行说明

### 前置条件
```bash
# 构建 processor
cargo build --release -p processor

# 确保 converter 存在（如果需要 convert-osgb）
# 对于 D0，我们只测试 process-tileset
```

### 运行 D0
```bash
# 手动运行
cd docs/acceptance/v1-convergence
./scripts/run-acceptance.sh --level d0 --output-dir ../../../acceptance-results/v1-convergence/manual-$(date +%Y%m%d-%H%M%S)

# CI 运行（R08.2+）
# GitHub Actions 会自动运行 D0
```

### 预期输出
```
acceptance-results/v1-convergence/<run-id>/
├── run-info.json
├── d0-tiny/
│   └── single-tile/
│       ├── input/          # Copy of fixture
│       ├── output/         # Processed output
│       ├── logs.txt        # Processor logs
│       └── status.json     # Pass/fail + validation
└── summary.md
```

## 状态字段

**run-info.json** 中的关键状态：
- `tipSha`: 运行时的 git commit
- `binaries[].sha256`: 每个工具的二进制 hash
- `parameters`: 运行参数（texture mode, rebuild 等）
- `environment`: OS、Rust 版本等

**每个 fixture 的 status.json**:
```json
{
  "fixture": "d0-tiny/single-tile",
  "passed": true,
  "duration": "1.2s",
  "validation": {
    "layerA": "PASS",
    "errors": 0,
    "warnings": 0
  },
  "outputSize": "1234 bytes"
}
```

## 明确不继承的例子

**错误**: "D0 在 SHA abc 通过了，所以 SHA def 也应该通过"

**正确**: "D0 在 SHA abc 通过了。SHA def 需要新的运行来确认。"

**实现**: 
- 每个 `<run-id>/` 目录独立
- CI 每次 push 重新运行 D0
- 文档不引用历史结果作为当前状态的证据

## 下一步 (R08.2+)

1. **R08.2**: 空间质量 checklist 脚手架
   - 定义检查项（bounding volume 准确性、GE 单调性等）
   - 集成到 acceptance harness
2. **R08.3**: D1 medium fixture 定义和获取
   - 识别开源数据集
   - 验证许可
   - 创建下载/设置脚本
3. **R08.4**: Cesium A/B 对比集成（R09 前期）
   - 定义对比方法
   - 集成到 harness

## License 和 Fixture 来源

### D0 Tiny Fixtures
- **License**: MIT
- **来源**: Repository-created，手工构造
- **可分发**: 是

### D1 Medium (待定)
- **License**: 待验证（目标 CC-BY 或类似）
- **来源**: 待识别开源数据集
- **可分发**: 取决于 license

### D2 Real (待定)
- **License**: 私有或严格限制
- **来源**: 客户数据或特定授权
- **可分发**: 否

---

**状态**: 框架文档完成，D0 fixture + 脚本待实现
