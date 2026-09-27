# FBX / OBJ 模型转换实施计划

## 当前基线与结论

本计划以 `feat/model-conversion-v1-integration` 为工作分支，基于当前 `master` 的 `99535b6`。目标不是重写 FBX 解析器：`geoforge-converter` 已有 ufbx 驱动的 FBX → 3D Tiles 转换路径。本次工作要做的是把它安全接入 GeoForge 的任务协议、处理流水线和桌面界面；OBJ 使用转换器已有的 OSG OBJ 读取路径，重点补上单位/轴约定、材质依赖、验证和可靠错误处理。

已纳入当前集成分支的内容：

- FBX / OBJ 模型任务类型、任务参数及格式能力描述。
- 桌面端模型转换表单、输入扫描和任务提交流程。
- 处理端模型配置生成、格式检查、转换及输出验证流程。
- 本地坐标与投影坐标配置支持。
- 提交阶段对临时目录归属和 staged 输出目录的校验；修复主线调用方与提交接口不匹配的问题。
- 旧功能分支中与当前主线不兼容的构建产物未合并。

实现与代码级验收已完成，但发布交付仍未完成：新 converter/processor 在隔离的 packaged runtime 中完成 FBX、OBJ、纹理和严格失败任务。两个随仓库打包的 converter 仍是旧版，Tauri 安装器构建工具也未安装；必须先产出并发布新 converter artifact，更新运行清单，再构建/启动安装包。主仓库 201 项、Tauri 19 项、前端测试及生产构建均通过；converter 7 项测试通过。

## 执行约束

1. 修改前执行 `git status --short --branch`；保留已有修改。不要重置、清理或覆盖用户未提交文件。
2. `geoforge-converter` 已存在 FBX 实现；先复用并验证，不新增另一套 FBX 解析器。
3. OBJ 不是“只要选中扩展名就算支持”：必须确认 OBJ 几何、MTL、纹理路径、法线/UV 和单位/轴变换都能通过真实转换。
4. 不允许仅凭进程退出码 0 判定成功。每次成功转换都检查 `tileset.json`、引用的 tile 内容、坐标范围及日志；失败时不得留下看似成功的最终输出目录。
5. 不提交 `target/`、`dist/`、本地绝对路径、样例生成物、无关格式化或第三方源码。
6. 如转换器目录在执行环境中只读，停止写入并请求针对该仓库的授权；不要用拷贝/覆盖整个仓库绕过权限。

## T0：冻结基线并建立可复现样例

**目的：** 先知道现有转换器实际能做什么，再针对缺口修改。

**执行：**

1. 记录两仓库的分支、HEAD、工作区状态和 upstream 关系；把既有修改逐项列入工作记录。
2. 检查 converter 的 CLI 帮助、支持格式分派和现有测试，确认 FBX / OBJ 的真实入口和输出格式。
3. 选取小型、可再分发的 fixture：一个无纹理 FBX、一个带内嵌纹理或外部纹理 FBX、一个含三角面/UV/法线/MTL 的 OBJ；记录来源和许可。
4. 在干净临时输出目录分别运行 CLI，保存完整命令、版本/提交号、日志和产物清单。
5. 对照输出检查：`tileset.json` 可解析；引用文件存在；几何非空；bounding volume 有限；模型位置/朝向和比例符合 fixture 预期。

**伪代码：**

```text
for format, fixture in [(fbx, fbx_fixture), (obj, obj_fixture)]:
    output = fresh_temp_dir()
    result = run_converter(fixture, output, format)
    assert result.exit_code == 0
    assert parse(output / "tileset.json")
    assert all_referenced_tiles_exist(output)
    record(log, version, hashes, bounds, output_files)
```

**验收：** 能从记录中复现两种格式的基线转换；失败时有最小化错误日志，而不是推测“转换器支持”。

## T1：固定产品契约与配置校验

**目的：** UI、Rust 处理器与 native converter 对配置含义一致。

