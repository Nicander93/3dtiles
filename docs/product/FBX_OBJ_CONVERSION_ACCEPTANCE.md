# FBX / OBJ 转换能力验收记录

## 记录范围

本记录汇总开发工作区已执行的代码级、处理器流水线和隔离运行时验收，不等同于正式安装包验收。主仓库工作分支为 `feat/model-conversion-v1-integration`；converter 修复提交为 `a2f658d`，正式发布版本为 `v0.2.1`。两仓原有未提交文件均未重置或清理。

## 功能结果

| 能力 | 验收结果 | 证据/边界 |
|---|---|---|
| FBX 转换 | 通过 | Windows Debug、Release CLI 均使用仓库自带 ufbx FBX fixture 实际输出 tileset 与 B3DM |
| FBX 内嵌贴图 | 通过 | embedded-texture FBX strict 模式加载 PNG，并完成处理器验证/提交 |
| FBX 外部贴图 | 通过 | 外部绝对路径贴图缺失时 strict 失败；添加纹理根目录后同一 FBX 转换成功 |
| OBJ 转换 | 通过 | Debug、Release CLI smoke 输出 tileset 与 B3DM |
| 投影坐标 OBJ | 通过 | 厘米单位小型 fixture 经投影坐标配置输出；观察到输入 10 单位按厘米解释为 0.1 m，root transform 有地理定位 |
| OBJ 外部贴图 | 通过 | 以外部纹理根目录加载包含空格路径的 PNG，native converter 日志确认图像加载并嵌入输出 |
| 缺失贴图 warn | 通过 | 告警后仍生成 tileset |
| 缺失贴图 error | 通过 | CLI 返回非零，日志指出缺失 baseColor 贴图，未生成最终 tileset |
| 失败清理与输出冲突 | 通过 | 处理器 strict 缺图失败后无 final/temp；已有输出冲突失败且原 tileset hash 不变、无 temp |
| 子进程超时/取消 | 通过 | 处理器单测分别触发 100ms 超时和取消长运行子进程，验证退出码 124/130 |
| 隔离 packaged runtime | 通过 | 现有 runtime 副本替换本次 Release converter/processor 后能力探测 ready；真实 FBX/OBJ convert-model 完成 validate/commit |
| 主仓库依赖扫描 | 通过 | `scan-model --texture-root` JSON 输出有效，发现 MTL 和纹理资源；测试覆盖 map 选项及引号/空格路径 |
| 前端防错 | 通过 | 输入/纹理根目录变化会触发新扫描；旧扫描不会覆盖当前结果；未完成/过期/失败扫描不能提交 |
| 前端配置校验 | 通过 | Node 测试覆盖 OBJ 单位/轴、输入输出相同、锚点数值/范围、projected CRS/能力及 origin 数值 |
| 桌面 API 接线 | 通过 | 新增选择纹理目录 Tauri 命令并由桌面 API 调用；Tauri Rust 检查通过 |

## 已运行检查

