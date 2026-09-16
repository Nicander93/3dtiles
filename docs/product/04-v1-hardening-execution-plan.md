# GeoForge V1 可用性与健壮性执行计划

日期：2026-09-15。初始审阅基线：主仓库 `4b72121`、相邻 converter `9f0629b`；本轮实施提交：主仓库 `e23d604`、converter `76ef4d6`。

本计划结合用户提供的《03-v1-hardening-and-release-plan-v2.md》、崩溃建议截图及当前源码编制。附件中的实现建议作为待核对材料，不代表已经验证的原因。随后已按任务卡执行主仓库和桌面端的可重复验证，并完成 H02–H08、H11–H15 的代码改动；H07/H08 已在本机 MSVC/vcpkg 环境构建出候选 converter，修复 OSG UTF-8 输入路径，并完成真实 OSGB、10 次重复、本地 Cesium 页面和临时 NSIS 安装验收。正式发布、安装桌面 WebView、桌面关闭/Job Object 及当前用户失败样本仍单独列为待完成项，详见 `HARDENING_PROGRESS.md` 与 `RELEASE_ACCEPTANCE.md`。

## 1. 这轮要交付什么

先做到：正常数据可以从安装版完成转换；失败能说明阶段、原因和日志位置；取消、中断、重试不破坏输入及既有成果；长日志不会拖垮桌面端。然后扩大真实数据覆盖范围。

建议分三次交付：

1. **第一批：安全与诊断。** H01–H06。修复明确的问题，取得当前失败样本的可靠诊断证据。
2. **第二批：转换稳定与安装验证。** H07–H10。修 converter、更新固定 runtime、验证安装版。若出现原生崩溃，执行 H09 的独立排查分支。
3. **第三批：长任务与发布门槛。** H11–H15。补齐资源、预检、校验和回归。H16 的部分成果容错另行评审，不作为首轮交付要求。

任务按编号执行；每张任务卡一个独立改动，不让执行 agent 一次包办整个阶段。安全相关任务及 C++/FFI 任务需逐项复核。

## 2. 相比原计划，需要修正的事实

| 主题 | 当前代码证据 | 本轮决定 |
| --- | --- | --- |
| UI 未知错误 | `apps/desktop/src/api/client.ts:361` 的 `friendlyError` 只接收 Error 类 | 仍需修复 string、普通对象和空消息 |
| 输出选择冲突 | 两个转换页面已有 `suggestOutputPath`，但选择器直接把已存在父目录写入 `output` | 修选择行为及路径预览，复用现有函数 |
| 子进程日志 | `crates/processor/src/util.rs` 当前 stdout 为 null、stderr 为 pipe；返回 i32；按 UTF-8 行读取并 flatten 错误 | 不能按“当前双管道”设计；先补可靠日志和结果摘要 |
| 保留具体错误 | `process_manager.rs` 的 scheduler 和 `apply_event` 已有保护 | 补回归，不重写一遍；补丢失的失败阶段 |
| 临时目录/提交 | `stages/commit.rs` 已使用输出同级 `.geoforge-task-<id>`，pipeline 已校验后 no-replace 提交 | 保留架构，重点修临时目录归属、清理和失败留存 |
| 失败留存 | `pipeline.rs::run_task` 普通失败和取消都调用清理 | 当前并非失败保留；日志应独立于工作目录保存 |
| 中断恢复 | `task_store.rs::mark_stale_interrupted` 已实现，`state.rs` 已调用 | 不新增状态；验证子进程确实终止及队列恢复 |
| Unicode 崩溃 | 相邻 converter 的 `src/osgb.rs:159` 仍是 `String::from_utf8(...).unwrap()`，后续 JSON 解析也 unwrap | 不只是改一个 unwrap；需要完整失败传播 |
| C++ 异常 | converter `src/osgb23dtile.cpp:869,1011` 仍有实际执行的 exit(1) | 普通数据错误逐层返回，CLI 顶层非零退出仍是合理行为 |
| 静默少块 | converter 遇空指针仅打印日志，随后发送空 JSON；汇总会忽略空 JSON | 首轮改为任务明确失败，防止缺块仍成功 |
| 中文 Tile | processor 扫描只接收 `Tile_数字_数字`；converter 则寻找目录内同名 OSGB | 仅修 URI 编码不能保证中文 Tile 通过桌面链路；转换与重建的布局约束必须分开 |
| 测试基础 | path_policy、validate 等已有单测；前端 package.json 没有 test 脚本；Tauri 不在根 workspace 内 | 复用已有测试，桌面 Rust 单独执行，不能写“npm test 已通过” |
| runtime 更新 | `third_party/3dtiles-converter.json` 固定 v0.1.0 的 URL 和 SHA256 | 修相邻源码不等于安装版已更新，必须独立完成 runtime 集成 |

