# 仓库重构验收报告

日期：2026-09-13

## 实际目录与调用链

```
Desktop (apps/desktop)
  → processor.exe (crates/processor)  [GEOFORGE_PROCESSOR / sidecar / target]
      → _3dtile (engines/3dtiles-converter 或 GEOFORGE_3DTILE / Docker)
      → top_rebuild (crates/top_rebuild)
      → 纹理脚本 (tools/…，T07 前仍可能用 Python)
  → SQLite 任务/成果（应用数据目录）
```

扫描：`processor scan-osgb`（桌面不再内联复制扫描逻辑）。

## 相对方案的差异

- Docker 转换回退仍保留（引擎部署方式），与已删除的 Python **任务** HTTP 回退不同。
- `pythonServerUrl` 设置字段仍存在于诊断区，正式 `submit_task` 不再使用。
- `python_bridge.rs` 已从桌面源码移除。
- 远端仓库改名 `geoforge`：**未执行**（需另行授权）。

## 保留 / 迁移 / 删除

| 项 | 动作 |
| --- | --- |
| `geoforge-protocol` | 新建 |
| 转换器源码 | → `engines/3dtiles-converter` |
| 根虚拟 workspace | 产品 crates only |
| Python 任务桥接 | 正式路径删除 |
| 纹理 Python | 保留至 T07 |
| UI 黑白改版 | 保留 |

## 有效命令

```bash
cargo build                          # protocol + processor + top_rebuild
cargo test -p geoforge-protocol
cargo build -p processor
cd engines/3dtiles-converter && cargo build --release   # 需 vcpkg/OSG；本环境可能未验证
cd apps/desktop && npm run prepare:sidecars && npm run tauri:dev
```

## 已验证

- `cargo test -p geoforge-protocol` 通过
- `cargo build`（产品 default-members）通过，不编译 `_3dtile`
- 提交任务在缺 processor 时返回明确错误（代码路径）

## 未验证

- Windows 原生 `_3dtile` 完整编译与 DLL 收集（T05 / prepare-runtime.ps1）
- 真实 OSGB 端到端转换
- 干净机安装包（T09–T10）
- 子模块递归克隆后的转换器构建
- PyInstaller `geoforge-texture` 打包

详见 [V1_COMPLETION_REPORT.md](./V1_COMPLETION_REPORT.md)。

## 远端改名说明（未执行）

1. GitHub 将仓库重命名为 `geoforge`（或脱离 fork 网络，按托管策略）
2. 更新 clone URL、Actions badge、文档链接、子模块若有硬编码
3. **不要**改 Tauri identifier `local.geoforge.threed`（用户数据目录）