- 主仓库：`rtk cargo test --workspace`，201 项通过（19 个测试套件）。
- Tauri Rust：19 项测试通过；`rtk cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过。
- 前端：`rtk npm test` 通过（含扫描门禁用例）；最终复跑 `rtk npm run build`（TypeScript + Vite，4608 modules）通过。构建生成的 `dist` 文件已恢复/清理，未作为源代码改动保留。
- converter：`rtk cargo test`，7 项通过；Debug `rtk cargo build` 与 Release `rtk cargo build --release` 通过。
- converter CLI：`--help` 与 `--capabilities-json` 正常；能力声明包含 `fbx`、`obj` 和 projected georeference。
- 运行时注意：本机 CLI 执行需将 converter 的 `vcpkg_installed\x64-windows\bin` 加入 `PATH`，否则 Windows 可能报动态库加载失败。正式桌面包还必须验证依赖 DLL 和 OSG 插件部署。

## Fixtures 与产物指纹

| Fixture | SHA-256 |
|---|---|
| `blender_279_ball_7400_binary.fbx` | `4A3796F053D7F17351CC5F2E0F60ECF1AE3EC3EA70073CF9147A726D03CCBCE8` |
| `blender_293_embedded_textures_7400_binary.fbx` | `0747B141C0EA4467574719BBEBE855EF2A0F25C9BF51792439A6BF1D2F380944` |
| `maya_absolute_texture_7700_binary.fbx` | `A1D98DB7C84786B256B7D48D6F39AE34AAE0DAA9D9FB58E9148A52F0A1DF50C1` |
| `texture-root.obj` | `44B72AA6123B03C8279ABB72D15FE93307B92F7BA8369B0EC6343FD369D301C9` |
| `missing-texture.obj` | `AA4173860A13EE5BE0CFC095A2B0AB09C638DCED43C52F2702E2266F71D85F7B` |
| v0.2.1 Release `_3dtile.exe` | `D03AA9F6AEB9C4C749A1CE9B4955BDD12FA9490C5FAFE7A0084DAF8702F00823` |
| v0.2.1 Windows ZIP | `86DE5812946586E41EB15CA50C691CE1C01D0E4D65B1DCD822E5850701D7D0AF` |

测试素材来自 converter 仓库的 ufbx 数据集或本地 `tests/fixtures`，不会打进产品运行时。ufbx 树附带 MIT/Unlicense 双许可证；将其 FBX 测试样例再分发前仍需逐项核对素材来源许可。

## 复现命令样例

在主仓库根目录执行（路径按本机仓库位置调整）：

```powershell
rtk cargo test --workspace
rtk cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
rtk npm test
rtk npm run build
rtk cargo run -p processor -- scan-model --path '<converter-root>\tests\fixtures\texture-root.obj' --texture-root '<converter-root>\thirdparty\ufbx\data'
```

在 converter 仓库根目录执行：

```powershell
rtk cargo test
rtk cargo build
rtk cargo build --release
rtk cargo run -- --help
rtk cargo run -- --capabilities-json
```

端到端 FBX/OBJ smoke 命令和配置使用本机 `target` 下的临时 fixture/output 目录；详细参数应以对应 fixture 配置和 CLI `--help` 为准，避免把含本机绝对路径的命令复制进产品配置。

## v0.2.1 正式发布包复验（2026-09-24）

- converter 的 [Release v0.2.1](https://github.com/Nicander93/geoforge-converter/releases/tag/v0.2.1) 和 Windows CI 均成功；CI 解压最终 ZIP 后实际转换了 FBX、OBJ fixture，并检查 `tileset.json` 与非空 B3DM。v0.2.0 因携带过旧 MSVC CRT，实转换会崩溃，不可用于产品依赖。
- 下载的 ZIP 与 Release 公布的 SHA-256 及随包 `.sha256` 一致。主仓库 `third_party/3dtiles-converter.json` 已固定到 v0.2.1 和该校验值；`prepare-runtime.ps1` 从 manifest 缓存命中并成功暂存。
- 隔离运行时 `target/fbxobj-runtime-v0.2.1` 的处理器报告 `packaged=true`、`model.ready=true`。FBX 与带外部纹理的 OBJ 任务均经过 convert、validate、commit，分别生成一个 tileset 和一个非空 B3DM。缺失纹理的 OBJ 在 strict 模式返回非零，最终输出和任务临时目录均不存在。
- 主仓库 `cargo test --workspace`、Tauri `cargo test`/`cargo check`、桌面前端 `npm test`/`npm run build` 均通过。Windows PowerShell 本机执行 npm 时需调用 `npm.cmd`，避免执行策略拦截 `npm.ps1`。

## 交付前仍需完成

1. 主仓库集成分支的代码与新 manifest 尚未提交和推送；正式桌面安装包还未构建、安装和启动。旧的 `dist/runtime`、app 资源目录不是本次验收对象，不得当作 v0.2.1 交付物。
2. 本机未发现 NSIS/WiX 工具；可使用主仓库 `release-windows.yml` 的 `workflow_dispatch` 生成 NSIS artifact，再下载、校验、安装并做 UI 端到端验收。
3. 本机仅完成 Windows 验收；converter 的跨平台 native fixture 矩阵未完成。
4. 尚未用用户提供的生产模型做业务验收。
