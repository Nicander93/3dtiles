# 仓库重构基线与迁移清�?

日期�?026-09-13  
HEAD：`c1e49c356e9903251255311bc80a743847566737`（工作区另有未提�?UI 改动，须保留�?

## 环境

| �?| 状�?|
| --- | --- |
| rustc / cargo | 1.96.0 可用 |
| `cargo build -p processor -p top_rebuild` | 通过 |
| `_3dtile` / OSG/vcpkg 原生转换 | 本机未在本基线复跑；记为环境/构建阻塞 |
| 真实 OSGB 样例 | 仓库内无 OSGB 数据；tiles �?`tests/fixtures/top_rebuild/*` |

## 正式入口（重构前�?

- Desktop：`apps/desktop`（Tauri + React�?
- Processor：`crates/processor`（`run` / `scan-osgb` / convert / process-tileset�?
- 重建：`crates/top_rebuild`
- 转换器：�?package `_3dtile`（Cargo + CMake/vcpkg�?
- 历史回退：`python_bridge` + `tools/experiments/desktop_server_py`

## 迁移清单

| 内容 | 动作 |
| --- | --- |
| TaskConfig / Stage / 退出码 / 事件字段 | �?`crates/protocol`（geoforge-protocol�?|
| Emitter（stdout JSONL�?| 留在 processor |
| commands 内复�?scan | �?调用 `processor scan-osgb` |
| Python HTTP 任务回退 | 正式路径移除；缺 processor 明确失败 |
| �?`_3dtile` 源码/CMake/vcpkg | �?`engines/3dtiles-converter/` |
| rebuild_top_cli stub | 核实引用后移除或接入 |
| UI 黑白改版 | 保留，不回滚 |
| 纹理 Python | 保留�?T07 |

## 已知失败分类

- Processor 缺失时当前会静默�?Python：原有设计缺�?�?本轮改为显式失败
- commit 覆盖已有输出：原有缺�?�?T01
- 取消杀�?PID：原有缺�?�?T03
