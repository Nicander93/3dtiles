# V1 Hardening 回归记录

这份记录只保存可复现实验的元数据和结果，不提交真实 OSGB 数据。

## 环境基线

| 项目 | 值 |
| --- | --- |
| GeoForge commit | `0fef91e9cd12e182db7b7e9215f78cb342328682` |
| GeoForge 版本 | `0.1.0`（仓库/桌面配置） |
| Processor 路径 | `D:\\code\\3dtiles\\target\\release\\processor.exe` |
| Converter 版本 / source commit | `0.1.0` / `a464f0b8e89c13ddfbf0f50af84ea153cdc90b0b`（本机候选构建） |
| Converter SHA256 | 本机候选 runtime `_3dtile.exe`: `40015f4d776db5acb187a8537e08905a8a47efd80e3c771d07a9bdb24fa6a32e`; 固定发布清单值为 `77cadd01941add0297a52a22ce26918d8d22c3729a0ae564556fce14327b5c5b`，两者不一致，不能宣称已更新正式发布包 |
| Runtime 根目录 | `D:\\code\\3dtiles\\dist\\runtime`（本机候选，converter `--help` 返回 0；已完成临时 NSIS 安装验收） |
| Installer SHA256 | 本机最新候选 `GeoForge 3D_0.1.0_x64-setup.exe`: `ef3c0589d75524b5a8fda92f5397870b7f06a8f9ee86e292f881ae1a3aac7e79` |
| Windows 版本 | 未能读取 WMI；本记录不伪造版本 |
| CPU / 内存 | 未能读取 WMI；本记录不伪造硬件数据 |

## 实验记录

每次实验使用新的输出目录，开发 CLI、开发桌面和安装桌面分开记录。

