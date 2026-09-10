# GeoForge 3D V1：生产可用化、自验证与公开数据验收执行方案

> 面向 Coding Agent 的后续执行文档  
> 日期：2026-09-10  
> 目标仓库：`https://github.com/Nicander93/3dtiles`  
> 当前基线：`master`，合并提交 `a6c9393685a9600d42981ae2ff65846ed5cb522e`  
> 前置文档：`docs/product/03-v1-architecture-rebuild-plan.md`、Phase 0–10 reports、《倾斜摄影 3D Tiles 顶层重建技术方案 V1》  
> 本轮目标：**不再以“算法原型能跑”为验收标准，而以“限定 V1 支持范围内可用于真实生产数据”为验收标准。**

---

# 0. Agent 执行方式

收到本文后，直接开始执行。不要先重新设计一套新架构，也不要重复 Phase 0–10 已经完成的工作。

本轮允许 Agent：

- 联网查找和下载公开数据；
- 安装测试工具；
- 增加 acceptance / benchmark 脚本；
- 修改 Rust / Tauri / React / C++ 代码；
- 增加 CI；
- 为测试构建辅助数据布局；
- 运行长时间真实数据测试；
- 生成日志、指标、截图、校验报告。

本轮不允许：

- 用 synthetic fixture 替代最终真实数据验收；
- 遇到真实数据失败后把输入替换成更简单的 toy data，然后宣布生产可用；
- 对损坏/缺失 content 自动生成 box、空 mesh 或测试纹理继续通过；
- 为了让测试通过而静默忽略 transform、纹理、extension、missing URI、budget、gap 等错误；
- 大幅改变产品范围，例如引入 remeshing、Atlas、implicit tiling、分布式处理，除非已有真实测试明确证明 V1 主路线无法达到生产要求；
- 把外部公开数据提交到 Git 仓库；
- 因为某一台开发机“能跑”就宣布可交付。

## 0.1 执行节奏

Agent 可以连续完成本文件中的 Phase 11～18，不必每个 Phase 都等待用户确认，但必须：

1. **每个 Phase 单独提交或至少形成独立 commit。**
2. 每个 Phase 完成后先运行其验收命令。
3. 若验收失败，先修复，不要继续把失败累积到后续 Phase。
4. 真实数据测试耗时较长时可以继续执行，不需要等待人工确认。
5. 只有遇到以下真正 blocker 才停止并汇报：
   - 官方公开数据已不可下载且没有合法替代数据；
   - 磁盘空间明显不足，预计需要超过 30 GB 新下载；
   - 构建依赖需要管理员权限且当前环境无法安装；
   - 发现当前 V1 技术路线在真实数据上存在结构性不可行问题，需要引入 remeshing / Atlas 等已明确延后的大能力。

---

# 1. 当前版本的准确定位

当前代码已经完成一次重要的架构收敛：

```text
apps/desktop
    Tauri 2 + React + CesiumJS
          │
          ▼
crates/processor
    TaskConfig / JSONL / pipeline
          │
          ├── OSGB converter
          ├── top_rebuild
          ├── texture
          └── validate / commit
          │
          ▼
crates/top_rebuild
    SourceBlock / Representation
    TreeBuilder / ProxyBuilder
    TilesetWriter / Texture / Gap
```

Qt / OSGB Native Viewer 已退出产品路径，这个方向保持不变。

当前 `top_rebuild` 已有：

- 规则 Quadtree Bottom-up；
- SourceBlock / Representation；
- glTF scene/node transform 展开；
- parent local frame；
- 跨 child primitive 汇总；
- meshoptimizer；
- LockBorder；
- texture hash / resize / optional KTX2；
- REPLACE HLOD；
- `rebuild_metrics.json`；
- 4×4 / 16×16 synthetic tests。

这些代码不是废弃原型，**本轮应在其基础上加固，而不是推倒重写。**

但是当前版本仍然只能定义为：

> Engineering Prototype / 真实数据验证候选版本

不能定义为：

> Production Ready

原因不是“功能少”，而是下面几个正确性、发布、安全和真实数据验证问题还没有关闭。

---

# 2. 本轮“生产可用”的严格定义

本文中的“生产可用”是一个**限定范围的 V1 production-ready**，不是宣称支持所有 3D Tiles / 所有 OSGB。

## 2.1 V1 正式支持范围

生产验收通过后，可以明确宣称支持：

### OSGB 输入

- ContextCapture / Smart3D 风格的倾斜摄影 OSGB；
- 以空间 Block / Tile 组织；
- 存在稳定的根层或可解析 PagedLOD 层级；
- 坐标系 / 原点信息可获得，或者用户明确提供；
- 规则或近似规则 XY Block 网格；
- 文件没有损坏。

### OSGB → 3D Tiles

- 转换为可独立加载的 3D Tiles；
- 保持地理定位；
- 保持纹理；
- 支持可选顶层 Proxy HLOD；
- 支持可选 KTX2；
- 输出可以在 CesiumJS 中使用。

### 已有 3D Tiles

V1 的后处理正式支持分两类：

1. **纹理处理**：对本工具明确兼容的本地 B3DM/GLB 3D Tiles 数据处理。
2. **顶层重建**：主要保证本工具自身 OSGB converter 输出，以及结构兼容的 explicit external-block 3D Tiles。

对 arbitrary 3D Tiles 1.1、implicit tiling、ADD refine、multiple contents、特殊 required extensions 等，V1 可以**明确拒绝**，但必须在执行前给出可解释的 compatibility error，不能处理一半才损坏成果。

## 2.2 “生产可用”必须同时满足

不是只满足算法正确，还必须满足：

```text
数据正确性
+ 输出完整性
+ 失败安全
+ 可重复验证
+ 真实数据质量
+ 性能可接受
+ Windows 安装/运行
+ 无开发机路径依赖
+ 无用户手装 Python
```

---

# 3. 本轮 P0 阻塞项

以下项目任何一项未关闭，最终状态都必须是 `NOT PRODUCTION READY`。

---

## P0-1：release 路径禁止 synthetic fallback

当前 `WriteOptions` / release CLI 仍允许：

```text
synthesize_if_empty = true
```

