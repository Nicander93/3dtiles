# V1 Hardening 实施进度

更新：2026-09-16

## 任务卡状态

| 卡片 | 状态 | 当前证据/限制 |
| --- | --- | --- |
| H01 | 已完成 | 基线、旧 runtime hash 和负向实验已记录 |
| H02 | 已完成 | UTF-8 截断、协议 stdout 和日志 tail 单测通过 |
| H03 | 已完成 | 受控并发更新和日志写失败单测通过 |
| H04 | 已完成 | 临时目录归属、task marker、独占创建和 no-overwrite 单测通过 |
| H05 | 已完成 | 两个转换页面和错误解析已构建验证 |
| H06 | 已完成 | stderr/stdout sidecar、受控非零 converter fixture、退出码和任务详情链路已验证 |
| H07 | 已完成（本机候选构建） | converter Rust/C++ 修改已用 MSVC 18.6.2 + vcpkg x64-windows 构建；真实 OSGB 与中文输入路径已通过本机候选回归 |
| H08 | 已完成（本机候选构建） | OSG `OSG_USE_UTF8_FILENAME` 路径修复、URI/JSON 修改已进入候选 runtime；真实中文输入/输出目录转换通过 |
| H09 | 未触发 | 尚未取得同一新 binary 的原生崩溃证据 |
| H10 | 本机候选已验证，正式发布待更新 | 本机候选 runtime 已通过 `--help`、负向输入、真实 OSGB 和 NSIS 临时安装；固定清单仍保留已发布 SHA256，待发布新 converter 后再更新 |
| H11 | 代码完成，processor 取消已验证 | 大样本 CLI 取消返回 `CANCELLED`，无最终/临时目录和残留进程；桌面关闭后的 Job Object 进程树测试仍待单独验收 |
| H12 | 已完成 | 文件 tail 有界并处理 UTF-8 边界 |
| H13 | 已完成 | scanner/layout/capabilities 单测和 smoke 通过 |
| H14 | 代码完成，已取得大样本取消/收尾证据 | 磁盘、线程、峰值内存指标已接入；约 932 MB/4567 Tile 样本取消后无残留；完整成功资源曲线、权限/磁盘故障注入仍待执行 |
| H15 | CI 门槛完成，安装验收部分完成 | `npm ci`、前端测试/构建、Rust/Tauri、NSIS 临时安装、真实 OSGB 与中文路径、10 次重复已通过；Cesium、当前用户失败样本、桌面关闭/Job Object 仍待执行 |
| H16 | 按计划延期 | 保持严格失败，不启用部分成果 |

## 已完成并验证