| 编号 | 应用形态 | 输入样本 | 操作/参数 | 输出目录 | 结果 | 退出码 | 日志位置 | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | 待填写 | 小型 ASCII 对照样本 | convert-only | 待填写 | 待执行 | — | — | — |
| 02 | 待填写 | 当前用户失败样本 | 与原问题相同 | 待填写 | 待执行 | — | — | — |
| 03 | 开发 CLI | 无输入；能力探测 | `target/debug/processor.exe capabilities --json` | — | 通过 | 0 | stdout JSON | converter/top_rebuild `--help` 返回 0；本机 Python 脚本可启动但缺少 basisu，纹理模式标为不可用；未替代真实转换 |
| 04 | 本机旧 runtime CLI | 不存在的 OSGB 路径 | `_3dtile.exe -f osgb -i missing -o <temp>` | 临时目录 | 失败但错误地返回成功码 | 0 | 临时 stderr（已清理） | 固定发布 runtime 尚未包含本轮 converter 非零退出修复 |
| 05 | 开发 CLI | 仓库 `grid_4x4` 3D Tiles fixture | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | 临时任务目录 | 阻塞 | 1 | processor JSONL | fixture 中引用的 `.b3dm` 文件为 0 字节，top_rebuild 明确返回 `CONTENT_MISSING`；不能替代真实 OSGB/安装版验收 |
| 06 | 开发 CLI | 隔离 synthetic grid 4×4（以有效 B3DM 替换零字节占位） | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | `.tmp-hardening-grid4-output` | 通过 | 0 | processor JSONL | 完成 scan→rebuild→validate→commit；输出含 `tileset.json`、`rebuild_metrics.json` 和 4 个 proxy L1/1 个 proxy L2；不是 OSGB 验收 |
| 07 | 开发前端 | 纯函数回归 | `npm test` | — | 通过 | 0 | npm stdout | 覆盖错误文本、输出目录建议、路径比较和真实进度计算；不替代安装版验收 |
| 08 | 开发前端 | 按锁文件干净安装 | `npm ci --no-audit --no-fund`（临时工作区缓存） | — | 通过 | 0 | npm stdout | 随后重新执行 `npm test` 与 `npm run build`；未修改 package-lock；不替代安装版验收 |
| 09 | 开发 CLI/Tauri | 健壮性回归 | `cargo test`；Tauri `cargo test --lib` | — | 通过 | 0 | cargo stdout | processor 27 项、workspace 90 项、Tauri 8 项；新增协议行上限、marker 归属、Tileset 边界、GLTF 路径越界和分块取消检查 |
| 10 | 开发 CLI | 隔离 synthetic grid 4×4（替换零字节 B3DM） | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | 临时目录（已清理） | 通过 | 0 | processor JSONL | 在新增 geometricError/boundingVolume/GLTF 越界校验后重跑；scan→rebuild→validate→commit 全部通过；不替代真实 OSGB/安装版验收 |
| 11 | 发布脚本 | PowerShell 语法与门槛检查 | `Parser.ParseFile` 检查 `prepare-converter.ps1`、`prepare-runtime.ps1`、`package-windows.ps1` | — | 通过 | 0 | PowerShell stdout | converter help 非零/超时现在阻止 staging；runtime manifest 含 SHA256；未执行真实 installer |
| 12 | converter 源码 | FFI 参数边界审查 | `cargo metadata --no-deps`；`git diff --check`；审查 `osgb23dtile_path`/`osgb2glb` | — | 通过 | — | — | 入口拒绝空指针、负层级、非有限坐标；JSON 返回检查空值、`int` 长度上限和分配失败 |
| 13 | converter 本机 Release CLI | MSVC/vcpkg 原生构建与启动（历史候选） | `cargo build --release`；`_3dtile.exe --help` | `D:\\code\\3dtiles\\dist\\runtime` | 通过 | 0 | stdout/stderr | 历史候选 hash `af7384f15059d6c3f6927f1aa754bffcf2036504fce3cb3628eb536c77e8e81e`；最新嵌套读取错误传播修复见回归 40/42 |
| 14 | converter 本机 Release CLI | 中文/空格路径与伪成功负向测试 | `-f osgb -i "中文 目录\\输入.osgb" -o "输出 目录"` | 临时目录 | 通过 | 2 / 1 | stderr | 不存在输入返回 2；空 OSGB 返回 1 且没有伪造 `tileset.json` |
| 15 | NSIS 临时安装 | 安装后 runtime 完整性、启动、真实中文路径和 URI | 静默安装 `/S /D=<temp>`；`_3dtile.exe --help`；真实 `OSGBny` 转换；检查 URI；卸载 | 临时目录（已清理） | 通过 | 0 | `help.log`、`unicode.log` | 安装包 hash `04c2cb84c94a18692945fd33d9628d99074bdd00f2cd53b1cbdebb2c19359da0`；输出 85 个文件、URI 6/6 可解析；未使用开发覆盖变量 |
| 16 | processor 本机 Release runtime | synthetic grid 4×4 重建回归 | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | 临时目录（已清理） | 通过 | 0 | processor JSONL | 使用候选 runtime/bin/top_rebuild.exe 完成 scan→rebuild→validate→commit；不替代真实 OSGB |
| 17 | converter 本机 Release CLI | 真实 `OSGBny`，中文输入根与中文/空格输出目录（历史候选） | `_3dtile.exe -f osgb -i "...\\中文 数据根\\OSGBny 输入" -o "...\\中文 输出修复\\OSGBny uri"` | `.cache\\中文 输出修复\\OSGBny uri` | 通过 | 0 | converter stdout/stderr | 78 个 OSGB、约 14 MB；生成 `tileset.json`，共 85 个文件、约 13.3 MB；根 URI 6/6 可解析；历史候选 hash `af7384f15059d6c3f6927f1aa754bffcf2036504fce3cb3628eb536c77e8e81e`；最新候选 hash 见回归 40 |
| 18 | processor 本机 Release runtime | 同一真实 `OSGBny`，中文输入/输出目录 | `processor.exe convert-osgb --input ... --output ... --task-id unicode-uri-osgb` | `.cache\\中文 输出修复\\OSGBny processor uri` | 通过 | 0 | processor JSONL | 完成 scan→convert→validate→commit；记录 `converter.elapsedMs=1626`、`converter.peakMemoryBytes=35676160`，根 URI 6/6 可解析，输出目录已提交 |
| 19 | processor 本机 Release runtime | 对真实 `OSGBny` 成果执行规则网格重建 | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | 临时任务目录（失败后未提交） | 预期拒绝 | 1 | processor JSONL | 明确返回 `GRID_SPATIAL_MISMATCH`；样本为稀疏/非规则布局，不视为 converter 崩溃 |
| 20 | processor 本机 Release runtime | 真实 `OSGBny` 重复稳定性 | `convert-osgb --input OSGBny --output run-XX --task-id repeat-XX`，连续 10 次 | `.cache\\repeat-osgbny\\run-01..run-10` | 通过 | 0（10/10） | `run-*.log`、`summary.json` | 每次均生成 `tileset.json`，86 个文件；耗时 1627–1729 ms，`converter.peakMemoryBytes` 34,897,920–37,687,296；未出现偶发失败 |
| 21 | processor CLI | 大样本取消与收尾（`hk_11-NW-10B/osgb`，4567 个 OSGB，约 932 MB） | `run --task task.json`，启动约 1200 ms 后向 stdin 写入 `cancel` | `.cache\\cancel-test` | 通过 | 2 | `stdout.jsonl`、`stderr.log` | 返回 `CANCELLED`；最终输出不存在、临时目录数为 0，残留 `processor`/`_3dtile` 进程数为 0；这是 processor CLI 证据，桌面关闭/Job Object 仍需单独验收 |
| 22 | processor 本机 Release runtime | 已存在的输出目录（sentinel） | `convert-osgb --input OSGBny --output existing` | `.cache\\safety-checks\\existing` | 通过 | 1 | stdout JSONL | 返回 `PATH_OUTPUT_EXISTS`；原有 `sentinel.txt` 内容保持 `keep-me`，没有创建 `tileset.json` |
| 23 | processor 本机 Release runtime | 小型 `OSGBny`，低磁盘阈值告警 | `GEOFORGE_LOW_DISK_BYTES=999999999999999999 convert-osgb ...` | `.cache\\safety-checks\\low-disk` | 通过（告警） | 0 | stdout JSONL | 发出 `LOW_DISK_SPACE` warning 与 `output.parentFreeBytes` metric，仍完成 validate→commit；该阈值仅用于受控告警回归，不代表真实磁盘已满 |
| 24 | 本地 Cesium 页面（候选 runtime 成果） | 回归 23 的真实 `OSGBny` 输出 | `cesium-preview.html?tileset=http://127.0.0.1:8765/.cache/safety-checks/low-disk/tileset.json` | — | 通过 | — | 浏览器 DOM/截图、控制台日志 | 页面显示 `tilesLoaded · view r≈959m`，Cesium 画布可见模型，控制台无 error/warning；这是本地网页预览证据，尚不等同安装桌面 WebView 验收 |
| 25 | processor 本机 Release runtime | 输出父路径实际为文件 | `convert-osgb --output safety-checks\\not-a-directory\\child` | `.cache\\safety-checks\\not-a-directory` | 通过（拒绝） | 1 | stdout JSONL | 返回 `PATH_OUTPUT_NOT_WRITABLE`，父文件和内容保持不变，未创建 child；真实 ACL 无写权限仍待安装版人工验收 |
| 26 | processor 本机 CLI | 缺少 `metadata.xml` | `scan-osgb --path .cache\\scan-negative\\missing-metadata` | — | 通过（拒绝） | 1 | stdout JSON | `valid=false`，错误明确包含 `Missing metadata.xml`，未进入转换 |
| 27 | processor 本机 CLI | 损坏 `metadata.xml` | `scan-osgb --path .cache\\scan-negative\\broken-metadata` | — | 通过（拒绝） | 1 | stdout JSON | `valid=false`，错误明确包含 `XML parse error: unclosed tag <SRS>`，未进入转换 |
| 28 | Desktop Rust Windows unit test | Job Object kill-on-close | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --locked` | — | 通过 | 0（8/8） | cargo stdout | 直接调用桌面实际 Job Object 绑定，关闭 Job 后 30 秒 `cmd/ping` 子进程在 3 秒内结束；不是完整 GUI 关闭验收 |
| 29 | NSIS 临时安装启动 | 默认用户 AppData 写入与窗口可见性 | 静默安装后启动 `geoforge-desktop.exe`；另以 `GEOFORGE_DATA_DIR=.cache\\installed-ui-data` 隔离启动 | `.cache\\installed-ui-test`（已清理） | 阻塞 | — | Tauri stderr / Process 状态 | 当前受限执行环境对用户 AppData 仅有读取权限，默认启动报 SQLite `attempt to write a readonly database`；隔离目录启动进程可驻留，但窗口未被自动化接口枚举，因此不宣称安装 GUI 验收 |
| 30 | processor 本机 Release runtime | 资源曲线（同一真实 `OSGBny`） | `GEOFORGE_CONVERT_THREADS=1,2,4 convert-osgb` | `.cache\\resource-curve\\threads-*` | 通过 | 0（3/3） | `threads-*.log` | 1 worker：3035 ms / 29,237,248 B；2：1629 ms / 35,753,984 B；4：1025 ms / 47,386,624 B；每次 86 个文件且 `tileset.json` 存在 |
| 31 | NSIS 临时安装 | 安装目录包含空格；ASCII 输入 `data\\real\\OSGBny\\OSGBny` | 安装到 `.cache\\installed path with spaces-20260916`，从安装目录 `resources\\runtime\\converter\\_3dtile.exe` 执行 convert-only；随后静默卸载 | `.cache\\installed path with spaces-output-20260916` | 通过 | 0 | `.cache\\installed-path-spaces-converter.log` | 安装器退出 0；安装版 converter 退出 0，生成 85 个文件和 `tileset.json`，根 URI 6/6 可解析；卸载退出 0，安装目录已移除；无残留 processor/converter 进程 |
| 32 | converter 本机 Release CLI | 输入和输出路径包含方括号、加号、括号；ASCII `OSGBny` | `_3dtile.exe -f osgb -i ".cache\\special [path]+(v1) input" -o ".cache\\special output [path]+(v1)"` | `.cache\\special output [path]+(v1)` | 通过 | 0 | `.cache\\special-path-converter.log` | 候选 converter 退出 0，生成 85 个文件和 `tileset.json`，根 URI 6/6 可解析；输入样本保持不变；无残留 processor/converter 进程 |
| 33 | processor 本机 Release runtime | 输入路径约 288 字符的真实 `OSGBny` | `processor.exe convert-osgb --input <long path> --output <new path> --task-id long-path-fixed` | `.cache\\long-path-20260916\\processor-fixed\\final output` | 通过（明确拒绝） | 1 | `.cache\\long-path-processor-fixed.jsonl`、`.cache\\long-path-converter.log` | scan 阶段通过；converter 阶段返回 `CONVERTER_EXIT_NONZERO`，stderr 指出具体 Tile 的 `open file/read node files` 失败；最终目录和 `tileset.json` 均不存在，仅保留 1 个归属明确的临时工作目录；无残留进程。当前候选明确不支持此长度，未宣称长路径兼容 |
| 34 | processor 本机 Release runtime | 已有连续香港 5×4 真实成果（4,585 个内容文件，约 1.23 GB） | `processor.exe process-tileset --input data\\real\\v1_e2e_convert_keep --output <new path> --rebuild-top --levels 0 --texture keep --task-id large-rebuild-current` | `.cache\\large-rebuild-current-20260916` | 通过 | 0 | `.cache\\large-rebuild-current-20260916.jsonl` | 当前 processor 完成 rebuild→validate→commit；耗时 29,034 ms，输出 4,596 个文件、1,237,277,271 bytes，root children=2、proxy=9、level 0/1/2/3=20/6/2/1；保留 `GAP_WARN`/`BUDGET_NOT_REACHED` 既有警告；无残留进程 |
| 35 | processor 本机 Release runtime（历史候选） | 香港 8×8 真实 OSGB 全量转换（含 62 字节空 OSGB 叶节点） | `GEOFORGE_CONVERT_THREADS=2 processor.exe convert-osgb --input data\\real\\hk_8x8\\osgb --output <new path> --task-id hk8x8-convert-fixed-t2` | `.cache\\hk8x8-convert-fixed-t2-20260916` | 通过 | 0 | `.cache\\hk8x8-convert-fixed-t2-20260916.stdout` | 历史候选输入 11,124 个 OSGB、约 2.47 GB；耗时 267,409 ms，converter 峰值 88,641,536 bytes；输出 11,186 个文件、3,107,088,112 bytes，root children=64，64/64 个根 URI 可解析；空叶节点记录 `skipping empty OSGB node` 后继续，最终 validate→commit 成功；最新候选复测见回归 40 |
| 36 | processor 本机 Release runtime（历史候选） | 香港 8×8 转换成果执行完整顶层重建 | `processor.exe process-tileset --input .cache\\hk8x8-convert-fixed-t2-20260916 --output <new path> --rebuild-top --levels 0 --texture keep --task-id hk8x8-rebuild-top-20260917` | `.cache\\hk8x8-rebuild-top-20260917` | 通过 | 0 | `.cache\\hk8x8-rebuild-top-20260917.jsonl` | 历史候选明确记录 `rebuild→validate→commit→succeeded`；耗时 72,205 ms，输出 11,208 个文件、3,145,536,788 bytes，root children=4、节点 85、proxy=21，85/85 个 content URI 可解析；最新候选复测见回归 41 |
| 37 | 本机候选 NSIS 安装（历史候选） | 安装包携带历史 converter runtime；安装后真实转换与卸载 | 安装到 `.cache\\installed-candidate-20260917c`；校验 `_3dtile.exe` SHA256；执行 ASCII `OSGBny` convert-only；静默卸载 | `.cache\\installed-candidate-output-20260917c` | 通过 | 0（安装/转换/卸载） | `.cache\\installed-candidate-20260917c.log` | 历史候选 hash=`af7384f15059d6c3f6927f1aa754bffcf2036504fce3cb3628eb536c77e8e81e`；最新候选安装验收见回归 43；该次安装目录无空格，带空格安装路径沿用回归 31 |
| 38 | converter 本机 Release CLI | 真实 OSGB 中包含中文 Tile 目录和文件名 | 隔离复制 `OSGBny`，将一个 Tile 重命名为 `Tile_中文_+006_+006`，执行 `_3dtile.exe -f osgb ...` | `.cache\\unicode-tile-name-20260917\\output` | 通过 | 0 | `.cache\\unicode-tile-name-20260917\\converter.log` | 生成 85 个文件和 `tileset.json`；6/6 个根 URI 可解析，其中 1 个 URI 保留中文 Tile 名；原始样本未修改；无残留 converter 进程 |
| 39 | processor 本机 Release runtime | 中文 Tile 目录/文件名的 processor 端到端转换 | `processor.exe convert-osgb --input .cache\\unicode-tile-name-20260917\\OSGBny --output <new path> --task-id unicode-tile-name-processor` | `.cache\\unicode-tile-name-processor-20260917` | 通过 | 0 | `.cache\\unicode-tile-name-processor-20260917.jsonl` | 完成 scan→convert→validate→commit；输出 86 个文件，6/6 URI 可解析且含 1 个中文 URI；`output.bytes=13,276,031`；无残留进程 |
| 40 | processor 本机 Release runtime | 最新 converter 候选下香港 8×8 全量转换 | `processor.exe convert-osgb --input data\\real\\hk_8x8\\osgb --output <new path> --task-id hk8x8-convert-final` | `.cache\\hk8x8-convert-final-20260917` | 通过 | 0 | `.cache\\hk8x8-convert-final-20260917.jsonl` | converter hash=`40015f4d776db5acb187a8537e08905a8a47efd80e3c771d07a9bdb24fa6a32e`；耗时 99,842 ms（converter.elapsedMs=94,058），峰值 161,906,688 bytes；输出 11,186 个文件、3,107,088,217 bytes，root children=64，64/64 URI 可解析；无残留进程 |
| 41 | processor 本机 Release runtime | 最新 8×8 转换成果执行完整顶层重建 | `processor.exe process-tileset --input .cache\\hk8x8-convert-final-20260917 --output <new path> --rebuild-top --levels 0 --texture keep --task-id hk8x8-rebuild-final` | `.cache\\hk8x8-rebuild-final-20260917` | 通过 | 0 | `.cache\\hk8x8-rebuild-final-20260917.jsonl` | 明确记录 `rebuild→validate→commit→succeeded`；耗时 73,910 ms，输出 11,208 个文件、3,145,539,790 bytes，root children=4、节点 85、proxy=21，85/85 URI 可解析；无残留进程 |
| 42 | processor 本机 Release runtime | 单个损坏 OSGB 子 Tile 的严格失败 | `processor.exe convert-osgb --input .cache\\corrupt-osgb-20260917\\OSGBny --output <new path> --task-id corrupt-osgb-processor` | `.cache\\corrupt-osgb-processor-20260917` | 通过（明确拒绝） | 1 | `.cache\\corrupt-osgb-processor-20260917.jsonl`、`.cache\\corrupt-osgb-20260917\\converter-fixed.log` | 损坏文件 `Tile_+006_+006_L20_0uuuu1.osgb` 被 converter 明确报告 `read node files ... fail`，返回 1；processor 返回 `CONVERTER_EXIT_NONZERO`，最终输出目录和 `tileset.json` 均不存在；converter 侧仅有未提交的部分临时文件；无残留进程 |
| 43 | 最新候选 NSIS 安装 | 带空格安装路径携带最新 converter；安装后转换与卸载 | 安装到 `.cache\\installed-candidate-space-final2-20260917`，校验 hash，执行 ASCII `OSGBny` convert-only，静默卸载 | `.cache\\installed-candidate-space-final2-output-20260917` | 通过 | 0（安装/转换/卸载） | `.cache\\installed-candidate-space-final2.log` | 安装包 SHA256=`ef3c0589d75524b5a8fda92f5397870b7f06a8f9ee86e292f881ae1a3aac7e79`；安装内 converter hash=`40015f4d776db5acb187a8537e08905a8a47efd80e3c771d07a9bdb24fa6a32e`；转换 85 个文件且 `tileset.json` 存在；卸载退出 0、安装目录移除；无残留进程 |
| 44 | processor 本机 Release runtime | 运行中强制终止 converter | 启动 `processor.exe convert-osgb` 后终止本次任务的 `_3dtile.exe` 子进程；等待 processor 收尾 | `.cache\\converter-crash-20260917` | 通过（明确失败） | 1 | `.cache\\converter-crash-20260917.stdout`、`.cache\\converter-crash-20260917.stderr` | processor 返回 `CONVERTER_EXIT_NONZERO`，记录 `convert exited -1` 和 stderr tail；最终输出目录及 `tileset.json` 均不存在；无残留 processor/converter 进程 |

## 单次记录模板

```text
time:
app: cli | dev-desktop | installed-desktop
input:
input_size:
tile_count:
operation:
options:
output:
processor_exit_code:
converter_exit_code:
result: passed | failed | blocked | not-run
stage:
error:
stderr_tail:
task_log:
converter_sha256:
duration:
peak_memory:
known_warning:
```

不要把一次成功重试记录成“稳定通过”。发生过异常时，保留首次失败和后续尝试的完整记录。