当真实 content 缺失或不可用时，代码可能创建 box / test content。

这在 fixture 中可以存在，在 release 路径中禁止。

### 必须修改

正式 `top_rebuild`：

```text
synthesize_if_empty = false
inject_test_textures = false
```

只有以下显式测试入口允许 synthetic：

```text
top_rebuild_debug
test fixture generator
unit/integration test
```

### 正式路径遇到以下情况必须失败

- content 文件不存在；
- B3DM header 无效；
- GLB 无法解析；
- POSITION 缺失；
- texture 引用损坏；
- external tileset 不存在；
- required content 无法形成完整 coverage。

统一错误码建议：

```text
CONTENT_MISSING
CONTENT_INVALID
GLTF_INVALID
TEXTURE_MISSING
SOURCE_COVERAGE_INCOMPLETE
```

### 验收

增加测试：

```text
release_missing_content_must_fail
release_corrupt_b3dm_must_fail
debug_fixture_may_synthesize_only_when_explicit
```

---

## P0-2：必须完整保留原始 Block 子树

当前 `ensure_leaf_content()` 重新生成 Block `tileset.json` 的做法不允许继续用于正式路径。

生产语义必须是：

```text
New Proxy HLOD
   │
   └── Original Block External Tileset
            │
            └── 原始完整 LOD subtree
```

而不是：

```text
rep0
 └── rep1
```

### 必须实现

对每一个 SourceBlock：

1. 保存其原始 external tileset 根路径。
2. TopRebuild 只为 Proxy 选择 source representation。
3. 在最终树的 L0 处挂载**原始 external tileset**。
4. 原 external tileset 内部结构、children、content、geometricError、metadata、extensions 不因顶层重建而重写。
5. 如果为了输出独立成果需要复制文件，应完整复制对应 subtree 依赖。
6. URI 重定位必须正确。

### 增加 subtree preservation 校验

对每个原 Block：

- 计算 source external tileset 的 normalized structure digest；
- 输出后的 Block external tileset 做相同 digest；
- 路径变化可忽略；
- 不允许节点数减少；
- 不允许 content URI 对应内容缺失；
- 不允许 extension 被静默删除。

建议产生：

```text
subtree_preservation.json
```

例如：

```json
{
  "block": "Tile_+012_+008",
  "sourceNodeCount": 187,
  "outputNodeCount": 187,
  "sourceContentCount": 186,
  "outputContentCount": 186,
  "structurePreserved": true
}
```

### 验收

真实数据中随机抽至少 10 个 Block：

```text
structurePreserved = true
missingUri = 0
```

---

## P0-3：Representation 必须表示“完整覆盖”，不能等价于单个 content

当前递归把所有 content-bearing node 平铺进 `representations[]` 的语义对分支 LOD 树不可靠。

必须改为：

```text
Representation = 一个 SourceBlock 在某一误差级别下的完整可渲染覆盖集合
```

推荐数据模型：

```rust
pub struct Representation {
    pub id: String,
    pub geometric_error_meters: f64,
    pub parts: Vec<RepresentationPart>,
    pub bounds: BoundingVolume,
    pub triangle_count: u64,
    pub texture_bytes: u64,
}

pub struct RepresentationPart {
    pub content_path: PathBuf,
    pub world_transform: Mat4d,
    pub bounds: BoundingVolume,
}
```

或者保持其他等价结构，但语义必须正确。

### Coverage Frontier

实现一个明确的 selector：

```text
输入：SourceBlock LOD tree + target error
输出：一个无重叠、完整覆盖 SourceBlock 的 frontier
```

规则：

- 一个 node content 足够表示其完整空间覆盖时，可以停在该 node；
- 如果 parent content 缺失或不足，则下降到 children；
- 不能同时选择 parent 与其 descendant 造成重复覆盖；
- 如果一个分支需要下降，必须保证最终 frontier 对 SourceBlock 覆盖完整；
- ADD refine V1 不参与重建，预检时报 unsupported；
- required extension 影响 geometry/transform 时，不理解就拒绝。

### 验收

至少加入以下 fixture：

```text
single-chain LOD
root-content + 4 child branch
root no content + 4 child coverage
one child missing → SOURCE_COVERAGE_INCOMPLETE
parent + child overlap → frontier 不重复选择
```

真实公开 OSGB 转换结果中打印每个 Block：

```text
frontierParts
sourceError
triangleCount
textureBytes
```

不能所有 Block 都机械等于 1 个 content，除非真实数据本身确实如此。

---

## P0-4：transform / boundingVolume 必须建立明确的 world-space 语义

所有空间计算必须区分：

```text
local bounds
tile transform
accumulated world transform
world bounds
parent local bounds
```

### 禁止

不能直接：

```text
JSON boundingVolume.box
    ↓
union_all
```

除非已经证明这些 bounds 在同一个 world frame。

### 必须实现

推荐统一结构：

```rust
pub struct SpatialBounds {
    pub local: BoundingVolume,
    pub world_aabb: Aabb3d,
}
```

解析时：

```text
local BV corners
  × accumulated tile transform
  = world corners
  → conservative world AABB
```

建立 parent：

```text
children world AABB union
  ↓
parent world bounds
  ↓
选择 parent local frame
  ↓
world bounds → parent local BV
```

### Cesium box

必须正确处理 box 的三个 half-axis：

```text
center
halfAxisX
halfAxisY
halfAxisZ
```

不能假设：

```text
hx = b[3]
hy = b[7]
hz = b[11]
```

即不能假设 box 一定轴对齐。

至少支持：

- 任意 oriented `box`;
- `region` 可以做只读兼容性识别；若当前算法不支持 region top rebuild，则预检明确拒绝；
- `sphere` 同理。

### Transform tests

必须增加：

```text
identity
translation
rotation Z
rotation + translation
nested transforms
large ECEF translation
oriented bounding box
negative grid index
```

数值要求：

- 纯 double matrix invariant：`maxAbsError <= 1e-8`；
- 原始 child 重挂载前后 world origin 差：`<= 1e-4 m`；
- 写 GLB 为 local float 后，在典型城市尺度下 sampled vertex world roundtrip 误差：`<= 0.05 m`；
- 不允许 NaN / Inf。