### 本次额外发现的优先风险

- **日志切片可能 panic：** `task_store.rs:273` 用字节偏移截断 UTF-8 字符串，中文或 emoji 落在切点时可能崩溃。
- **任务记录有丢更新风险：** `update_fields` 和 `append_log` 分别读取整条记录再 upsert；读写各自加锁，整个读改写不在同一临界区。并发日志、取消、状态事件可能覆盖彼此字段。
- **tail 并非尾部读取：** `get_logs` 先 `read_to_string` 整个文件再取末尾行；大日志轮询仍会产生大量 I/O 和内存分配。
- **临时目录清理不证明归属：** `prepare_temp` 删除所有同名旧目录；错误收尾从原始 output/taskId 重新计算路径。路径校验失败也可能进入清理。还应覆盖 input 恰等于派生临时目录的场景。
- **组件探测存在假阳性：** `capabilities.rs::probe_bin` 把任意可用退出码视为 launchOk；超过 3 秒被 kill 后也设置 ok=true。
- **强杀恢复尚不能由已有 Job Object 推导为安全：** 当前创建并分配 Job，但没有发现 kill-on-close 配置；存在分配失败退化和启动后分配的窗口。需实测孤儿进程。
- **既有实数回归不等于安装版通过：** `REAL_DATA_VALIDATION.md` 的部分转换基线来自 Docker，不能代替本轮 Windows 固定 runtime 端到端验证。

以上为源码审阅发现的可达风险或证据缺口，未声称已经在本机复现所有故障。

## 3. 任务卡

### H01 / P0：锁定复现基线

**范围：** 新增 `docs/product/HARDENING_REGRESSION.md`；核查固定 converter 清单、实际安装路径和样本。

**执行：**

1. 记录主程序版本/commit、processor 实际路径、converter 实际路径及 SHA256、runtime 根、系统和样本编号。
2. 记录现有失败数据的输入方式、参数、是否重建、退出码、日志；同一输入每次使用新的输出目录。
3. 准备一个已知可转换的小样本作为对照。真实数据不加入 Git；缺样本时标为“待提供”，仍继续 H02–H06。
4. 将“开发 CLI”“开发桌面”“安装桌面”分为三列，不能混用验证结果。

**完成标准：** 他人能根据记录重复同一个实验。没有完整日志时不填写根因。

### H02 / P0：修复中文日志 panic

**范围：** `apps/desktop/src-tauri/src/task_store.rs::append_log`。

**执行：** 用安全 UTF-8 边界截断内存日志，保留约 200 KB 的现有策略；单条超长日志也必须受限。不在此卡改数据库结构或 tail 算法。

**验证：** ASCII、中文、emoji、恰好到阈值和单条超限；输出仍为合法 UTF-8、不 panic，任务磁盘日志不因内存截断而丢失。

### H03 / P0：阻止日志写入覆盖任务状态

**范围：** `task_store.rs`，重点 `update_fields`、`append_log`、`request_cancel`。

**执行：**