- 任务日志内存尾部按 UTF-8 字符边界截断，新增中文和 emoji 单测。
- 任务写入增加进程内串行写锁，避免日志、取消和错误更新互相覆盖。
- 任务日志文件创建/写入失败会返回明确错误，同时保留数据库内存日志，不递归写入错误日志。
- 日志读取限制为最多 256 KiB 和 5000 行，避免每次轮询读取完整大文件。
- processor stdout/诊断管道读写失败时会主动终止子进程，避免桌面层因管道错误遗留后台 processor。
- converter stderr 的显示读取按 64 KiB 分片，超长无换行输出不会无限增长单个内存 buffer。
- 桌面 processor stderr sidecar 同样按固定块写原始文件、按有界片段更新任务日志。
- 桌面 processor JSONL stdout 也按有界单行读取；完整原始字节仍写入 sidecar，超长协议行不会无限占用内存。
- 任务列表轮询不再传输完整日志正文，打开详情时才请求有界日志 tail。
- legacy Python desktop server 的任务列表也只返回空 log，占用 tail 查询时再读取详情。
- converter 命令保留最多 100 行 / 64 KiB stderr 尾部；非零退出错误包含退出码和尾部内容。
- 新增跨平台受控命令回归，验证非零退出码和 stderr 尾部不会被吞掉。
- 桌面 processor 为 stdout JSONL 和 stderr 分别保留独立原始诊断文件，并在任务详情显示路径；协议 stdout 管道保持不变。
- 任务详情保留机器可读的 `errorCode`/`failedStage`，并给出针对输出目录、converter 退出和取消的简短建议。
- 前端错误解析支持字符串、Error、`message` 和嵌套 `detail.message`。
- 两个页面的目录选择异常会显示；输出选择器选择父目录后生成成果子目录，后端仍禁止覆盖。
- 临时工作目录已存在时拒绝删除；取消只清理由本次运行创建的目录，普通失败保留目录供排查。
- 临时目录 marker 必须与 `.geoforge-task-<taskId>` 目录名一致；不匹配的目录不会被清理或提交。
- Windows processor Job Object 设置 `KILL_ON_JOB_CLOSE`，降低强杀留下子进程的风险。
- Tileset 复制改为 1 MiB 分块并在每块前检查取消，避免大文件复制长时间不响应。
- Windows 输出父目录预检记录剩余空间 metric；低于 `GEOFORGE_LOW_DISK_BYTES` 时发出 warning，不把容量估算当作硬保证。
- converter Rust Tile 结果改为显式 Success/Failure，覆盖空指针、非法长度、非法 UTF-8、非法 JSON、空 Tile 和结果通道错误。
- converter 并发支持 `GEOFORGE_CONVERT_THREADS`；未设置时使用逻辑 CPU 的一半并限制为最多 8 个线程。
- processor 记录 converter 配置线程数和耗时 metric，converter 启动时也记录实际 worker 数。
- processor 在可用平台采样 converter 峰值工作集并发出 `converter.peakMemoryBytes`；采样不可用时不伪造数值。
- converter C++ 未知 primitive 改为返回失败并逐层传播；Tile 写入失败不再伪装成功。
- OSGB/GLB 的 extern C 入口增加异常边界，C++ 异常转为明确失败返回，不越过 FFI 边界。
- converter C++ FFI 入口拒绝空指针、负层级和非有限坐标；返回 JSON 前检查空值、长度上限和内存分配失败。
- converter 输出 URI 使用 UTF-8 文件名和 JSON serializer 转义。
- capabilities 探测现在要求 `--help` 实际以退出码 0 完成；Python 纹理脚本还需解释器和 basisu/打包纹理组件，避免仅凭文件存在误报可用。
- OSGB metadata 预检现在区分 XML 结构错误与缺少 SRS；convert-only 可识别 converter 能发现的同名非标准 Tile，rebuild 会明确拒绝该布局。
- Tileset 校验要求 geometricError 为数字、boundingVolume 只有一个合法变体，并拒绝 GLTF buffer/image URI 越出数据根目录。
- 新增受控并发回归，验证日志、取消和错误更新不会互相覆盖；日志文件 tail 也从 UTF-8 字符边界开始。
- 新增回归记录模板和真实数据矩阵。
- 用隔离的有效 B3DM synthetic fixture 完成一次 `process-tileset` 的 scan→rebuild→validate→commit smoke；仓库原始 0 字节占位 fixture 仍记录为阻塞，未冒充真实数据回归。
- 新增发布验收清单，明确 CI 证据与 Windows 安装版人工验收的边界。
- Windows runtime manifest 现在记录每个文件 SHA256；converter `--help` 超时或非零会阻止 staging；Tauri runtime staging 会先清理旧文件，Release Windows 先执行 `npm test`。
- Product/Windows CI 现在额外运行完整 Rust workspace 测试；Windows CI 明确执行 Tauri Windows target check。
- 前端新增无外部依赖的纯函数回归测试，覆盖 `friendlyError`、输出目录建议、路径比较和真实进度计算；Product/Windows CI 在 build 前执行 `npm test`。
- 使用 `package-lock.json` 完成一次干净 `npm ci`，随后重新通过 `npm test` 和 `npm run build`；安装版验收仍单独保留。
- 在新增 Tileset 边界校验后重新执行 synthetic grid 4×4 的 `process-tileset`，再次完成 scan→rebuild→validate→commit，退出码为 0，临时输入/输出随后清理。
- 使用候选 runtime 的 `top_rebuild.exe` 重新执行 processor synthetic grid 4×4 smoke，确认 packaged runtime 解析、重建、校验和提交链路均返回 0。
- 在 Visual Studio 18.6.2/MSVC 14.51.36231 和 vcpkg `x64-windows` 依赖树上完成 converter Release 原生构建；加入 OSG UTF-8 路径和相对 URI 修复后的候选 `_3dtile.exe` SHA256 为 `7da792b3ecc51176bc829cb317da97c77f86fbfd34806c41ceb471b0e47fe2e7`。
- 用最终候选 runtime 生成 NSIS 安装包，静默安装到临时目录后确认 `_3dtile.exe`、processor、OSG 插件、GDAL/PROJ 数据均存在；安装后的 converter `--help` 返回 0，真实 OSGB 中文输入/输出目录转换返回 0，URI 引用可解析，随后卸载并清理临时目录。安装包 SHA256 为 `04c2cb84c94a18692945fd33d9628d99074bdd00f2cd53b1cbdebb2c19359da0`；本机 converter zip SHA256 为 `81da5566a1e883eb5b07f45d1f06f93e34d715637d0f5ecd0f7f765b8111cabf`。
- 修复 converter 在 `OSG_USE_UTF8_FILENAME=ON` 下错误转入系统代码页的问题；真实 `OSGBny`（78 个 Tile、约 14 MB）放在中文输入根目录时，候选 converter 直接转换和 processor 的 scan→convert→validate→commit 均退出 0，中文/空格输出目录中的 `tileset.json` 和 85 个成果文件可读。
- 对同一真实成果执行 `rebuild-top` 时，因其布局不是规则网格而明确返回 `GRID_SPATIAL_MISMATCH`，退出码 1 且不提交输出；这是布局约束的预期拒绝，不是崩溃。
- 在同一真实 `OSGBny` 上连续执行 10 次 `convert-osgb`，10/10 返回 0 并生成完整成果；日志记录耗时 1627–1729 ms、峰值工作集 34,897,920–37,687,296 字节，汇总见 `.cache/repeat-osgbny/summary.json`。
- 对约 932 MB、4567 个 Tile 的真实样本执行 processor CLI 取消回归：约 1200 ms 后发送 `cancel`，返回 `CANCELLED`（退出码 2），最终输出和临时目录均不存在，未发现残留 processor/converter 进程；完整桌面关闭/Job Object 仍需 Windows 安装版验收。
- 对已存在成果目录执行输出安全回归：返回 `PATH_OUTPUT_EXISTS`（退出码 1），sentinel 内容保持不变且未创建 `tileset.json`；使用极高 `GEOFORGE_LOW_DISK_BYTES` 做受控阈值回归时发出 `LOW_DISK_SPACE` warning，仍能完成小样本提交（详见回归 22–23）。
- 将输出父路径设为已有普通文件时，processor 返回 `PATH_OUTPUT_NOT_WRITABLE`（退出码 1），未创建子目录且原文件内容保持不变；真实 ACL 拒绝写入仍需安装版测试（回归 25）。
- 扫描负向 fixture：缺失 `metadata.xml` 与未闭合 XML 均在转换前返回 `valid=false`、退出码 1，并保留具体路径/解析错误（回归 26–27）。