---

# 4. Phase 11：TopRebuild Correctness Hardening

本 Phase 只关闭 P0-1～P0-4，不做 packaging。

## 4.1 修改范围建议

重点检查：

```text
crates/top_rebuild/src/adapter.rs
crates/top_rebuild/src/types.rs
crates/top_rebuild/src/selector.rs
crates/top_rebuild/src/tree_builder.rs
crates/top_rebuild/src/proxy_builder.rs
crates/top_rebuild/src/tileset_writer.rs
crates/top_rebuild/src/glb.rs
```

允许重构数据模型，但不要改变：

```text
Quadtree
Bottom-up
meshoptimizer
LockBorder
原 UV
no atlas
no remesh
REPLACE
```

这些 V1 核心路线。

## 4.2 TreeBuilder 的职责边界

TreeBuilder 只决定：

```text
哪些空间 block 组成 parent
parent 的空间范围
parent/child 关系
level
```

不要让 TreeBuilder 负责：

- 读取 mesh；
- 复制 content；
- 处理纹理；
- 修复损坏文件。

## 4.3 ProxyBuilder 输入

ProxyBuilder 应接收“一个 parent 的完整 child source coverage”，而不是假定一个 child = 一个 GLB。

类似：

```rust
pub struct ProxySource {
    pub parts: Vec<ChildContent>,
}
```

每个 part 都要进入 parent local frame 后汇总。

## 4.4 完成条件

Phase 11 结束时必须：

```text
cargo test -p top_rebuild
```

全部通过，并新增至少：

- subtree preservation tests；
- coverage frontier tests；
- oriented box tests；
- nested transform tests；
- no synthetic release tests。

---

# 5. Phase 12：真正的 Validator

当前 validator 只检查 `tileset.json` 存在和 JSON 可解析，不满足生产要求。

需要建立两层校验。

---

## 5.1 Layer A：内置快速校验

Processor commit 前必须递归检查：

### Tileset JSON

- `asset` 存在；
- `root` 存在；
- `geometricError` finite 且非负；
- `transform` 长度正确且 finite；
- boundingVolume 合法；
- REPLACE tree 中 parent geometricError > child geometricError；
- content URI 合法；
- external tileset 可解析；
- 防循环引用；
- 不允许逃逸输出目录的相对 URI。

### Content

每个本地 content：

- 文件存在；
- size > 合理最小值；
- B3DM magic / byteLength 正确；
- GLB magic / version / chunk length 正确；
- glTF POSITION 可读；
- index 不越界；
- vertex / normal / UV 无 NaN/Inf；
- texture bufferView / image 可读；
- KTX2 声明与实际数据一致。

### HLOD

- root 到 leaf 可达；
- 每个原 Block 至少出现一次；
- 每个 Proxy 有 content；
- parent bounds 覆盖 children world bounds；
- world transform invariant；
- level 数与 tree report 一致。

输出：

```text
validation_internal.json
```

---

## 5.2 Layer B：官方 3D Tiles Validator

开发/验收环境集成 CesiumGS `3d-tiles-validator`。

推荐：

```bash
npx 3d-tiles-validator \
  --tilesetFile <output>/tileset.json \
  --reportFile <acceptance>/validator-report.json
```

不要把 Node runtime 强制塞进最终应用；这里可以作为 CI / acceptance tool。

### Gate

最终生产验收：

```text
validator error severity = 0
```

warning 可以存在，但必须：

- 记录；
- 分类；
- 说明是否是 known acceptable warning；
- 不允许无上限忽略 warning。

---

# 6. Phase 13：真正安全的 staged commit

当前逻辑删除现有 `final_output` 后再 rename/copy，不够安全。

## 6.1 必须改为 sibling staging

始终在 final output 的同一父目录：

```text
/output-parent/
    final-output/
    .geoforge-stage-<task-id>/
    .geoforge-backup-<task-id>/
```

这样 staged → final 应在同一 filesystem。

## 6.2 Commit 事务

顺序：

```text
1. 处理全部写入 stage
2. internal validate
3. official validator（生产 acceptance；应用内可选）
4. fsync/close 所有文件
5. 如果 final 不存在：
      rename(stage, final)
6. 如果 final 已存在：
      rename(final, backup)
      rename(stage, final)
      成功 → 删除 backup
      失败 → rename(backup, final) 回滚
7. 成功后登记 artifact
```

### 禁止

commit 阶段禁止：

```text
remove final
copy stage -> final
```

不允许 copy 一半留下半成品。

如果 `rename` 因 filesystem / 权限失败：

```text
COMMIT_RENAME_FAILED
```

任务失败，原成果必须仍可用。

## 6.3 Cancel / Crash Test

自动测试以下场景：

- convert 中 cancel；
- rebuild 中 cancel；
- texture 中 cancel；
- validate 前 kill；
- commit 前 kill；
- final 已存在时模拟第二次 rename 失败。

每次检查：

```text
原 final hash 不变
不存在 partial final
stage 可被下次启动清理
task 状态正确
```

---

# 7. Phase 14：Zero-Python Release Runtime

生产 V1 安装后，用户不能被要求：

```text
安装 Python
创建 venv
pip install
设置 /workspace 路径
手工编译 processor
手工找 basisu
```

---

## 7.1 Processor

正式 Desktop 必须把 `processor` 作为 Tauri sidecar 或等价 bundled binary。

不得依赖：

```text
GEOFORGE_PROCESSOR
../../../target/debug/processor
/workspace/repos/...
```

环境变量只允许作为 developer override。

---

## 7.2 TopRebuild

`top_rebuild` 同样作为 sidecar，或者直接作为 `processor` library 链接。

两个方案任选其一：

### 方案 A：sidecar

```text
desktop
  └── processor
         └── top_rebuild sidecar
```

优点：边界明确。

### 方案 B：processor 直接依赖 top_rebuild crate

```text
desktop
  └── processor
         └── top_rebuild lib
```

优点：少一个进程。

如果当前代码已经稳定，优先 B，减少发布组件。

---

## 7.3 Converter

OSGB converter 是正式产品必需组件。

