# V1 加固与发布执行清单

这份清单用于把剩余工作交给能力较弱的执行 Agent。已完成的代码和本机候选验证不要重复修改；每一步都要保留命令、退出码、日志路径和产物 hash。

## 当前基线

- 主仓库：`D:\code\3dtiles`
- converter 仓库：`D:\code\geoforge-converter`
- 最新主仓库提交：`8047027`（processor 重建边界校验）/ `b03bf48`（Tileset 输入校验）
- converter 候选提交：`bbe1426`
- 最新候选安装包：`apps/desktop/src-tauri/target/release/bundle/nsis/GeoForge 3D_0.1.0_x64-setup.exe`
- 最新候选安装包 SHA256：`5dd2c9242ebc47759571d362c7dd5c7c71bf6fcbbeead5e66915cccc392f15af`

仓库中已有的 `apps/desktop/src-tauri/windows-numerics.crate` 和 converter 仓库的 `Cargo.lock` 是既有未跟踪文件，不要删除、提交或覆盖。

## 1. 每次执行前的固定检查

```powershell
Set-Location D:\code\3dtiles
git status --short
git diff --check
cargo test --workspace --locked
Push-Location apps\desktop
npm test
Pop-Location
cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml --lib --locked
```

如果基线检查失败，先记录失败，不要顺手重构。

## 2. 重新生成候选安装包

只有需要重新打包时执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File apps\desktop\scripts\package-windows.ps1 `
  -SkipBuild -SkipTextureBundle `
  -ConverterZip D:\code\3dtiles\.cache\geoforge-converter-0.1.1-unicode-crt.zip
```

确认以下文件存在：

```text
resources/runtime/converter/_3dtile.exe
resources/runtime/converter/msvcp140.dll
resources/runtime/converter/vcruntime140.dll
resources/runtime/bin/processor.exe
resources/runtime/bin/top_rebuild.exe
resources/runtime/bin/msvcp140.dll
resources/runtime/bin/vcruntime140.dll
```

不要把本机候选 hash 写入 `third_party/3dtiles-converter.json`；正式清单只有在 converter zip 已经发布到固定 URL 后才能更新。

## 3. 安装版运行时检查

每次使用新的临时安装目录，例如 `.cache\installed-final-YYYYMMDD`：

1. 静默安装，记录安装器退出码。
2. 在安装目录执行 `_3dtile.exe --help`，必须返回 0。
3. 用 ASCII、中文根目录、中文 Tile 名、空格和 `[]()+-` 路径分别执行 convert-only。
4. 用 `processor.exe convert-osgb` 重复一次，确认输出有 `tileset.json`。
5. 用中文 `GEOFORGE_DATA_DIR` 启动桌面，等待 8 秒确认创建 `tasks.db`，随后结束测试进程。
6. 用 `dumpbin /dependents` 扫描安装 runtime 的 exe/dll；未解析项必须区分 Windows 系统 DLL，不能只看主 exe。

结果写入 `docs/product/HARDENING_REGRESSION.md`，并记录输入、输出、退出码、文件数、日志和 SHA256。

## 4. 真实 Windows 人工验收

这些项目不能用 CLI 结果替代：

- 在安装版 WebView 中关闭有活动任务的窗口，确认任务变为 `interrupted`、processor/converter 无残留；重新启动后不显示 `running`。
- 在 Cesium WebView 检查定位、缺块、纹理和 LOD。
- 在没有全局 VC++ Runtime、没有源码目录和开发覆盖变量的机器上安装运行。
- 对真实失败样本、无权限目录、磁盘空间不足和超长路径记录明确错误。
- 对至少一份 50 GB 级或等价大体量数据记录耗时、峰值内存和输出完整性。

受限环境无法把 `WM_CLOSE` 消息当成 Tauri `CloseRequested/Destroyed` 事件证据；不要把“窗口句柄消失”填写为通过。

## 5. 发布前最终门槛

只有以下条件都具备，才能更新正式 converter 清单并发布：

1. converter zip 有正式版本号、固定 URL 和 SHA256。
2. 干净 Windows 环境安装无缺 DLL 弹窗。
3. 安装版真实转换、取消、重启恢复和 WebView 预览均有记录。
4. 当前用户失败样本有结论；未提供样本时必须标为“待提供”，不能声称全部通过。
5. `RELEASE_ACCEPTANCE.md` 中所有人工项目都有 `通过/失败/阻塞/未执行` 及证据路径。

正式发布、推送 tag、上传 zip 或更新外部 release 需要明确授权；没有授权时只保留本机候选包。