1. 用测试构造“读取旧记录→另一方取消/更新错误→旧记录写回”的交错。
2. 将同一记录的读改写置于一个数据库锁/事务边界内，或按字段更新；复用私有数据库辅助函数，避免在已持锁时再次调用会加锁的公开方法。
3. 日志写入只更新日志相关字段。文件写失败必须可报告，不能递归调用 append_log 报自己的错误。

**验证：** 多线程追加日志同时取消、应用错误和完成事件，cancelRequested、error、终态均不回退。测试使用 barrier/受控交错，避免靠 sleep 碰运气。

### H04 / P0：临时目录必须确认归属

**范围：** `crates/processor/src/path_policy.rs`、`stages/commit.rs`、`pipeline.rs`。

**执行：**

1. 校验 taskId 和规范化 input/output 后，显式校验派生工作目录与输入、最终目录互不重叠。
2. 新任务创建临时目录使用独占创建；已有目录第一版直接报错，不先删除。加入任务归属标记。
3. pipeline 持有本次成功创建的工作目录上下文；失败清理只使用该上下文，禁止在外层错误分支从原始参数重新拼删除路径。
4. 普通失败保留工作目录，并记录位置；取消仅清理本次目录；成功清理剩余空壳。日志放在独立任务日志位置。
5. 清理检查规范化边界和归属；拒绝链接/junction 指向其他目录的情况。若需要更复杂的防竞态设计，单独提交设计，不让 agent 猜。

**验证：** 全部在隔离测试目录：非法 taskId、无效输入、预先存在同名目录、input=workdir、提交时 output 被其他方创建、普通失败和取消。输入与旧目录放 sentinel 文件，逐一确认内容仍在。保留既有 no-overwrite 测试。

**不做：** 扫描磁盘后按 `.geoforge-*` 通配符删除；跨盘 copy 提交；断点续转。

### H05 / P0：用户能正常选择输出位置并看懂错误

**范围：** `src/api/client.ts`、`src/lib/formUtils.ts`、`src/pages/OsgbConvert.tsx`、`src/pages/ProcessTiles.tsx`（均在 apps/desktop 下）。

**执行：**

1. friendlyError 支持非空 string、Error、`{message: string}`，无有效文本才使用兜底；不把对象变成 `[object Object]`。
2. 两页输入/输出选择器都捕获失败并显示错误；用户取消选择保持原值，不报错。
3. 输出选择器选择父目录，复用 suggestOutputPath 生成最终子目录；保留可编辑的最终路径，显示“输出位置”和“最终成果目录”的区别。
4. 手工输入、默认输出根、重新执行、localStorage 历史值保持最终 output 语义；禁止重复追加 `_tiles`。首版重名明确提示改名，不做扫描自动编号。
5. 后端仍保留 no-overwrite；前端预检查不能替代提交时再次检查。

**验证：** 两页都检查选择/取消/选择失败、中文/空格、根目录/UNC、选中已有父目录、最终目录重名、重新打开表单和提交参数。复查提交后的自动建议不会被 useEffect 改回旧路径。

### H06 / P0：把 converter 的真实失败传到任务详情

**依赖：** H02–H04。

**范围：** `crates/processor/src/util.rs`、`stages/convert.rs`；桌面 `process_manager.rs`、`Processing.tsx`；必要时扩展现有 `progress` 字段，不更换数据库 schema。

**执行：**