**执行：**

1. 以 `crates/protocol` 的模型任务配置为唯一公共契约，逐字段确认 `format / unit / axes / georeference / texture / modelOutput` 的默认值、允许值和必填条件。
2. FBX 默认采用文件元数据；如果 native converter 已统一成米和 Y-up，拒绝额外套用 OBJ 归一化。
3. OBJ 必须显式指定单位和轴向；列出单位倍率：m=1、cm=0.01、mm=0.001、ft=0.3048；定义右手 Y-up / Z-up 变换，并说明法线应使用逆转置矩阵归一化。
4. `missingTexturePolicy` 只允许 `warn` / `error`（或协议当前明确枚举）；未知值、格式错配、空 CRS、非有限坐标、非法倍率一律在转换前报错。
5. 为合法与非法配置各加协议/处理器单测，并确保错误信息指出 JSON 字段及可接受值。

**关键逻辑：**

```text
validate(config):
    require config.model.format == input_extension
    if format == FBX: require unit == fromMetadata and axes == fromMetadata
    if format == OBJ: require explicit supported unit and axes
    validate texture policy and texture roots
    validate georeference parameters are finite and internally consistent
```

**验收：** 两格式合法配置均能序列化为 converter 接受的配置；非法配置不会启动 native conversion。

## T2：转换器入口、单位/轴向与错误传播

**目的：** 转换真正失败时，应用收到失败而非伪成功。

**执行：**

1. 保持 FBX 既有 ufbx 实现；追踪 `CLI → Rust → FFI → FBXPipeline → tileset` 的返回值和异常。
2. 检查 FBX 加载失败、空场景、零网格、贴图解码失败、输出写入失败是否会向上返回非零/结构化错误。修复只限真实缺口。
3. 验证 OBJ 读取插件在部署产物中可用；为不同机器缺插件、坏 OBJ、空 OBJ、坏 MTL 添加明确失败信息。
4. 单独检查归一化矩阵应用对象：位置、法线、包围盒、模型锚点使用同一约定，避免重复缩放或只变换顶点不变换 bounds。
5. 测试带父节点变换的 FBX 和 Z-up OBJ，比较输出边界框与预期比例。
6. `build.rs` 应根据 Debug / Release 为 vcpkg 链接提供匹配的库目录；Debug 搜索 `debug/lib`，Release 搜索 `lib`。不要因为本机链接成功就硬编码绝对路径。

**验收：** 至少一个 FBX 和 OBJ 的真实 CLI 转换成功；损坏输入和缺少运行时插件时返回失败，且处理器/桌面端展示可读错误。

## T3：纹理与 OBJ 材质依赖

**目的：** 明确资源如何查找、复制/嵌入以及何时算失败。

**执行：**

1. OBJ 扫描 `mtllib`，解析多个 MTL；处理相对路径时以 OBJ 所在目录为基准，不能把 OBJ 文件路径当目录。
2. MTL 的 `map_Kd` 至少覆盖普通文件名、带空格路径和常见 map 选项；无法可靠解析的选项必须明确警告，不能静默拼错路径。
3. FBX 外部纹理按顺序查找：嵌入内容 → FBX 所在目录下原相对路径 → 同目录文件名 → 用户配置的 textureRoots；记录最终命中路径。
4. 两种格式复用同一缺图策略。`warn` 继续生成无该贴图的模型并记录引用；`error` 将所有缺失/无法解码纹理汇总并使转换失败。
5. 检查纹理使用场景：diffuse/base color 至少支持；normal map 只在确实实现时宣称支持；bump-height 不能伪装成 tangent-space normal。
6. 用临时目录测试路径穿越/绝对路径策略，避免扫描阶段无意读取项目范围以外的敏感文件；textureRoots 应是显式授权根目录。

**伪代码：**

