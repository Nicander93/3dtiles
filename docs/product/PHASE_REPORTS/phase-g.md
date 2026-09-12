## Phase G

Date: **2026-09-12** Asia/Shanghai  
Scope: Desktop / Release 查找规则（部分）  
**没有做出可分发的 Windows 安装包，也没有做安装后冒烟。**

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

未执行：`npm run tauri:build` 安装包、首次启动、选 OSGB、取消任务、重启历史。

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

### 验收

- [x] sidecar 查找不再只认 Linux 仓库路径
- [x] Windows `.exe` 被考虑到
- [ ] 安装包内含 Desktop + Processor + TopRebuild + `_3dtile` + GDAL/OSG + Cesium
- [ ] 用户无需自装 Python / Node / Rust / CMake / OSG / GDAL
- [ ] Windows 安装冒烟（启动 / 转换 / 重建 / 预览 / 取消 / 重启历史）
- [ ] 评估并收敛 `python_bridge` 产品 fallback

### 未解决

- 没有 Windows 安装包
- `_3dtile` 本机仍靠 Docker，不在 `target/release`
- Python fallback 仍在
- 本轮不做新 UI 框架、不做工作流引擎

### 下一步

有 16×16+ 真数据和安装包之后，再谈 V1 发布。当前准确描述见 SUMMARY 与 `REAL_DATA_VALIDATION.md`。
