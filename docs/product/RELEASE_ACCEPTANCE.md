# V1 发布验收清单

这份清单把 CI 能证明的内容和必须在 Windows 安装环境人工完成的内容分开。未执行项保持 `未执行`，不能改写成通过。

## CI 门槛

- [x] 根仓库 `cargo test` 通过（90 项，16 suites）。
- [x] `apps/desktop` 前端 `npm ci && npm test && npm run build` 通过（2026-09-16；`npm ci` 使用工作区临时缓存，未修改 lockfile）。
- [x] 干净安装后的 `npm test` 和 `npm run build` 通过。
- [x] Desktop Tauri library 测试通过（11 项，包含关闭清理、Job Object、启动恢复和错误诊断回归）；关闭事件同时覆盖 `CloseRequested`/`Destroyed` 并显式退出应用，安装版完整 WebView 点击验收仍保留为人工项。
- [x] Desktop Windows target `cargo check` 通过。
- [x] `processor capabilities --json` 能报告 converter、top_rebuild 和纹理能力。
- [x] `git diff --check` 通过。
- [x] Release Windows workflow 在 `npm ci` 后执行 `npm test`；安装器本身仍需 Windows 安装验收。
- [x] 发布脚本支持 `-ConverterZip` 本地候选输入并生成一致的 runtime manifest；正式构建默认仍使用 `third_party/3dtiles-converter.json`（回归 45）。
- [x] 从 `apps/desktop` 工作目录传入相对 `-ConverterZip`/`-OutDir` 时，脚本会规范化路径并生成正确的 manifest 相对文件名（回归 46）。
- [x] 本机候选 converter 已完成 MSVC/vcpkg Release 构建，NSIS 临时安装后 `--help` 正常，负向输入返回非零且无伪造成果。
- [x] 本机候选 converter 对真实 `OSGBny`（中文输入/输出目录）完成 convert-only 与 processor scan→convert→validate→commit，退出码均为 0。
- [x] 本机候选 converter 对包含中文 Tile 目录/文件名的真实样本完成 convert-only；输出 85 个文件，6/6 个根 URI 可解析（回归 38）。
- [x] processor 对同一中文 Tile 名称样本完成 scan→convert→validate→commit；输出 86 个文件，6/6 URI 可解析（回归 39）。
- [x] 最新本机候选 converter 对香港 8×8 真实 OSGB（11,124 个 OSGB、约 2.47 GB）完成全量 convert→validate→commit；退出码 0，耗时 99,842 ms，输出 11,186 个文件，64/64 个根 URI 可解析（回归 40）。
- [x] 同一最新候选成果完成 `rebuild-top --levels 0 --texture keep`→validate→commit；退出码 0，耗时 73,910 ms，输出 11,208 个文件，85/85 个 content URI 可解析（回归 41）。
- [x] 最新 converter Rust 候选（commit `bbe1426`，包含 `219b29b` 的错误传播修复）完成 Release 构建；发布脚本用本地候选 zip 完成 converter/runtime staging，`--help` 返回 0，manifest 不包含自身，真实 OSGB 转换返回 0，损坏输入返回 1，中文绝对路径转换返回 0（回归 48、55）。
- [x] Windows converter 发布 zip 已包含 x64 MSVC release CRT；桌面 runtime 将 CRT 同时部署到 converter 与 processor/top_rebuild 目录，并在 staging/NSIS 清单中强制检查（回归 54）。
- [x] converter `d06a495` 对有效 EPSG:4544 原点完成转换；投影域外原点返回明确 `OSGB_EPSG_TRANSFORM_FAILED`，processor 退出 1 且不提交最终目录（回归 49）。
- [x] 最新含 CRT/Unicode 修复的候选已重新打包 NSIS；安装到隔离目录后可启动并创建 `tasks.db`，安装内 converter/processor 对中文输入根、中文 Tile 名和中文/空格输出路径均成功，强制结束后无残留进程（回归 52–55）。
- [x] H11/H13 代码变更后已重新构建 NSIS（SHA256=`f40fe1e6506db3cccb83230e34ac1493e14e9abc473b490a9606005ee47a3a55`）；全新隔离安装完成 converter/top_rebuild `--help`、中文输入转换检查（回归 56）。
- [x] processor 重建坐标溢出修复已重新打入 NSIS（SHA256=`5dd2c9242ebc47759571d362c7dd5c7c71bf6fcbbeead5e66915cccc392f15af`）；全新隔离安装完成 converter `--help`、真实转换和 processor capabilities 检查（回归 59）。
- [x] 最新前端结构化错误解析、设置页目录选择失败提示和产品 CRT 完整门槛已打入 NSIS（SHA256=`a6b92ecdde25d2eddac08b9fb477f8658c7ad877a268d3caadcbe4386fc3d9ea`）；全新隔离安装完成 converter `--help`、85 文件真实转换、processor 86 文件转换和 capabilities 检查（回归 61）。

## Windows 安装验收

执行环境必须没有源码目录和开发覆盖变量。每次使用新的输出目录，并把记录写入 `HARDENING_REGRESSION.md`。

- [x] 安装路径包含空格时，ASCII 小样本 convert-only 成功（回归 31；安装到带空格目录，安装版 converter 退出 0，85 个文件，根 URI 6/6 可解析，随后已卸载）。
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

安装版启动曾做过一次隔离检查：当前受限执行环境无法写入默认用户 AppData，因而出现 SQLite `attempt to write a readonly database`；改用仓库隔离数据目录后进程可驻留，但窗口未被自动化接口枚举。该环境限制记录为回归 29，不能替代真实用户 Windows 安装验收。

## 当前状态

本仓库已完成 CI 可验证的加固和诊断链，并在本机完成候选 converter、真实 OSGB（含中文输入/输出路径）、10 次重复、香港 8×8 大数据和 NSIS 临时安装验收。processor CLI 的大样本取消和 Windows Job Object kill-on-close 单元测试也已通过；桌面关闭/多级进程树、安装桌面 WebView、当前用户失败样本、权限/磁盘故障仍是安装版待执行项。最新 converter 候选还完成了错误返回和发布脚本 staging 负向验证（回归 48）。正式发布还需把候选 converter 以带版本号的 zip 发布到固定 URL、更新 `third_party/3dtiles-converter.json` 的 SHA256；在此之前固定 runtime 清单不得改成候选 hash。
