# Acceptance Test Scripts

这个目录包含 R08 验收测试 harness 的运行脚本。

## run-acceptance.sh

主验收测试运行脚本。

### 用法

```bash
# 运行 D0 tiny fixtures (默认)
./run-acceptance.sh --level d0

# 指定输出目录
./run-acceptance.sh --level d0 --output-dir /tmp/my-acceptance-run

# 使用 debug build
./run-acceptance.sh --level d0 --processor ../../../target/debug/processor

# 查看帮助
./run-acceptance.sh --help
```

### 参数

- `--level LEVEL`: 数据梯度级别 (d0, d1, d2)，默认 d0
- `--output-dir DIR`: 输出目录，默认自动生成时间戳目录
- `--processor PATH`: processor 二进制路径，默认 `target/release/processor`

### 输出

脚本在 `acceptance-results/v1-convergence/<run-id>/` 下创建：

- `run-info.json`: 运行元数据（git SHA、binary hash、参数）
- `<level>/*/status.json`: 每个 fixture 的结果
- `<level>/*/logs.txt`: Processor 日志
- `<level>/*/output/`: 处理后的输出
- `summary.md`: 运行摘要

### 退出码

- `0`: 所有测试通过
- `1`: 至少一个测试失败或脚本错误

### 示例

```bash
# 运行 D0，查看结果
$ ./run-acceptance.sh --level d0
[2026-09-21 15:04:23] === R08 Acceptance Test Harness ===
[2026-09-21 15:04:23] Level: d0
[2026-09-21 15:04:23] Output: .../acceptance-results/v1-convergence/20260921-150423
[2026-09-21 15:04:23] Running D0 tiny fixtures...
[2026-09-21 15:04:24] ✅ single-tile: PASS (1 s)
[2026-09-21 15:04:24] === Run Complete ===

# 检查结果
$ cat acceptance-results/v1-convergence/20260921-150423/summary.md
```

## CI 集成 (R08.2+)

在 GitHub Actions 中运行：

```yaml
- name: Run D0 acceptance tests
  run: |
    cargo build --release -p processor
    cd docs/acceptance/v1-convergence
    ./scripts/run-acceptance.sh --level d0
```

## 前置条件

### D0
- ✅ Processor 已构建 (`cargo build --release -p processor`)
- ✅ Fixtures 存在 (`fixtures/d0-tiny/`)

### D1 (R08.2+)
- ⚠️ D1 fixtures 已下载或设置
- ⚠️ 足够的磁盘空间 (~100MB)

### D2 (R08.3+)
- ⚠️ D2 数据集在本地可用
- ⚠️ 足够的磁盘空间 (1GB+)
- ⚠️ 足够的运行时间

---

**状态**: R08.1 完成 D0 runner，D1/D2 待 R08.2+