Windows release 必须能从安装目录解析，例如：

```text
resources/bin/_3dtile.exe
```

不能使用：

```text
/workspace/runtime/3dtile-bin/run.sh
```

保留 env override 仅供开发。

如果上游 converter 需要 DLL：

- 一并打包；
- 启动前 capabilities 检查；
- 缺 DLL 时给明确错误；
- 不让用户自己找 vcpkg。

---

## 7.4 KTX2

当前 process-tileset texture stage 仍依赖 Python post-process，需要移除正式依赖。

优先路线：

1. 抽取/复用 `top_rebuild::texture` 与 GLB/B3DM 读写能力；
2. 实现 Rust 版 tileset texture walker；
3. 递归处理 B3DM/GLB；
4. 调用 bundled BasisU CLI 或可链接库；
5. 更新 `KHR_texture_basisu`；
6. 再用 validator + Cesium 实测。

最终：

```text
texture.mode=keep       → no Python
texture.mode=ktx2-*     → no Python
```

Python 版本移动为：

```text
tools/experiments/
```

仅 regression。

---

## 7.5 Tauri Bundle

重点配置：

```text
apps/desktop/src-tauri/tauri.conf.json
```

Windows x86_64 至少打包：

```text
GeoForge 3D.exe
processor.exe（若 sidecar）
_3dtile.exe
converter runtime DLLs
basisu.exe（若使用 CLI）
Cesium 静态资源
```

不要提交 build output 到 Git，除非仓库已有明确 release artifact 策略。

---

# 8. Phase 15：生产级真实公开数据测试框架

真实验收不能依赖人工临时命令。

新增：

```text
scripts/acceptance/
    download_hk_pland.py
    prepare_hk_pland.py
    run_acceptance.py
    cesium_benchmark.mjs
    compare_tilesets.py

tests/acceptance/
    README.md
    thresholds.json
```

公开数据缓存：

```text
.geoforge-testdata/
```

加入 `.gitignore`。

结果：

```text
acceptance-results/<run-id>/
    environment.json
    dataset-manifest.json
    source-inventory.json
    converter.log
    rebuild.log
    validation_internal.json
    validator-report.json
    rebuild_metrics.json
    subtree_preservation.json
    cesium-baseline.json
    cesium-rebuild.json
    screenshots/
    result.md
```

## 8.1 environment.json

必须记录：

```text
git commit
OS
CPU
RAM
GPU
Rust version
Node version
converter version/hash
basisu version/hash
processor version
top_rebuild version
Cesium version
test start/end time
```

没有环境信息的性能数字不作为验收证据。

---

# 9. Phase 16：主真实数据集——香港规划署公开 3D Photo-realistic Model

本轮优先使用香港规划署 Planning Department 的公开 3D Photo-realistic Model。

官方页面：

```text
https://www.pland.gov.hk/pland_en/info_serv/3D_models/download.htm
```

官方说明：

```text
https://www.pland.gov.hk/pland_en/info_serv/3D_models/Remarks_for_the_3D_Photo-realistic_Model.pdf
```

该数据的价值：

- 真实航空摄影生成的高分辨率 photogrammetry；
- 覆盖香港岛部分区域和九龙部分区域；
- 官方同时提供：
  - OSGB
  - OBJ
  - Cesium 3D Tiles
- CSV 内有：
  - GRID_NAME
  - MIN_X / MIN_Y / MAX_X / MAX_Y
  - WGS84 bounds
  - OSGB_URL
  - CESIUM_URL
- 可用同一区域的官方 Cesium 3D Tiles 作为空间/视觉参考。

官方 ZIP 请求格式为：

```text
https://pdmap.pland.gov.hk/PLANDWEB/public/3d_photo_realistic_models/<FORMAT>/<GRID_NAME>_<FORMAT>.zip
```

不要在代码中硬编码某一个 grid。必须通过官方 CSV / bounds 选择连续区域。

## 9.1 版权与测试数据规则

该数据只用于本项目内部、非商业开发验证：

- 不提交 ZIP / OSGB / B3DM 到 Git；
- 不重新公开镜像；
- acceptance report 只保存统计、截图、hash、grid name；
- 下载前保存官方来源和访问日期；
- 如果页面条款变化，以当前官方条款为准。

---

## 9.2 自动选择测试区域

下载脚本必须：

1. 获取官方 OSGB CSV。
2. 读取 grid bounds。
3. 根据 `MIN_X/MIN_Y/MAX_X/MAX_Y` 建立邻接关系。
4. 自动寻找连续矩形或尽可能接近矩形的 grid 集。
5. 优先避免只依赖字符串 `tile_x_y` 猜邻接。

### Test A：真实 2×2

目的：快速调试。

```text
4 grids
```

不是最终验收，只用于修 bug。

### Test B：真实 4×4 —— 强制 Gate

必须找到：

```text
16 个连续真实 grid
```

如果官方数据不存在完整 4×4，则选择：

```text
>= 12 个连续 grid
```

但必须在报告里解释 coverage。

### Test C：更大真实区域 —— 强制 Gate

目标：

```text
8×8 = 64 grids
```

但考虑公开数据单 tile 可能很大，使用以下约束：

```text
预计新增下载 <= 20 GB
磁盘剩余空间 >= 2.5 × 预计下载
```

如果 8×8 超过 20 GB：

- 自动缩小；
- 选择 **16～36 个连续 grid** 的最大区域；
- 真实数据总输入至少应显著大于现有 OSGBny；
- 报告中记录为什么没有使用 8×8。

若机器已有足够资源，可继续到 8×8，不需要停下来询问用户。

---

# 10. 香港数据准备原则

PlanD 的 OSGB 分发结构不一定完全等同本工具默认 Smart3D 根目录结构。

测试脚本允许做**非破坏性 staging**：

```text
downloaded/
    原始 ZIP 解压结果

staged/
    Data/
       Tile_+xxx_+yyy/
          ...
    metadata.xml
```

但必须满足：