1. 为 converter 增加有界命令结果：退出码、stderr 摘要、诊断日志路径。建议摘要同时限 100 行与 64 KiB。
2. 第一版保留其他工具的 run_logged 返回接口，新增 converter 专用入口或包装，避免顺带大改 rebuild/texture。
3. 为 converter 保存原始 stdout/stderr 到任务诊断目录的独立文件；不能改动 **processor→desktop 的 stdout JSONL 协议管道**。若保留实时预览，按字节读取文件的新增部分，并有界解码。
4. 日志解码失败不能 flatten 丢弃；允许显示层有损解码，但保留原始字节。这个例外不能用于 JSON URI、文件路径或业务数据。
5. 非零退出时显示退出码、阶段、最后有效诊断和日志位置；无 stderr 也明确报告。禁止把“没有尾部文本”解释为数据损坏。
6. 保留桌面层现有具体错误保护；失败前记录 failedStage，避免 scheduler 将 stage 改为 failed 后 UI 丢失出错环节。结果页提供日志定位/复制错误入口，复用已有打开路径能力。
7. 所有 reader、进程等待在失败和取消时均有收尾路径；不能因仍持有句柄而无限等待。

**验证：** 用可控假 converter 覆盖退出 0、非零、仅 stdout 错误、空 stderr、非 UTF-8、超长单行、持续输出、运行中取消；确认具体错误不被 processor exited 覆盖，日志落在临时工作目录之外。

**证据边界：** 写文件是增强诊断并检验截图假设的实施选择，不是已证明的原生崩溃修复。

### H07 / P0：converter 的普通失败完整返回

**仓库：** `D:/code/geoforge-converter`；不能在主仓库寻找已移走的 engines 目录。

**范围：** `src/osgb.rs`、`src/osgb23dtile.cpp`、必要的 `src/extern.h`。分两次改动：Rust 汇总；C++ 错误传播。

**执行：**

1. 每个并行 Tile 得到 Success/Failure；优先使用并行 collect 汇总，或保证 channel 每个任务恰有一个结果且正确关闭。不能只移除 send 后仍按旧 task_count 阻塞接收。
2. null 指针、非法/空 JSON、无效 bbox、UTF-8 错误、写文件失败必须归入失败。校验 FFI 返回长度，明确所有路径上缓冲区释放一次。
3. Rust 汇总不再对外部 JSON 和 bbox unwrap；首版任一 Tile 失败则整体失败，不能从根节点悄悄删除后仍成功。零成功 Tile 必须失败。
4. 两处 C++ 普通数据 exit(1) 改为逐层传播至 Tile 失败；带 Tile 路径和 primitive 类型。不能单独改成 return false 却不检查调用方。
5. C++ 异常不得越过 extern C 边界；对异常和无效返回分别转为明确失败。硬件级崩溃另走 H09，不能靠 Result 保证捕获。

**验证：** 非法 UTF-8、非法 JSON、空返回、空几何、未知 primitive、损坏输入和写失败；CLI 非零且错误包含对象，不 hang、不报告完整成功。已有 ASCII 小样本仍通过。

### H08 / P0：修 URI 编码与 JSON 转义

**依赖：** H07；仓库与 H07 相同。

**范围：** converter `encode_tile_json` 及其路径辅助函数。

**执行：**

1. 沿输入路径→OSG 路径→实际输出文件名→JSON URI 核对编码；现有 `utf8_string` 已存在，优先审查复用，避免重复转换。
2. 文件系统访问和 JSON URI 的表示分开；URI 经 JSON serializer 转义，不手工加引号。不把 Windows 当前代码页无法表示的字符悄悄替换成问号。当前候选 OSG 构建启用了 `OSG_USE_UTF8_FILENAME`，因此传给 OSG 的路径必须保持 UTF-8。
3. 明确 JSON 转义与 URI 转义是不同问题；含空格、`%`、`#` 等场景要从 URI 真正解析并访问到文件，不能只验证 JSON 能 parse。
4. 第一版保留现有函数结构，不重写整个转换器。若未来更换为未启用 `OSG_USE_UTF8_FILENAME` 的 OSG 构建，必须先明确记录支持范围或在任务开始前拒绝不兼容路径。

**验证：** converter CLI 下 ASCII、中文数据根、中文 Tile/文件名、空格、括号等合法 Windows 名称；引号/反斜杠用纯字符串序列化测试，不创建 Windows 不允许的文件名。逐条确认输出 URI 对应文件且预览加载成功。

