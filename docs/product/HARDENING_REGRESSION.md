# V1 Hardening 回归记录

这份记录只保存可复现实验的元数据和结果，不提交真实 OSGB 数据。

## 环境基线

| 项目 | 值 |
| --- | --- |
| GeoForge commit | `e23d60476abbd651b12cb4fa82098013d5f6b6d4` |
| GeoForge 版本 | `0.1.0`（仓库/桌面配置） |
| Processor 路径 | `D:\\code\\3dtiles\\target\\debug\\processor.exe` |
| Converter 版本 / source commit | `0.1.0` / `76ef4d6ce419c675c9742ed13cf600d74ca5c78e`（本机候选构建） |
| Converter SHA256 | 本机候选 runtime `_3dtile.exe`: `7da792b3ecc51176bc829cb317da97c77f86fbfd34806c41ceb471b0e47fe2e7`; 固定发布清单值为 `77cadd01941add0297a52a22ce26918d8d22c3729a0ae564556fce14327b5c5b`，两者不一致，不能宣称已更新正式发布包 |
| Runtime 根目录 | `D:\\code\\3dtiles\\dist\\runtime`（本机候选，converter `--help` 返回 0；已完成临时 NSIS 安装验收） |
| Installer SHA256 | 本机候选 `GeoForge 3D_0.1.0_x64-setup.exe`: `04c2cb84c94a18692945fd33d9628d99074bdd00f2cd53b1cbdebb2c19359da0` |
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
| 09 | 开发 CLI/Tauri | 健壮性回归 | `cargo test`；Tauri `cargo test --lib` | — | 通过 | 0 | cargo stdout | processor 27 项、workspace 90 项、Tauri 7 项；新增协议行上限、marker 归属、Tileset 边界、GLTF 路径越界和分块取消检查 |
| 10 | 开发 CLI | 隔离 synthetic grid 4×4（替换零字节 B3DM） | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | 临时目录（已清理） | 通过 | 0 | processor JSONL | 在新增 geometricError/boundingVolume/GLTF 越界校验后重跑；scan→rebuild→validate→commit 全部通过；不替代真实 OSGB/安装版验收 |
| 11 | 发布脚本 | PowerShell 语法与门槛检查 | `Parser.ParseFile` 检查 `prepare-converter.ps1`、`prepare-runtime.ps1`、`package-windows.ps1` | — | 通过 | 0 | PowerShell stdout | converter help 非零/超时现在阻止 staging；runtime manifest 含 SHA256；未执行真实 installer |
| 12 | converter 源码 | FFI 参数边界审查 | `cargo metadata --no-deps`；`git diff --check`；审查 `osgb23dtile_path`/`osgb2glb` | — | 通过 | — | — | 入口拒绝空指针、负层级、非有限坐标；JSON 返回检查空值、`int` 长度上限和分配失败 |
| 13 | converter 本机 Release CLI | MSVC/vcpkg 原生构建与启动 | `cargo build --release`；`_3dtile.exe --help` | `D:\\code\\3dtiles\\dist\\runtime` | 通过 | 0 | stdout/stderr | MSVC 14.51.36231；加入 OSG UTF-8 路径与相对 URI 修复后的候选 hash `7da792b3ecc51176bc829cb317da97c77f86fbfd34806c41ceb471b0e47fe2e7` |
| 14 | converter 本机 Release CLI | 中文/空格路径与伪成功负向测试 | `-f osgb -i "中文 目录\\输入.osgb" -o "输出 目录"` | 临时目录 | 通过 | 2 / 1 | stderr | 不存在输入返回 2；空 OSGB 返回 1 且没有伪造 `tileset.json` |
| 15 | NSIS 临时安装 | 安装后 runtime 完整性、启动、真实中文路径和 URI | 静默安装 `/S /D=<temp>`；`_3dtile.exe --help`；真实 `OSGBny` 转换；检查 URI；卸载 | 临时目录（已清理） | 通过 | 0 | `help.log`、`unicode.log` | 安装包 hash `04c2cb84c94a18692945fd33d9628d99074bdd00f2cd53b1cbdebb2c19359da0`；输出 85 个文件、URI 6/6 可解析；未使用开发覆盖变量 |
| 16 | processor 本机 Release runtime | synthetic grid 4×4 重建回归 | `processor.exe process-tileset --rebuild-top --levels 2 --texture keep` | 临时目录（已清理） | 通过 | 0 | processor JSONL | 使用候选 runtime/bin/top_rebuild.exe 完成 scan→rebuild→validate→commit；不替代真实 OSGB |
| 17 | converter 本机 Release CLI | 真实 `OSGBny`，中文输入根与中文/空格输出目录 | `_3dtile.exe -f osgb -i "...\\中文 数据根\\OSGBny 输入" -o "...\\中文 输出修复\\OSGBny uri"` | `.cache\\中文 输出修复\\OSGBny uri` | 通过 | 0 | converter stdout/stderr | 78 个 OSGB、约 14 MB；生成 `tileset.json`，共 85 个文件、约 13.3 MB；根 URI 6/6 可解析；候选 hash `7da792b3ecc51176bc829cb317da97c77f86fbfd34806c41ceb471b0e47fe2e7` |
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