1. 不修改下载的原始文件。
2. 每个 staging 文件有 source mapping。
3. 不生成伪 mesh。
4. 不补不存在的 LOD。
5. 如果只是目录名转换，记录 rename mapping。
6. 坐标 metadata 不能凭空猜；优先使用数据自身 metadata / 官方 specification。
7. 如果 HK Island 与 Kowloon OSGB 结构不同，分别做 adapter，不把两类结构误认为同一种。

输出：

```text
dataset-manifest.json
```

包含：

```json
{
  "source": "Hong Kong Planning Department",
  "grids": [],
  "originalPaths": [],
  "stagedPaths": [],
  "mapping": [],
  "totalBytes": 0
}
```

---

# 11. Phase 16 验收流程

对真实 4×4 和更大区域分别执行。

---

## 11.1 Source inventory

转换前统计：

```text
grid count
OSGB file count
texture file count
input bytes
LOD depth distribution
metadata CRS
origin
root transform
```

随机抽 10 个 Block 输出 LOD/tree 结构。

---

## 11.2 OSGB → 3D Tiles baseline

先关闭 TopRebuild / KTX2：

```text
convert only
texture keep
rebuild false
```

必须：

- exit 0；
- 0 missing files；
- internal validator pass；
- Cesium official validator 0 errors；
- Cesium 能加载；
- 位置正确；
- 纹理完整。

如果 baseline converter 在真实公共数据上都失败，先修 converter integration，不能继续 TopRebuild 并宣布成功。

---

## 11.3 对照官方 Cesium 3D Tiles

对测试区域下载对应官方 CESIUM ZIP（若体积合理）。

不是要求字节/拓扑一致，而是比较：

```text
地理中心
coverage bbox
高度范围
主要地物位置
视觉朝向
数量级
```

允许模型切片方法不同。

### 空间门槛

在同一参考坐标系：

```text
horizontal center deviation <= 1 m
```

如果官方 source / metadata 存在已知 datum 差异，可以调整阈值，但必须有明确证据，不能凭观察修改。

高度：

```text
无系统性整体漂移
```

如果垂直基准不同，报告明确标注。

---

## 11.4 TopRebuild

对 baseline output 执行：

```text
rebuildTop = true
texture = keep
```

先排除 KTX2 对问题定位的干扰。

真实 4×4：

```text
L0 ~16
L1 ~4
L2 ~1
```

如果因边界/缺块不是严格 16→4→1，允许不完全四叉树，但必须：

- coverage 完整；
- 不重复；
- 不丢块；
- parent child 关系来自真实空间邻接。

---

## 11.5 TopRebuild 数据正确性 Gate

必须全部通过：

### 原子树

```text
原始 Block 数 = 输出 leaf external Block 数
random 10 block structure preserved = true
missing content URI = 0
```

### Geometry

```text
NaN = 0
Inf = 0
index out of range = 0
invalid primitive = 0
```

### Bounds

```text
每个 parent 覆盖全部 child bounds
uncovered child = 0
```

容差：

```text
0.10 m
```

### Transform

原始 child 重挂载：

```text
max world transform drift <= 0.001 m
```

GLB local float roundtrip：

```text
P99 <= 0.05 m
max <= 0.10 m
```

### geometricError

```text
parent > max(children)
all finite
all >= 0
```

不要求启发式已经“数学最优”，但必须通过实际 Cesium SSE 验证。

---

# 12. Boundary / Seam 生产验收

当前 `maxGap` 如果只是测相邻对象本身距离，不足以判断“简化引入的裂缝”。

本轮应增加：

```text
sourceBoundary
proxyBoundary
boundaryDisplacement
sourcePairGap
proxyPairGap
addedGap
```

### LockBorder 单体不变量

对被锁定边界顶点：

```text
max boundary displacement <= 1e-4 m
```

### 相邻 Block

用 source gap 作为 baseline。

验收：

```text
P95 addedGap <= 0.10 m
max addedGap <= 0.50 m
```

如果 source 本来就有明显裂缝：

```text
proxy 不得显著恶化 source
```

报告必须同时显示：

```text
sourceMaxGap
proxyMaxGap
addedMaxGap
sourceP95
proxyP95
addedP95
```

不要再把“两个本来不接触的测试 box 相距 60m”作为 seam 质量指标。

---

# 13. Simplification / Budget 生产验收

对每个 Proxy 记录：

```text
trianglesBefore
trianglesTarget
trianglesAfter
resultErrorMeters
groupCount
glbBytes
textureBytes
```

## 13.1 Budget

生产模式：

- budget 未达到可以 warning；
- 但不得无限超标。

建议 hard limit：

```text
trianglesAfter <= max(targetTriangles * 1.25, targetTriangles + 5000)
```

超过：

```text
PROXY_TRIANGLE_BUDGET_FAILED
```

如果 LockBorder 导致无法达到，允许自适应：

1. 保持边界锁定；
2. 适当提高 proxy budget；
3. 重新生成一次；
4. 记录实际值。

禁止为了达到数量强行解除边界保护。

## 13.2 result error

必须使用 meshoptimizer actual result error。

如果：

```text
resultError > targetError
```

记录：

```text
SIMPLIFICATION_ERROR_EXCEEDED
```

是否 hard fail 可以随质量档位决定，但 production 默认不能无提示。

---

# 14. Texture / KTX2 真实验收

先在 `texture=keep` 下完成全部几何 Gate，再开启 KTX2。

## 14.1 Texture keep

检查：

```text
有 baseColorTexture 的 source primitive
→ 输出 proxy 对应可见 material 仍应有纹理
```

必须统计：

```text
sourceTexturedPrimitiveCount
proxyTexturedPrimitiveCount
missingTextureBindingCount
textureDecodeErrorCount
```

要求：

```text
missingTextureBindingCount = 0
textureDecodeErrorCount = 0
```

不允许白模 / 黑模 / test solid texture。

## 14.2 KTX2

分别至少跑：

```text
ETC1S
```

UASTC 如果已有完整支持再跑；若未完成可以保持 V1 非默认。

检查：

- `KHR_texture_basisu` 合法；
- `image/ktx2` 有效；
- validator 通过；
- Cesium 加载；
- 无明显全黑、紫块、UV 错位。

### 生产默认

建议：

```text
keep
ktx2-etc1s
```

作为正式选项。

