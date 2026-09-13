# V1 功能补齐进度报告

日期：2026-09-13

## 总览

| 任务 | 状态 | 说明 |
| --- | --- | --- |
| T01 路径策略 / 安全提交 | 已实现 | `path_policy` + no-replace commit；单元测试通过 |
| T02 输出校验 | 已实现 | 结构/引用/B3DM/GLB；坏数据拒绝 commit |
| T03 进程树取消 | 已实现 | 协作 cancel + 宽限期；Windows Job Object；Docker `docker stop` |
| T04 串行队列 | 已实现 | 单 runner；queued 取消不启动；登记失败可重登记；真实 progress |
| T05 原生转换器运行时 | 脚本就绪 | `prepare-runtime.ps1`；本机完整 vcpkg 构建 **待验收** |
| T06 统一定位 / capabilities | 已实现 | `processor capabilities --json`；桌面同路径 |
| T07 KTX2 封装 | 代码就绪 | `tools/texture_ktx2/`；嵌入 KTX2 检测；PyInstaller 包 **待构建机验收** |
| T08 重建边界 | 已实现 | 连续网格预检；quality → 三角预算；>16×16 警告 |
| T09 安装包入口 | 脚本就绪 | `package-windows.ps1`；缺文件失败；完整 NSIS **待构建机** |
| T10 干净机验收 | 待验收 | 无干净 VM；清单见下 |

## 有效命令

```bash
cargo test -p processor --lib
cargo run -p processor -- capabilities --json
# Windows
powershell -File apps/desktop/scripts/prepare-runtime.ps1
powershell -File apps/desktop/scripts/package-windows.ps1
```

## T10 验收清单（待执行）

1. 干净 Windows + WebView2（若需首次下载须写明）
2. 安装 NSIS 包，不设 GEOFORGE_* / Docker / Python
3. OSGB 小样 → 转换 → 可选重建/压缩 → 预览
4. 已有 3D Tiles → 重建/压缩 → 预览
5. 取消任务、重启后历史与成果可读
6. 记录：安装包哈希、OS 版本、任务 ID、日志路径

## 阻塞项

- Windows MSVC + vcpkg 完整编译 `_3dtile` 与 DLL 收集（T05/T09）
- PyInstaller `geoforge-texture` onedir（T07/T09）
- 干净机实测（T10）