**注意：** 中文 Tile 的桌面端验收还依赖 H13。若启用重建后不满足规则网格，应明确拒绝，不能宣称“所有 Unicode 布局已支持”。

### H09 / 条件分支：排查安装环境原生崩溃

**触发：** H06 取得日志后，当前失败样本仍出现进程异常终止。

**执行：**

1. 对同一二进制/hash、参数、cwd、DLL/插件目录、环境和输入，比较 CLI、开发桌面、安装桌面。每次使用全新输出，记录每次结果。
2. 一次只改变一个因素：日志输出方式、并发数、安装路径等；记录试验组/对照组各次结果。先用少量重复排查，随后对候选修复做至少 10 次重复，不将 1 次成功视作稳定。
3. 收集可用的 Windows 故障模块/异常记录，必要时由熟悉 native 调试的人分析 dump。截图本身没有足够证据证明管道是根因，也不能证明“茅台数据失败”与其他失败同因。
4. 优先修根因。自动重试只作为**有证据后的可选缓解**：最多额外 1 次，只针对已确认错误白名单，保留首次日志；新 attempt 工作目录；取消、编码/数据错误、磁盘/权限错误、提交失败均不重试。
5. 重试成功仍记录发生过异常，计入发布回归结果；禁止循环重试掩盖稳定性问题。

**完成标准：** 输出对照记录、结论的证据强度及修复验证。仍无法解释崩溃时报告为发布阻塞项，agent 不自行引入重试策略。

### H10 / P0：让修复真正进入安装包

**依赖：** H07/H08，以及触发时的 H09。

**范围：** converter 的构建/发布入口；主仓库 `third_party/3dtiles-converter.json`、`apps/desktop/scripts/prepare-converter.ps1`、`prepare-runtime.ps1`、`package-windows.ps1`。

**执行：**

1. 先构建修复后的 converter 测试产物，记录 source commit 与包 SHA256；正式发布动作遵守当次任务授权。
2. 消费新版本的固定 URL、版本、SHA256，禁止主仓库改用 latest 或关闭校验。
3. 清洁 staging 或验证完整文件清单，避免 Copy-Item 合并残留旧 DLL/旧 converter；删除只限已确认的构建 staging，不碰安装目录或用户数据。
4. 安装后核查实际加载二进制和插件来源；清除测试进程的开发覆盖变量进行验证，不改用户全局环境。
5. 对照 release workflow 当前 `-SkipTextureBundle`：以转换+重建为首版范围，未打包纹理工具时 UI 必须准确禁用相关能力。

**验证：** 安装路径含空格、中文用户/数据路径、无开发工具和无源码目录的干净 Windows 环境；转换和重建分别验证。记录 installer hash、runtime hash、输出校验和预览结果。

### H11 / P1：可靠取消与强杀后的进程收尾

**范围：** 桌面 `process_manager.rs`、`state.rs`；processor `cancel.rs`、`pipeline.rs::copy_dir`、`util.rs`。

**执行：**

1. 保留已有 interrupted 恢复和串行队列，补队列取消、运行取消、强杀桌面/processor/converter 的回归。
2. Windows Job Object 配置和验证 kill-on-close；记录 Assign 失败，不静默当作进程树已受控。审查启动到加入 Job 的窗口，必要时拆独立 Windows 实现任务。
3. copy_dir 增加取消检查；大文件复制必要时分块检查，避免复制很久后才响应。
4. 测试 commit 前、rename 后/result 前、result 后三种取消或崩溃窗口。已提交成果不能删除；磁盘已提交但状态未登记时，使用已有 manifest 核对并提供恢复登记或明确提示，不重新覆盖输出。
5. 只对核验归属的 interrupted/failed 工作目录提供手动清理；不新增断点续转。