UASTC 不成熟时不要强行宣布支持。

---

# 15. Phase 17：Cesium 自动化 A/B 性能与视觉验收

必须建立可重复 Cesium benchmark，而不是只截图“看起来能加载”。

建议使用：

```text
Playwright 或 headless Chromium
```

固定：

```text
Cesium version
viewport
deviceScaleFactor
maximumScreenSpaceError
camera position
camera orientation
cache setting
network = localhost
```

## 15.1 两组

### Baseline

```text
OSGB → 3D Tiles
无 TopRebuild
```

### Candidate

```text
同一输出
+ TopRebuild
```

## 15.2 Camera set

对真实 4×4 和大区域，至少设：

```text
far-overview
mid
near
```

far-overview 必须完整覆盖测试区域。

## 15.3 记录

```text
timeToFirstTile
timeToOverviewReady
HTTP request count
3D Tiles content request count
bytes transferred
failed request count
tilesLoaded time
peak JS heap（可获取时）
Cesium statistics / selected tile count（可获取时）
```

## 15.4 Far-view 目标

目标不是追求一个绝对数字，而是证明 TopRebuild 产生实际收益。

在固定 far camera + SSE 下，至少满足其中两项，并且不能有明显回退：

```text
content request count 降低 >= 50%
transferred bytes 降低 >= 40%
timeToOverviewReady 改善 >= 25%
```

如果 baseline 本身已有优秀跨 Block HLOD 导致收益较小：

- 不能伪造；
- 报告真实结果；
- 解释 TopRebuild 是否仍有价值。

对本项目 converter 的 typical block-local LOD 输出，预期应能看到明显远景请求数下降。

---

# 16. Cesium 视觉 Gate

自动生成截图：

```text
baseline_far.png
rebuild_far.png
baseline_mid.png
rebuild_mid.png
baseline_near.png
rebuild_near.png
```

以及切换序列：

```text
far → mid → near
near → mid → far
```

必须检查：

- 无整块消失；
- 无错误世界位置；
- 无明显旋转/倒置；
- 无大面积白模；
- 无明显新裂缝；
- Proxy → original 的 REPLACE 时机合理；
- 拉近后原始高精度 Block 回来；
- 远处不加载所有细节 Block。

### 自动化不能完全判断视觉

因此 Agent 必须把截图和指标一起输出到 acceptance report。

可以做像素差辅助，但不能仅凭 pixel diff 判定三维质量。

---

# 17. geometricError / SSE 标定

真实数据上至少测试：

```text
maximumScreenSpaceError = 8
16
32
```

每组记录：

```text
far selected level
mid selected level
near selected level
requests
bytes
视觉截图
```

目标：

```text
far = 高层 Proxy
mid = 中层 Proxy
near = 原 Block
```

不能：

- 一直卡在 root proxy；
- 稍微靠近就全部加载最细；
- 频繁来回闪烁。

如果当前：

```text
proxyError = max(childError) + simplificationError
```

需要校准：

- 调 `sourceErrorRatio`；
- 调 source error heuristic；
- 调 proxy error floor。

但每次参数变化必须保留 benchmark 对比，避免“凭感觉调好看”。

最终把建议默认值写入：

```text
docs/product/PRODUCTION_DEFAULTS.md
```

---

# 18. 大数据内存与流式行为 Gate

Bottom-up 的目标之一是避免把完整城市 Mesh 一次放入内存。

## 18.1 记录

4×4 与更大真实数据：

```text
total input bytes
peak RSS
per-level time
largest proxy source bytes
largest proxy trianglesBefore
temporary disk peak
```

## 18.2 生产目标

在可用 RAM >= 16 GB 的测试机：

```text
peak RSS < 8 GB
```

且：

```text
大区域 peak RSS 不应近似按 Block 总数线性增长
```

更重要的相对 Gate：

```text
largeTest peakRSS <= max(2.5 × real4x4 peakRSS, 8 GB)
```

如果超出：

必须检查是否：

- 整个 tileset mesh 常驻；
- 所有 texture 同时 decode；
- payload map 保留大 `glb_bytes`；
- 上一级处理后没有释放下一级 mesh。

允许重构为：

```text
build parent
→ write
→ release children geometry
→ next group
```

不要用“测试机内存很大”掩盖内存模型问题。

---

# 19. Processor 生产级错误与可诊断性

所有失败必须在 JSONL 中有：

```json
{
  "type": "error",
  "code": "...",
  "message": "...",
  "stage": "...",
  "path": "...",
  "recoverable": false
}
```

建议正式错误码至少覆盖：

```text
MISSING_CRS
INPUT_INVALID
CONTENT_MISSING
CONTENT_INVALID
SOURCE_COVERAGE_INCOMPLETE
GRID_SPATIAL_MISMATCH
UNSUPPORTED_REFINE_ADD
UNSUPPORTED_REQUIRED_EXTENSION
UNSUPPORTED_BOUNDING_VOLUME
TRANSFORM_INVALID
PROXY_BUILD_FAILED
PROXY_TRIANGLE_BUDGET_FAILED
SIMPLIFICATION_ERROR_EXCEEDED
TEXTURE_ENCODE_FAILED
VALIDATION_FAILED
COMMIT_RENAME_FAILED
CANCELLED
```

UI 不要求为每个 code 定制页面，但日志中必须有明确原因和 path。

---

# 20. Phase 18：Windows Production Package

用户真实工作环境优先按：

```text
Windows 11 x86_64
```

验收。

## 20.1 Clean-machine 测试

必须在“不包含开发仓库”的环境中验证。

可以使用：

- Windows VM；
- GitHub Actions Windows runner 做 build；
- 另一份干净目录模拟安装环境；
- 如果当前 agent 只有 Linux，则至少产出 Windows CI + bundle，无法完成 GUI clean-machine 实测时必须标为 blocker，不能宣布最终 production-ready。

## 20.2 安装后禁止依赖

```text
Git
Rust
Node
Python
CMake
vcpkg
repo source tree
/workspace
环境变量
```

## 20.3 安装后检查

启动应用：

```text
health
capabilities
```

必须显示：

```text
processor = available
converter = available
topRebuild = available
basisu = available（若 KTX2 作为正式能力）
python = not required
```