```text
resolve(reference, modelDir, roots):
    candidates = [embedded(reference), modelDir / originalPath,
                  modelDir / basename(reference)]
    for root in roots: candidates += [root / originalPath, root / basename]
    return first_existing_supported_image(candidates)

if unresolved:
    collect(reference, material, slot)
    if policy == error: fail_after_scan_with_all_missing_references()
```

**验收：** OBJ/FBX 有纹理、无纹理、缺失纹理三组 fixture 都符合策略；`error` 模式以失败退出且不提交输出。

## T4：GeoForge 任务流水线与输出提交

**目的：** 将 CLI 的转换结果纳入现有安全任务机制。

**执行：**

1. 处理器只调用协议支持的 FBX/OBJ converter 参数，不拼接 shell 命令；传参使用 `Command` 参数数组。
2. 输入、MTL、纹理路径先做路径策略检查；输出统一写入任务拥有的临时根目录。
3. 用有界超时/取消机制运行外部进程；持续收集 stdout/stderr，但限制缓存字节数，避免超大日志占内存。
4. 检查 exit code；随后运行 tileset/资源验证器；只有验证成功才进入 commit。
5. staged 目录必须是 owned temp 根目录的直接子目录且不能是符号链接；提交阶段使用原子且不覆盖的 rename；任何失败都清理任务临时文件。
6. 事件顺序稳定：`Queued → Validating → Converting → ValidatingOutput → Committing → Completed`；失败事件带阶段和安全裁剪后的原因。

**验收：** 成功路径只出现一个最终输出；转换或验证失败时没有半成品最终目录；超时、取消、目标目录已存在均有测试。

## T5：桌面端体验和操作防错

**目的：** 用户能看懂格式限制和资源依赖，并且不能提交未完成扫描的输入。

**执行：**

1. 格式选择固定为 FBX / OBJ，选中 OBJ 才显示必填单位和轴向；FBX 元数据字段显示为自动读取且不可误改。
2. 提供坐标模式：本地坐标、经纬高锚点、投影 CRS；未实现的垂直基准不得伪装支持。
3. 扫描状态使用 `idle / scanning / valid / invalid`；路径变更立即清空旧扫描结果；只有扫描结果对应当前路径且 valid 才可提交。
4. 展示 OBJ 的 MTL/贴图缺项；FBX 展示检测到的贴图引用/缺失项；`warn/error` 文案解释是否继续转换。
5. 提交中防止重复提交，提供取消；结束后显示输出位置、转换摘要和可读错误，不把临时目录暴露成最终结果。
6. 增加 React 测试覆盖状态门控、配置必填和错误展示；保持既有 UI 风格和无障碍标签。

**验收：** 扫描未完成、扫描已过期、配置非法均无法提交；FBX/OBJ 成功与失败状态在 UI 可区分。

## T6：测试矩阵、回归与打包

**目的：** 证明功能在测试和实际发行形态中都可用。

| 场景 | 预期结果 |
|---|---|
| FBX 元数据单位/轴向 | 不重复归一化，边界框比例正确 |
| FBX 嵌入/外部纹理 | 对应纹理被保留或按策略报告 |
| OBJ cm + Y-up | 输出边长为输入的 0.01 倍 |
| OBJ mm + Z-up | 输出边长为输入的 0.001 倍且轴向正确 |
| OBJ 缺失 MTL / 贴图 | scan 和 warn/error 行为一致、错误清楚 |
| 坏文件 / 空场景 / 不支持图像 | 失败且不提交输出 |
| 非有限坐标 / 无效 CRS | 转换前拒绝 |
| 取消、超时、输出已存在 | 临时文件回收，最终目录保持原样 |

**执行命令：**

```text
rtk cargo test --workspace
cd apps/desktop && rtk npm test
cd apps/desktop && rtk npm run build
在 converter 仓库运行对应 cargo test 与 FBX/OBJ fixture 转换
构建桌面发行包，并在干净运行环境执行一次真实 FBX 和 OBJ 转换
```