**验证：** 取消后无遗留进程写盘，队列下一任务能启动；强杀后重开不显示 running；提交成果始终保留。建议取消响应目标 2 秒内，强制收尾按现有超时并记录实际耗时。

### H12 / P1：日志 tail 真正有界

**依赖：** H02/H03/H06。

**范围：** `task_store.rs::get_logs`、`commands.rs::get_task_logs`；必要时任务列表查询。

**执行：** 从文件末尾分块读，建议单次最多 256 KiB，返回末 500 行；处理读取块切开 UTF-8 字符的情况。夹紧调用方 tail，约定 tail=0 也不无限读。全量日志通过打开文件访问；任务列表避免反复传输每项的完整日志文本。

**验证：** 空文件、短文件、中文跨块、无换行超长行、读取同时追加、至少 100 MB 日志；给出读取字节量/延迟和 UI 响应证据，而非只看返回 500 行。

### H13 / P1：预检与真实支持范围一致

**范围：** `stages/scan.rs`、`geo.rs`、`stages/rebuild.rs`、`capabilities.rs`；必要时 `convertReady.ts`。

拆为三个小任务：

1. **探测可信。** --help 超时、非零、崩溃不能 launchOk=true；返回原因和退出码。若某组件 help 正常就非零，先核对固定版本行为，不能泛化允许所有非零。
2. **扫描可信。** read_dir/metadata 失败必须报告路径；明确 XML 解析错误与缺 SRS 的区别。扫描设置时间/数量边界，估算统计标注为估算；不能返回被截断的“完整 Tile 数”。
3. **布局约束分离。** convert-only 以 converter 可发现的同名根 OSGB 为依据；非标准目录不直接忽略。rebuild 继续要求实际支持的规则布局。缺 metadata+手工 CRS 当前仍被 scan 拦截：首版保持明确拒绝，支持该组合需另做 converter 参数验证，不能只移除前端限制。

**验证：** 中文/非标准目录 convert-only、规则网格 rebuild、稀疏布局明确提示、损坏 XML、不可读目录、数字超出 i32 的 Tile 名、组件缺失/超时。坐标解析不以 0 替代非法值；范围计算使用检查运算。

### H14 / P1：磁盘、内存与提交校验

拆三个任务，分别验收：

1. **文件系统：** 输出父目录小文件 create/write/flush/delete 预检；保留真正运行时的写失败处理；剩余空间只能做提示，不保证容量足够。磁盘满、权限、占用错误包含具体路径。受控故障注入，不填满用户系统盘。
2. **资源：** converter 增加集中线程配置，默认值先测量再定，至少保证 >=1；记录 threads、峰值内存、时间和临时空间。环境变量须在 converter 实际消费并打进 runtime，不能只在 processor 设置名字。
3. **校验：** 复用 `stages/validate.rs`，补非法字段类型、深层 children、缺资源、URI 编码、真正引用环与重复合法引用的区别。大 GLB/b3dm 校验限制内存分配。任何校验失败不得 commit；已有验证不代表完整 3D Tiles 标准合规。

**数据验证：** 从小样本→已有约 10 GB 级数据→目标大数据逐级推进，记录每级耗时、峰值和限制。超长路径/NAS 必须成功或给出可诊断拒绝；不在本轮承诺所有存储环境都支持。

### H15 / 发布门槛：回归结果进入 CI 和安装验收

**范围：** `.github/workflows/product.yml`、Windows 测试 workflow、`HARDENING_REGRESSION.md`。

**执行：** 增加 Windows 产品测试和独立 Tauri 测试/构建；前端 build 及已建立的小测试；保留 Linux 核心检查。打包流水线不能只产出安装器，还要具备固定小样本 smoke 或明确的人工安装验收步骤。

**必须通过：**

