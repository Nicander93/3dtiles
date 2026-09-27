# 辅助工具

本目录同时包含正式纹理处理工具和供开发使用的实验实现，使用时按下表区分。

| 目录 | 当前用途 | 引用方 |
| --- | --- | --- |
| `texture_ktx2/` | 正式 KTX2 后处理程序，可打包为 `geoforge-texture` | Processor、桌面运行时准备与打包脚本 |
| `experiments/rebuild_top_py/` | Python 重建基线，用于显式选择的回归对照 | `rebuild_top_cli`、Processor 的 Python 引擎选项 |
| `experiments/desktop_server_py/` | 旧 FastAPI 服务，供显式启动和参考 | `scripts/run_geoforge.sh --legacy-server` |
| `ktx2_postprocess/` | Node.js 纹理处理实验 | 手工实验；当前桌面打包脚本未引用 |

正式桌面流程使用 Rust Processor 和 TopRebuild。`texture_ktx2` 仍参与正式发布，不能随实验目录一起删除。

参见 [纹理工具说明](texture_ktx2/README.md) 和 [仓库结构](../docs/REPOSITORY_LAYOUT.md)。