## 20.4 用户流程 E2E

通过 GUI 完成：

```text
选择 OSGB
→ 输出目录
→ convert
→ rebuild
→ KTX2
→ 任务进度
→ 成功
→ 成果列表
→ Cesium preview
→ 打开目录
```

再完成：

```text
选择已有 3D Tiles
→ rebuild / texture
→ preview
```

至少主动取消一个真实任务：

```text
Cancel
```

确认：

- UI 状态 cancelled；
- 无半成品 final；
- 原成果不受影响。

---

# 21. 公开 3D Tiles 辅助兼容性测试

除了香港 OSGB 主测试，再使用公开 3D Tiles 做 parser / validator / preview 的兼容性测试。

---

## 21.1 CesiumGS 3D Tiles Samples

仓库：

```text
https://github.com/CesiumGS/3d-tiles-samples
```

用途：

- 规范边界；
- explicit / implicit；
- metadata；
- multiple contents；
- sparse quadtree；
- 不同 BV。

目的不是要求 TopRebuild 全支持，而是：

```text
支持 → 正确处理
不支持 → preflight 明确拒绝
```

不能 crash。

---

## 21.2 Utrecht Integrated Mesh

公开示例 tileset：

```text
https://tiles.arcgis.com/tiles/V6ZHFr6zdgNZuVG0/arcgis/rest/services/Utrecht_3D_Tiles_Integrated_Mesh/3DTilesServer/tileset.json
```

用于：

- Cesium preview remote compatibility；
- tileset preflight；
- 真实 integrated mesh 结构识别。

不要默认批量镜像整座城市；遵守服务条款。若要大量下载，先确认许可。

---

## 21.3 Strasbourg photogrammetry

Cesium 3D Tiles 社区公开资源列表中有 Strasbourg photomesh。

可作为备用真实 photogrammetry 3D Tiles compatibility case。

使用前 Agent 必须：

1. 找到当前公开官方/原始入口；
2. 确认测试访问允许；
3. 记录 source URL；
4. 不重新分发整套数据。

---

# 22. Preflight Capability Matrix

生产 UI 开始处理前必须先扫描并给出：

```json
{
  "inputType": "osgb|3dtiles",
  "compatible": true,
  "capabilities": {
    "convert": true,
    "rebuildTop": true,
    "textureKtx2": true
  },
  "reasons": []
}
```

对不支持：

```json
{
  "compatible": false,
  "capabilities": {
    "rebuildTop": false
  },
  "reasons": [
    {
      "code": "UNSUPPORTED_REFINE_ADD",
      "path": "root.children[2]"
    }
  ]
}
```

用户不能点开始之后跑 30 分钟才知道结构不支持。

---

# 23. 回归测试矩阵

每次真实数据修复不能破坏已有路径。

至少维护：

| Case | Convert | Rebuild | Texture | Expected |
|---|---:|---:|---:|---|
| one tile OSGB | yes | no | keep | PASS |
| one tile OSGB | yes | no | ETC1S | PASS |
| synthetic 4×4 | n/a | yes | keep | PASS |
| synthetic 16×16 | n/a | yes | keep | PASS |
| corrupt content | n/a | yes | keep | FAIL |
| missing subtree | n/a | yes | keep | FAIL |
| rotated OBB fixture | n/a | yes | keep | PASS |
| nested transform fixture | n/a | yes | keep | PASS |
| real HK 4×4 | yes | yes | keep | PASS |
| real HK 4×4 | yes | yes | ETC1S | PASS |
| real HK larger | yes | yes | keep | PASS |
| ADD tileset | n/a | rebuild | n/a | PRECHECK FAIL |
| implicit 1.1 sample | n/a | rebuild | n/a | PRECHECK FAIL or supported |
| cancel during rebuild | n/a | yes | n/a | CANCELLED + no partial |

---

# 24. GitHub CI / Release Gate

增加 CI，至少：

## Pull Request

Linux：

```text
cargo fmt --check
cargo clippy
cargo test -p top_rebuild
cargo test -p processor
npm ci
npm run build
```

Windows：

```text
cargo test -p top_rebuild
cargo test -p processor
Tauri compile/check
```

## Manual / Nightly Acceptance

由于真实公开数据太大，不放普通 PR CI。

增加：

```text
workflow_dispatch
```

运行：

```text
download public test subset
→ convert
→ rebuild
→ validate
→ benchmark
→ upload acceptance-results artifact
```

若公共下载速度/条款不适合 CI，则至少保留本地：

```text
python scripts/acceptance/run_acceptance.py ...
```

并把最近一次正式 acceptance report 作为 release artifact，而不是把原数据放仓库。

---

# 25. 最终生产验收清单

只有下面全部打勾，Agent 才允许输出：

```text
PRODUCTION_READY=true
```

---

## A. Architecture

- [ ] Qt 不在产品运行链路。
- [ ] Python 不在 release runtime。
- [ ] Desktop → Processor 边界稳定。
- [ ] 无 `/workspace/...` 等开发机硬编码依赖。
- [ ] Windows bundle 包含必要 sidecar/runtime。

## B. Source correctness

- [ ] release 不生成 synthetic replacement。
- [ ] corrupt/missing content fail-fast。
- [ ] Representation 使用完整 coverage frontier。
- [ ] 原 Block subtree 完整保留。
- [ ] transform world invariant 通过。
- [ ] oriented box / nested transform tests 通过。

## C. TopRebuild

- [ ] 真实 4×4 成功。
- [ ] 更大真实区域成功。
- [ ] Bottom-up 多级实际收敛。
- [ ] parent local frame。
- [ ] cross-child primitive grouping。
- [ ] meshoptimizer actual error。
- [ ] LockBorder。
- [ ] seam added-gap 在门槛内。
- [ ] no missing texture。
- [ ] REPLACE HLOD 正确。
- [ ] parent bounds 覆盖 child。
- [ ] geometricError 单调。

## D. Validation

- [ ] internal recursive validation 通过。
- [ ] CesiumGS 3d-tiles-validator error = 0。
- [ ] missing URI = 0。
- [ ] invalid B3DM/GLB = 0。
- [ ] NaN/Inf = 0。

