# V1 发布验收清单

这份清单把 CI 能证明的内容和必须在 Windows 安装环境人工完成的内容分开。未执行项保持 `未执行`，不能改写成通过。

## CI 门槛

- [x] 根仓库 `cargo test` 通过（90 项，16 suites）。
- [x] `apps/desktop` 前端 `npm ci && npm test && npm run build` 通过（2026-09-16；`npm ci` 使用工作区临时缓存，未修改 lockfile）。
- [x] 干净安装后的 `npm test` 和 `npm run build` 通过。
- [x] Desktop Tauri library 测试通过（7 项）。
- [x] Desktop Windows target `cargo check` 通过。
- [x] `processor capabilities --json` 能报告 converter、top_rebuild 和纹理能力。
- [x] `git diff --check` 通过。
- [x] Release Windows workflow 在 `npm ci` 后执行 `npm test`；安装器本身仍需 Windows 安装验收。
- [x] 本机候选 converter 已完成 MSVC/vcpkg Release 构建，NSIS 临时安装后 `--help` 正常，负向输入返回非零且无伪造成果。
- [x] 本机候选 converter 对真实 `OSGBny`（中文输入/输出目录）完成 convert-only 与 processor scan→convert→validate→commit，退出码均为 0。

## Windows 安装验收

执行环境必须没有源码目录和开发覆盖变量。每次使用新的输出目录，并把记录写入 `HARDENING_REGRESSION.md`。

- [ ] 安装路径包含空格时，ASCII 小样本 convert-only 成功。
- [ ] 中文用户目录和中文数据根在支持范围内成功；输出 URI 能被 Cesium 加载。
- [ ] convert + rebuild 与 convert-only 分开验证；规则网格、稀疏布局和非标准 Tile 得到符合预期的提示。
- [ ] 当前用户失败样本有明确结果：已修复，或在长任务开始前明确拒绝。
- [ ] converter 非零退出时任务详情显示阶段、退出码、stderr 尾部和诊断文件路径。
- [ ] 取消、关闭桌面、重新启动后无遗留 processor/converter 进程；已提交成果不被删除。
- [ ] 输出目录已有内容、父目录不可写、磁盘空间不足时不覆盖旧成果并给出路径相关提示。
- [ ] 安装桌面结果通过内部 3D Tiles 校验并在 Cesium WebView 中检查定位、缺块、纹理和 LOD。
- [x] 本机候选 runtime 对真实 `OSGBny` 连续执行 10 次；10/10 成功，日志和资源指标已保留（见回归 20）。

已在 processor CLI 完成输出目录 no-overwrite 和低磁盘阈值告警回归（回归 22–23）。安装版的真实权限拒绝、磁盘写满和桌面 UI 提示仍需人工执行。

本地 Cesium 页面已加载回归 23 的真实成果，页面显示 `tilesLoaded · view r≈959m` 且控制台无错误（回归 24）；安装桌面 WebView 仍保持未执行。

## 当前状态

本仓库已完成 CI 可验证的加固和诊断链，并在本机完成候选 converter、真实 OSGB（含中文输入/输出路径）、10 次重复与 NSIS 临时安装验收。processor CLI 的大样本取消和 Windows Job Object kill-on-close 单元测试也已通过；桌面关闭/多级进程树、安装桌面 WebView、当前用户失败样本、权限/磁盘故障仍是安装版待执行项。正式发布还需把候选 converter 以带版本号的 zip 发布到固定 URL、更新 `third_party/3dtiles-converter.json` 的 SHA256；在此之前固定 runtime 清单不得改成候选 hash。