- ASCII 与中文路径的支持范围内样本，convert-only 和 convert+rebuild 分开记录。
- 当前用户失败样本得到解决，或被明确归类为不支持且在长任务开始前给出原因；若是宣称支持的数据仍崩溃，阻塞发布。
- H02/H03 日志稳定，H04 文件安全，H06 诊断，H11 取消与中断均通过。
- 输出通过内部校验并在 Cesium 中实际加载，检查定位、缺块、纹理、LOD；不能只检查 tileset.json 存在。
- 安装版用实际发布 runtime 验证，重复转换没有未解释的偶发失败；明确大小/布局验证范围。
- 每项写“通过/失败/阻塞/未执行”，附产物和日志位置；不得把未执行写为通过。

### H16 / 后续评审：允许部分 Tile 失败

**本轮默认：严格失败。** 不直接采纳原文默认 1% 阈值。

原因：当前重建约束规则网格，跳过 Tile 可能破坏后续重建；Tile 数百分比也不代表缺失面积或影响。原文“比例阈值加绝对阈值”的 AND/OR 语义不够确定，不适合让弱 agent 自定。

要开启时，先完成可审阅规格：用户显式选择允许部分成果；精确的可恢复错误白名单；总数定义、阈值运算和边界样例；失败清单；所有失败必须失败；根节点不引用失败对象；重建如何处理空洞；UI 和成果清单明确缺失范围。此后再分 converter 汇总、processor 传递、UI 告警三张任务卡实施。

## 4. 验证命令与执行边界

以下是执行 agent 修改后应使用的命令；本轮已运行的结果记录在 `HARDENING_PROGRESS.md` 与 `HARDENING_REGRESSION.md`。真实 OSGB 的候选 runtime/临时安装、10 次重复和 processor CLI 取消结果已记录；Cesium、桌面关闭/Job Object 和正式发布仍需按清单执行。

主仓库根 `D:/code/3dtiles`：

```powershell
rtk cargo fmt --all -- --check
rtk cargo test -p geoforge-protocol
rtk cargo test -p processor --lib
```

涉及 top_rebuild 时另外运行：

```powershell
rtk cargo test -p top_rebuild
```

涉及桌面 Rust 时，必须单独覆盖（不属于根 workspace）：

```powershell
rtk cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
rtk cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
```

前端目录 `D:/code/3dtiles/apps/desktop`：

```powershell
rtk npm run build
```

前端已有无外部依赖的 `npm test` 纯函数回归入口；涉及表单或错误文案时必须运行它。构建需要 sidecars/resources 时按现有准备脚本补齐并记录；不要为让测试通过伪造 runtime。converter 的构建需遵循其自身仓库说明，主仓库 cargo test 不会覆盖 OSG/C++。

只为改动运行对应测试；依赖缺失、编译失败与功能测试失败分开报告。本次仓库已有 `apps/desktop/src-tauri/Cargo.lock` 修改和未跟踪的 `windows-numerics.crate`，执行 agent 不应覆盖、删除或混入本轮提交。

## 5. 可直接交给执行 agent 的提示词

> 阅读 docs/product/04-v1-hardening-execution-plan.md，只执行任务 Hxx。先核对当前源码和该卡依赖，列出将修改的函数；已完成内容只补缺失验证。一次只做一个明确行为，不顺带重构。测试使用隔离目录和小 fixture，禁止在用户真实输入上注入故障。修改后运行该范围对应检查，输出：变更文件、行为变化、验证命令及结果、尚未覆盖的场景、下一卡是否可开始。未经当前任务授权不要推 tag、发布安装包或更新外部 release。

遇到以下情况提交具体问题和可选方案，不自行扩大实现：需要改数据库/protocol schema、支持新的 Tile 布局算法、改变默认容错/重试策略、无法保证临时目录归属、需要 native dump 深度分析。其余已授权的小修复可直接完成。

**推荐第一个执行指令：完成 H01 的基线表并执行 H02。** 随后 H03、H04，再进入表单与 converter 诊断链。这样每次交付都能单独验收，也不会让能力较弱的 agent 同时处理 UI、状态竞争和 C++ 编码。
