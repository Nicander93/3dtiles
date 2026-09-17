# V1 真实数据回归矩阵

真实数据保存在本地或受控存储，不提交到 Git。每条记录都要填写 GeoForge 版本、converter 版本、输入大小、Tile 数、耗时、结果和已知警告。

| Case | 场景 | 预期 | 实际结果 | 记录位置 |
| --- | --- | --- | --- | --- |
| 01 | 标准 ContextCapture OSGB（仓库 `OSGBny`） | 成功 | 通过：converter 与 processor 均完成转换 | HARDENING_REGRESSION 17–18 |
| 02 | 已验证 CRS（如 EPSG:4544） | 成功 | 待执行 | — |
| 03 | 中文数据根目录 | 在支持范围内成功 | 通过：候选 converter 修复 OSG UTF-8 路径后，直接转换和 processor 转换均成功 | HARDENING_REGRESSION 17–18 |
| 04 | 中文 Tile/文件名 | 在 converter 支持范围内成功或提前报错 | 通过：隔离复制真实 `OSGBny`，将一个 Tile 重命名为 `Tile_中文_+006_+006` 后转换成功；生成 85 个文件，6/6 URI 可解析且含中文 URI | HARDENING_REGRESSION 38 |
| 05 | 路径包含空格、括号、方括号、加号或短横线 | 成功 | 通过：真实样本覆盖中文/空格；候选 converter 对方括号、加号和括号路径也成功，根 URI 6/6 可解析 | HARDENING_REGRESSION 17–18、32 |
| 06 | metadata.xml 缺失 | 转换前明确错误 | 通过：`scan-osgb` 返回 `valid=false` 和 `Missing metadata.xml` | HARDENING_REGRESSION 26 |
| 07 | metadata.xml 损坏 | 转换前明确错误 | 通过：`scan-osgb` 返回 `valid=false` 和具体 XML parse error | HARDENING_REGRESSION 27 |
| 08 | 单个 OSGB 损坏 | 首版严格失败且指出 Tile | 通过（严格失败）：损坏子 Tile 被明确指出，converter/processor 返回非零，最终目录和 `tileset.json` 均不存在 | HARDENING_REGRESSION 42 |
| 09 | 单个纹理损坏 | 明确失败或 warning，按当前能力记录 | 待执行 | — |
| 10 | 输出成果目录已存在 | 提交前拒绝覆盖 | 通过：返回 `PATH_OUTPUT_EXISTS`，sentinel 未改动且未创建 `tileset.json` | HARDENING_REGRESSION 22 |
| 11 | 输出目录不可写 | 明确错误并包含路径 | 部分通过：父路径为文件时返回 `PATH_OUTPUT_NOT_WRITABLE` 且不创建 child；真实 ACL 无写权限仍待执行 | HARDENING_REGRESSION 25 |
| 12 | 磁盘空间不足 | 明确错误或预警 | 部分通过：受控抬高 `GEOFORGE_LOW_DISK_BYTES` 后产生 `LOW_DISK_SPACE` warning；真实磁盘写满仍待执行 | HARDENING_REGRESSION 23 |
| 13 | 中途取消 | cancelled，输入和既有成果不变 | 通过：约 932 MB/4567 Tile 样本取消返回 `CANCELLED`，无最终/临时目录和残留进程；桌面关闭另测 | HARDENING_REGRESSION 21 |
| 14 | converter 异常退出 | failed，保留 stderr 诊断 | 通过（明确失败）：运行中强制终止 converter 后，processor 返回 `CONVERTER_EXIT_NONZERO`，保留退出码和 stderr tail，未提交最终目录 | HARDENING_REGRESSION 44 |
| 15 | 强制关闭 GeoForge | 下次启动为 interrupted | 待执行 | — |
| 16 | 50 GB 以上数据 | 在记录的资源范围内完成 | 待执行 | — |
| 17 | 大量 Tile | 内存和日志可控 | 通过：香港 8×8 真实 OSGB（11,124 个 OSGB、约 2.47 GB）在最新候选下完成全量 convert→validate→commit，耗时 99,842 ms，输出 11,186 个文件、约 3.11 GB；同一成果继续完成 `rebuild-top --levels 0`→validate→commit，耗时 73,910 ms，输出 11,208 个文件；另有 4,585 文件香港 5×4 成果重建和 1/2/4 worker 资源曲线通过 | HARDENING_REGRESSION 21、30、34–36、40–41 |
| 18 | 超长路径 | 成功或明确拒绝 | 通过（明确拒绝）：约 288 字符输入路径在 scan 后由 converter 返回 `CONVERTER_EXIT_NONZERO`，stderr 指出具体 Tile 文件读取失败；processor 不提交最终目录并保留归属明确的临时目录 | HARDENING_REGRESSION 33 |

首轮发布门槛：01、02、05、06、07、10、11、13、14、15 必须有结果；03、04 只能在实际布局满足 converter 与重建约束时宣称支持。
