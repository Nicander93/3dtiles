## Phase G

Date: **2026-09-12** Asia/Shanghai  
Scope: Desktop / Release 查找规则（部分）  
**已生成并完成 Windows NSIS 隔离安装与启动冒烟；安装包包含 Processor、TopRebuild 和离线 Cesium。原生 `_3dtile` 及其 GDAL/OSG 依赖尚未内置。**

### 本阶段目标

算法验证未到城区/百平方公里，按计划不应宣称产品发布完成。  
能做的是：正式二进制不要再写死 `/workspace/...`，Windows 要能找到旁边的 `.exe`。

### 发现的问题

1. `processor` 的 `_3dtile` 默认是 Linux 包装脚本 `/workspace/runtime/3dtile-bin/run.sh`。
2. `top_rebuild` 在 `target/{release,debug}` 里只找无后缀文件名，Windows 上是 `top_rebuild.exe`。
3. Tauri `resolve_processor_bin` 同样漏了 `.exe`，还写死 `/workspace/repos/3dtiles/...`。
4. `tauri.conf.json` 的 bundle **没有** sidecar：`processor` / `top_rebuild` / `_3dtile` / GDAL / OSG / BasisU。

### 修改

- `resolve_3dtile`：`GEOFORGE_3DTILE` → 与 processor 同目录 → `target/{release,debug}/_3dtile(.exe)` → Linux wrapper → PATH
- `resolve_top_rebuild`：同样认 `.exe`
- Tauri processor 查找：`target/{debug,release}/processor.exe`，去掉对 `/workspace` 的依赖

Python fallback（`python_bridge.rs`、设置里的 desktop_server URL）还在。正式包未保证 Processor 永远存在，所以不删。

### 关键文件

```text
crates/processor/src/util.rs
apps/desktop/src-tauri/src/process_manager.rs
```

### Tests

```powershell
cargo test -p processor --offline
cargo test -p top_rebuild --offline
```

```text
cargo test -p top_rebuild --offline
57 passed

cargo test -p processor --offline
0 passed（编译通过，含 -D warnings）
```

已执行：`npx tauri build --bundles nsis`、静默安装、首次启动、数据库初始化。转换、重建、预览与取消已在同一 Windows 主机上通过 Processor 全链路验证；安装版 UI 的人工点击回归和重启历史仍待补。

### Real Data Evidence

本阶段无新数据集。C 的 20 块仍用 CLI `top_rebuild.exe`，不经过安装包。

查找顺序（产品语义）：

```text
processor
  GEOFORGE_PROCESSOR
  → 当前 exe 同目录 processor(.exe)
  → 从 exe/cwd 向上找 target/{debug,release}/processor(.exe)

top_rebuild
  GEOFORGE_TOP_REBUILD
  → processor 同目录
  → repo target/{release,debug}

_3dtile
  GEOFORGE_3DTILE
  → processor 同目录
  → repo target/{release,debug}
  → /workspace/runtime/3dtile-bin/run.sh（仅当文件存在）
```

开发机仍可用 Docker 镜像里的 `_3dtile`，与 sidecar 不是同一条路径。

### 2026-09-12 补充

- Tauri `externalBin` 已登记 `processor` 与 `top_rebuild`。
- `npm run prepare:sidecars` 会编译 release 二进制，并生成 Tauri 要求的 target-triple 文件名。
- Cesium 1.125 已改为随前端构建复制，安装后预览不再依赖公网脚本。
- Processor 已在 Windows 上通过真实香港 5×4 OSGB 的 Docker fallback 全链路：转换 21.16 s，随后 Rust 重建与两次输出校验均成功。
- 本机原生 `_3dtile.exe` 构建仍缺 `VCPKG_ROOT` 与 C++ 依赖，因此当前 Windows 转换需要 Docker Desktop 或用户指定 `GEOFORGE_3DTILE`。
- Cargo 原生 TLS 握手在本机失败；使用锁文件对应的离线 vendor 依赖后，Tauri 编译和 NSIS 打包成功。
- 最终产物：`apps/desktop/src-tauri/target/release/bundle/nsis/GeoForge 3D_0.1.0_x64-setup.exe`（12,509,605 bytes，SHA-256 `fe1b2484daa35305257a30041b4cc1432f4599ef8d1e034205b5fa3908b08af4`）。
- 隔离静默安装退出码 0；安装目录包含 `geoforge-desktop.exe`、`processor.exe`、`top_rebuild.exe` 和卸载器。
- 最终安装版在沙箱外启动后持续运行，并创建 `tasks.db`、WAL 和 SHM 文件。沙箱内的退出码 101 来自本地监听端口被拒绝，不是安装包缺陷。

### 验收

- [x] sidecar 查找不再只认 Linux 仓库路径
- [x] Windows `.exe` 被考虑到
- [x] bundle 配置含 Processor + TopRebuild + Cesium
- [ ] 安装包内含原生 `_3dtile` + GDAL/OSG（当前可用 Docker fallback）
- [ ] 用户无需自装 Python / Node / Rust / CMake / OSG / GDAL
- [x] Windows NSIS 生成、隔离安装、首次启动与数据库初始化
- [x] 同机 Processor 转换 / 重建 / 预览 / 取消全链路
- [ ] 安装版 UI 人工点击回归与重启历史
- [ ] 评估并收敛 `python_bridge` 产品 fallback

### 未解决

- `_3dtile` 本机仍靠 Docker，不在 `target/release`
- Python fallback 仍在
- 本轮不做新 UI 框架、不做工作流引擎

### 下一步

当前可作为 **V1 可用候选版（Windows + Docker Desktop）**。若要宣称单机全内置、用户无需外部运行时，仍须把 `_3dtile.exe`、GDAL/OSG 插件和相关 DLL 纳入安装包，并补安装版 UI 回归与城区/百平方公里数据验证。
