# GeoForge 3D

本地三维地理数据工具箱：OSGB → 3D Tiles 转换、顶层重建、KTX2 纹理处理、Cesium 预览。

正式任务路径：**Desktop → Processor → Converter / TopRebuild**。缺 Processor 时明确失败，不再回退 Python HTTP 任务服务。

GeoForge 使用**预构建**的 Converter Runtime（[`Nicander93/geoforge-converter`](https://github.com/Nicander93/geoforge-converter)），不在本仓库编译 OSG/GDAL。

## 仓库结构

| 路径 | 职责 |
| --- | --- |
| `apps/desktop` | React UI + Tauri 外壳、任务/成果、本地预览 |
| `crates/protocol` | 任务配置与事件契约（`geoforge-protocol`） |
| `crates/processor` | 任务进程：扫描、转换、重建、纹理、校验、提交 |
| `crates/top_rebuild` | 自研顶层重建（Proxy HLOD） |
| `third_party/3dtiles-converter.json` | 固定 Converter Release URL + SHA256 |
| `tools/texture_ktx2` | KTX2 后处理（可封装为 `geoforge-texture`） |
| `docs/product/` | 产品、重构与 V1 补齐说明 |

## 快速开始（产品核心）

```bash
# 协议 / Processor / TopRebuild（不触发 OSG/GDAL）
cargo build
cargo test -p geoforge-protocol
cargo test -p processor --lib

# 桌面
cd apps/desktop
npm install
npm run prepare:sidecars
npm run tauri:dev
```

可选环境变量（开发覆盖；正式安装包应自带 runtime，一般不必设置）：

| 变量 | 含义 |
| --- | --- |
| `GEOFORGE_PROCESSOR` | processor 可执行文件 |
| `GEOFORGE_3DTILE` | `_3dtile` 可执行文件 |
| `GEOFORGE_TOP_REBUILD` | top_rebuild 可执行文件 |
| `GEOFORGE_RUNTIME_ROOT` | 打包 runtime 根目录 |
| `GEOFORGE_TEXTURE` / `GEOFORGE_BASISU` | 纹理工具与 BasisU |
| `GEOFORGE_DATA_DIR` | 用户数据目录 |

探测与任务共用同一套定位：

```bash
cargo run -p processor -- capabilities --json
```

## 转换器 Runtime

正式打包由 `prepare-converter.ps1` 按 `third_party/3dtiles-converter.json` 下载固定版本。

```powershell
powershell -File apps/desktop/scripts/prepare-converter.ps1
powershell -File apps/desktop/scripts/prepare-runtime.ps1
```

完整 Windows 安装包：

```powershell
cd apps/desktop
npm run package:windows
```

说明见 [docs/dependencies/3dtiles-converter.md](./docs/dependencies/3dtiles-converter.md)。

## 文档

- [仓库重构验收](./docs/product/REPO_REFACTOR_REPORT.md)
- [V1 补齐进度](./docs/product/V1_COMPLETION_REPORT.md)
- 历史阶段报告已归档至 [`docs/product/archive/`](./docs/product/archive/)，入口可能失效

## 许可证

转换器与上游组件保留原有许可证与版权声明。产品代码见仓库内各 crate 声明。