## E. Runtime safety

- [ ] temp → validate → atomic rename。
- [ ] 已存在 final 的 rollback test 通过。
- [ ] cancel 不留下 partial final。
- [ ] crash / restart 后 task 状态合理。
- [ ] artifact 只在 commit 成功后登记。

## F. Texture

- [ ] keep 正常。
- [ ] ETC1S 正常。
- [ ] KHR_texture_basisu validator 通过。
- [ ] Cesium 实际显示正常。
- [ ] release 不需要 Python。

## G. Real public data

- [ ] Hong Kong PlanD 真实 4×4 或 >=12 contiguous grids 完成。
- [ ] Hong Kong 更大真实区域完成（目标 8×8；受 20 GB 下载预算约束时至少 16～36 grids）。
- [ ] 与官方对应 Cesium 数据空间范围没有明显异常。
- [ ] 所有测试数据来源、grid、hash、大小记录完整。

## H. Performance

- [ ] 记录真实耗时。
- [ ] 记录 peak RSS。
- [ ] 记录 temp disk。
- [ ] 大区域 RSS 满足本文目标。
- [ ] far-view A/B 至少两项达到：
  - [ ] request count -50%
  - [ ] bytes -40%
  - [ ] overview ready time -25%
- [ ] 没有性能明显回退。

## I. Visual / Cesium

- [ ] far / mid / near screenshots。
- [ ] 无整块消失。
- [ ] 无明显错位。
- [ ] 无大面积白模/黑模。
- [ ] LOD REPLACE 切换合理。
- [ ] SSE 8/16/32 标定完成。

## J. Product packaging

- [ ] Windows installer/package build 成功。
- [ ] clean environment 能启动。
- [ ] clean environment 能跑真实 E2E。
- [ ] 不需要 Git/Rust/Node/Python。
- [ ] capabilities 全部正确。

---

# 26. 最终报告格式

最终必须新增：

```text
docs/product/PRODUCTION_ACCEPTANCE.md
```

内容不得只写“PASS”。

至少包含：

## 26.1 Version

```text
commit
release version
build date
```

## 26.2 Supported Scope

明确写支持什么、不支持什么。

## 26.3 Public Dataset

```text
source
license/usage note
grid names
grid count
download bytes
unpacked bytes
OSGB count
texture count
```

## 26.4 Correctness

表格：

```text
subtree preservation
missing URI
validator errors
transform drift
bounds failures
NaN/Inf
seam metrics
```

## 26.5 Performance

Baseline vs Rebuild：

```text
build time
peak RSS
output bytes
proxy levels
far requests
far bytes
timeToOverviewReady
```

## 26.6 Visual Evidence

链接 acceptance screenshots。

## 26.7 Packaging

记录 clean-machine / Windows 验证结果。

## 26.8 Remaining Limitations

必须诚实列出。

---

# 27. 最终状态用语

### 如果所有 Gate 通过

可以写：

> GeoForge 3D V1 is production-ready for the documented Smart3D/ContextCapture-style OSGB workflow and compatible explicit 3D Tiles inputs. Top-level Proxy HLOD has been validated on contiguous real photogrammetry data, including a larger public dataset subset, with output validation, failure safety, Cesium A/B evidence, and Windows packaged runtime.

中文：

> GeoForge 3D V1 已达到本文限定支持范围内的生产可用标准：完成真实连续倾斜摄影数据的 OSGB 转换、Proxy HLOD 顶层重建、纹理处理、输出校验、失败安全、Cesium A/B 验证和 Windows 独立运行验证。

### 如果真实 4×4 通过，但更大数据/Windows 包未通过

必须写：

> `PRODUCTION_READY=false`  
> Real-data algorithm validation passed; production packaging / scale validation pending.

### 如果 synthetic 通过但真实数据失败

必须写：

> `PRODUCTION_READY=false`  
> Engineering prototype only. Real photogrammetry acceptance failed.

不要用：

```text
基本生产可用
接近生产可用
大部分生产可用
```

来模糊 Gate。

---

# 28. 推荐执行顺序摘要

严格按下面顺序：

```text
Phase 11
Correctness:
release no synthesize
subtree preservation
coverage frontier
transform / bounds
        ↓
Phase 12
Recursive validator
+ Cesium official validator
        ↓
Phase 13
Atomic commit / cancel / rollback
        ↓
Phase 14
Zero-Python runtime
+ Tauri sidecar/package
        ↓
Phase 15
Acceptance harness
        ↓
Phase 16
Hong Kong PlanD real 2×2
        ↓
Hong Kong PlanD real 4×4
        ↓
Hong Kong larger subset
        ↓
Phase 17
Cesium visual + request/bytes/time A/B
+ SSE calibration
        ↓
Phase 18
Windows clean-package E2E
        ↓
PRODUCTION_ACCEPTANCE.md
        ↓
PRODUCTION_READY=true / false
```

---

# 29. Agent 的第一步

不要继续写 UI。

立即从以下代码审计开始：

```text
crates/top_rebuild/src/tileset_writer.rs
    ensure_leaf_content
    synthesize_if_empty

crates/top_rebuild/src/adapter.rs
    collect_representations
    boundingVolume + transform semantics

crates/top_rebuild/src/types.rs
    Representation
    BoundingVolume

crates/processor/src/stages/validate.rs
    当前验证深度

crates/processor/src/stages/commit.rs
    final replacement / rollback

crates/processor/src/util.rs
    /workspace runtime paths

apps/desktop/src-tauri/
    processor / converter / basisu packaging
```

先输出一个简短审计记录：

```text
docs/product/PHASE_REPORTS/phase-11-audit.md
```

然后直接开始修改，不需要等待人工确认。

---

# 30. 这轮工作的核心判断

这一轮不是“继续把功能写完整”，而是把当前实现从：

```text
代码看起来实现了算法
```

推进到：

```text
真实倾斜摄影数据证明算法正确
+
失败不会破坏成果
+
输出可被规范工具验证
+
远景性能有量化收益
+
普通 Windows 用户安装后可以直接使用
```

只有到达后者，才算本轮完成。