验证命令：

```text
cargo check                         通过
cargo test -p geoforge-protocol     通过
cargo test                            90 项通过（16 suites）
cargo test -p processor --lib        27 项通过
desktop Tauri cargo test --lib      7 项通过
npm test                             通过（纯函数回归）
npm run build                        通过
desktop Tauri cargo check (Windows target) 通过
cargo fmt --all -- --check           通过
git diff --check                     通过
```

## 尚未完成或受环境限制

- converter 源码的 `cargo metadata --no-deps` 和 `git diff --check` 通过；相邻仓库 `cargo fmt --all -- --check` 仍包含已有的 `common.rs`/`shape.rs`/`build.rs` 格式差异，未自动重排。
- 固定 converter runtime 的 URL/SHA256 尚未更新：本机候选二进制只用于验收，必须先在 converter 仓库发布带版本号的 zip，再更新主仓库清单。
- 真实中文输入根目录已通过；本地 Cesium 页面已加载真实 `OSGBny` 成果并显示 `tilesLoaded`（回归 24）。中文 Tile 文件名、长路径、权限/磁盘故障和安装桌面 WebView 仍待端到端回归。
- 当前真实失败样本尚未由用户提供；现有大样本仅完成取消/收尾验证，未宣称完整转换成功。
- 单 Tile 容错保持严格失败，未启用部分成果和失败率阈值。
- 原始诊断文件目前按任务保留，尚未增加自动轮换/配额策略；任务事件日志仍按 tail 规则限长。
- `cargo fmt --all` 和前端 build 生成了工作区中若干已有格式化/构建产物改动，尚未清理，以避免覆盖用户未提交状态；这些文件不应混入本轮功能提交。

Hardening follow-up:
- Converter CLI now returns a non-zero exit code when malformed input or a missing final artifact is detected.
- OSGB scanning now reports directory read errors and covers UTF-8 dataset paths, missing tile entries, and non-standard directories.
- Product and Windows CI now build the desktop frontend and run the Tauri library smoke test.
- Full converter validation now has a local MSVC/vcpkg candidate build; real OSGB conversion and CJK input/output paths pass locally, while release publication remains pending.
- Desktop Windows-specific Job Object code has passed `cargo check --target x86_64-pc-windows-msvc`; runtime kill-on-close behavior still needs an actual Windows process-tree test.
- 旧固定 runtime 的无效输入负向测试仍返回退出码 0；本机候选 runtime 已返回非零并完成临时 NSIS 安装验收，不能把旧固定 runtime 当作本轮修复证据。


