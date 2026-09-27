# 仓库目录结构

本仓库是 GeoForge 产品工作区：一个 Tauri 桌面应用、多个 Rust 包，以及构建和验收工具。转换器源码在独立的 `geoforge-converter` 仓库维护，本仓库通过版本清单使用其发布包。

## 目录职责

| 目录 | 放什么 | 主要入口 |
| --- | --- | --- |
| `apps/desktop/` | 桌面 UI、Tauri 外壳、桌面配置和打包脚本 | [桌面开发说明](../apps/desktop/README.md) |
| `crates/` | Rust 包，每个包各有自己的 `src/` | 根目录 `Cargo.toml` |
| `tools/` | 独立纹理程序与显式使用的实验工具 | [工具分类](../tools/README.md) |
| `scripts/` | 仓库级启动、样例准备和 fixture 生成脚本 | 脚本自身的用法说明 |
| `tests/` | 测试代码和受版本管理的测试样本 | `tests/fixtures/` |
| `examples/` | 独立预览示例与示例资产 | [示例说明](../examples/README.md) |
| `docs/` | 产品说明、开发文档、参考资料、历史计划与验收记录 | 根 README 和各专题入口 |
| `.github/` | CI 与发布工作流 | `.github/workflows/` |
| `.cargo/` | Cargo 构建配置 | `.cargo/config.toml` |
| `.vscode/` | 编辑器配置 | `.vscode/settings.json` |

`apps/desktop/src/` 是前端源码，`apps/desktop/src-tauri/src/` 是桌面 Rust 源码。根目录是 Cargo 虚拟工作区，因此没有统一的根 `src/`。

`crates/protocol`、`crates/processor`、`crates/top_rebuild` 是默认构建成员；`crates/rebuild_top_cli` 仍是工作区成员，提供 Python 基线入口。本轮保留这些包的名称和位置。

## 本机目录与构建产物

`.cache/`、`target/`、根 `dist/`、`data/`、`samples/`、`acceptance-results/` 用于缓存、构建产物或本机数据，受现有 Git 忽略规则管理。VS Code 已隐藏缓存和构建目录；隐藏不会删除内容。

`apps/desktop/dist/` 有历史上已跟踪的构建文件。停止跟踪它需要单独确认所有消费方均会构建前端，本轮保留现有文件及本机改动。

## 2026-09 目录整理

| 原路径 | 当前路径或处理 |
| --- | --- |
| `third_party/3dtiles-converter.json` | `apps/desktop/config/converter-runtime.json`，开发与发布脚本已同步 |
| `.devcontainer/` | 已移除，不再维护容器开发环境 |
| `vs/` | 旧 C++ 工程已在工作区移除，当前构建无引用 |
| `index.html` | `examples/preview/indoor.html`，继续读取根目录 `output/` |
| `matrix.xlsx` | `docs/reference/matrix.xlsx`，保留原始内容 |
| `data/test/test.osgb` | `tests/fixtures/osgb/test.osgb`，仅迁移这一个受版本管理的样本 |
| `todo.txt` | 空文件，已移除 |

历史计划和验收报告中的路径描述保留为当时的记录。当前启动、构建和发布路径以根 README、桌面 README 和可执行脚本为准。

## 仍有调用关系的历史内容

- `tools/experiments/rebuild_top_py` 和 `crates/rebuild_top_cli` 仍有显式回归入口；删除会改变可用功能。
- `examples/preview` 仍被样例脚本和早期桌面 smoke 示例引用。
- `docs/acceptance/v1-convergence` 内的 `scripts/` 和 `fixtures/` 仍由 Ubuntu CI 使用。进一步将它们归入根 `scripts/` 和 `tests/fixtures/` 时，需要同时迁移脚本定位逻辑、CI 命令、fixture 生成器和文档入口。

新增代码按职责放入现有目录。构建产物不再新增到受版本管理的源码目录；阶段记录放进对应专题，避免继续向根目录添加临时文档或示例页面。