执行后检查 `git diff --check`、工作区状态和生成产物；删除本轮命令生成且可明确归属本轮的 `dist` 产物前先核实，不要动预先存在的文件。Debug 与 Release 至少分别验证一次 converter 链接。

## T7：交付、文档和分支收尾

**目的：** 使用者知道支持边界，维护者可复现结果。

**执行：**

1. 写用户指南：支持格式、推荐 FBX/OBJ 导出设置、OBJ+MTL 打包方式、坐标配置、纹理根目录、已知限制和排错步骤。
2. 写验收记录：环境与 converter 版本、fixture 哈希、命令、关键日志、输出文件检查、已知未支持项。
3. 检查 license 与分发许可；第三方 fixture/示例材质不得未经许可放入发布包。
4. 确认 CI 覆盖协议/处理器/UI 测试；native fixture 测试如受平台限制，明确标记需运行的 Windows/macOS/Linux 矩阵。
5. 汇总改动、验证结果、剩余风险和回滚边界；只有所有阻断项清零并获得真实端到端证据后，才把功能标为完成。

## 当前执行状态

以下为执行中的里程碑记录；前文“当前基线”描述的是计划制定时的状态。2026-09-24 的正式包复验结果以同目录的验收记录为准。

- **T0 基线核查：** 已完成关键基线。Debug 与 Release converter CLI 均实际转换 FBX、OBJ；小型投影厘米 OBJ 也产出 tileset/B3DM 并验证了缩放与投影 root transform。可复现命令与观察结果见验收记录。
- **T1 配置/契约：** 两仓配置结构已对齐；converter 拒绝未知 `missingTexturePolicy`，允许 `warn/error`，并接收纹理根目录。主仓库扫描及任务参数传递已连接，纹理根目录要求为有效目录。
- **T2 转换器运行：** Windows Debug/Release 均成功构建并运行。为消除 MSVC Rust Debug 与 C++ Debug CRT 不兼容，native 静态库固定 Release 配置；跨平台/发行构建矩阵仍待发布前完成。
- **T3 纹理：** OBJ MTL map 解析、外部根目录与 warn/error 已覆盖；FBX 内嵌贴图 strict 模式成功，外部纹理缺失 strict 失败、增加纹理根目录后同一 fixture 成功。两种格式输出均经处理器验证器提交。
- **T4 提交安全：** staged 路径约束、no-replace commit、未提交 temp 全退出清理已实现；子进程默认 6 小时超时（`GEOFORGE_PROCESS_TIMEOUT_SECS` 可配置，最多 24 小时）、支持取消。单测覆盖超时/取消；隔离 packaged processor 实测 strict 缺图失败无 final/temp，输出冲突保留原文件。
- **T5 UI：** 输入/纹理目录扫描的过期结果被忽略；测试覆盖旧路径、debounce 未收敛、纹理目录不匹配、pending、invalid 和格式错配时禁提交流程；表单校验覆盖 OBJ 单位/轴、锚点、投影 CRS/能力与偏移值。测试通过 Node + TypeScript transpile 直接运行 UI 共用纯函数；当前仍没有 React DOM 组件挂载测试。
- **T6 验证打包：** workspace 201 项、Tauri 19 项、converter 7 项测试通过；npm 测试、生产构建和 Tauri check 通过。converter v0.2.1 已正式发布，CI 从最终 ZIP 完成 FBX/OBJ 实转换；主仓库 manifest 已固定 v0.2.1，隔离 packaged processor + runtime 完成真实成功/失败任务。正式桌面 NSIS 安装包仍待构建、安装和启动；converter 三平台矩阵未执行。
- **T7 文档收尾：** 执行计划、用户指南、验收记录已补；记录含 fixture 和 Release SHA-256、成功与失败证据、许可/分发提示。主仓库集成改动尚未提交或创建 PR；用户生产模型业务验收和正式安装包复验仍待完成。
