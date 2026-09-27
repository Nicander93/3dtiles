# P8验收文档：主程序准备与打包就绪

## 任务目标

GeoForge large-data 计划 **P8 (main-app half, prep)** 验收标准。本阶段完成主程序侧的进度事件接入和打包脚本审查，为converter v0.2.3发布做准备。

## 1. 进度事件接入 ✅

### 1.1 Convert Stage

**实现位置**: `crates/processor/src/stages/convert.rs`

**已实现功能**:
- 在convert stage启动时调用 `ProgressThrottle::report()` 
- 传递参数:
  - `stage`: `Stage::Convert`
  - `completed`: 0 (初始值)
  - `total`: 0 (未知时)
  - `parallelism`: `Some(threads)` - 转换器实际使用的线程数
  - `resource_wait`: `false` - convert阶段暂无资源准入机制

**说明**:
- P2转换器侧的block级进度尚未实现，主程序这边已准备好接收
- 当前发送初始进度以向UI展示并行度
- 限频机制 (400ms) 已通过 `ProgressThrottle` 实现

### 1.2 Rebuild Stage

**实现位置**: `crates/processor/src/stages/rebuild.rs`

**已实现功能**:
- 在rebuild stage启动时调用 `ProgressThrottle::report()`
- 传递参数:
  - `stage`: `Stage::Rebuild`
  - `completed`: 0
  - `total`: 0
  - `parallelism`: `Some(budget.rebuild_workers())` - P5已实现的并行worker数
  - `resource_wait`: `false`

**说明**:
- top_rebuild二进制内部的层级进度未通过进度事件传递
- 主程序显示rebuild并行度，为后续P5完整进度报告预留接口

### 1.3 Texture Stage

**实现位置**: `crates/processor/src/stages/texture.rs`

**已实现功能**:
- 在texture post-process启动时调用 `ProgressThrottle::report()`
- 传递参数:
  - `stage`: `Stage::Texture`
  - `completed`: 0
  - `total`: 0
  - `parallelism`: `Some(file_workers)` - P6已实现的并行文件worker数
  - `resource_wait`: `false`

**说明**:
- texture_ktx2.py的文件级进度暂未通过stdout传递
- 显示file workers并行度，配合P7 UI显示

### 1.4 限频机制

**实现位置**: `crates/processor/src/progress_throttle.rs` (P7已实现)

**特性**:
- 最小更新间隔: 400ms
- 避免高频进度更新阻塞Tauri前端
- force_flush在阶段结束时确保最后状态送达

## 2. P8验收包 (文档部分)

### 2.1 Windows离线安装清单

#### 必需组件

| 组件 | 版本 | 来源 | 说明 |
|------|------|------|------|
| 主程序安装包 | V1.x | `apps/desktop/src-tauri/` Tauri打包 | 包含processor.exe + UI |
| converter运行包 | v0.2.2 (当前) / v0.2.3 (待发布) | GitHub Release | _3dtile.exe + OSG/GDAL/PROJ + MSVC DLLs |
| top_rebuild.exe | 随主程序 | Rust编译产物，打包入runtime | 顶层重建核心 |
| basisu工具 | 随converter或单独 | vcpkg / 单独下载 | 可选，纹理KTX2编码 |

#### 离线验证流程

```powershell
# 1. 断网环境 (禁用网络适配器)
Disable-NetAdapter -Name "以太网" -Confirm:$false

# 2. 安装主程序 (MSI或便携包)
.\GeoForge-Setup.msi /quiet

# 3. 验证converter运行包存在
Test-Path "C:\Program Files\GeoForge\resources\runtime\converter\_3dtile.exe"

# 4. 验证MSVC DLLs
$dlls = @("msvcp140.dll", "msvcp140_2.dll", "vcruntime140.dll", "vcruntime140_1.dll")
$converterDir = "C:\Program Files\GeoForge\resources\runtime\converter"
foreach ($dll in $dlls) {
    if (-not (Test-Path (Join-Path $converterDir $dll))) {
        Write-Error "Missing $dll"
    }
}

# 5. 运行OSGB转换任务 (小样本)
# 通过UI或CLI提交任务，监控任务日志

# 6. 验证恢复功能
# 中断任务后重新恢复，检查P3 manifest机制

# 7. 测试rebuild + texture
# 确认完整流程不依赖网络下载
```

### 2.2 OSGB转换+恢复+重建+纹理验收

#### 测试样本

| 类型 | 规模 | 用途 |
|------|------|------|
| fixture 4×4 | 16 blocks | 结构回归 |
| 小样本 | 1-5 GB, 50-200 blocks | 快速功能验证 |
| 中样本 | 20-50 GB, 500-2k blocks | 持续处理与恢复 |
| 大样本 (可选) | 100-200 GB | 容量验证（标记"待验证"如无法实测） |

#### 验收项目

- [x] **P1**: 资源配置生效 (cpuWorkers / memoryBudgetMiB / ioWorkers)
- [x] **P2 (部分)**: Block manifest机制存在，转换器侧流式输出待实现
- [x] **P3**: 任务失败后恢复，只重跑失败/未完成单元
- [x] **P4**: 重建阶段O(N²)复杂度消除
- [x] **P5**: 同层proxy并行构建，内存准入控制
- [x] **P6**: 纹理文件并行编码，file workers正常工作
- [x] **P7**: UI显示已完成单元数、并行度、资源等待标识
- [ ] **P8**: 进度事件parallelism字段在各阶段正确传递 (**本阶段**)

#### FBX/OBJ回归

- [ ] FBX单模型转换不受OSGB改动影响
- [ ] OBJ坐标/UV/材质保持正确
- [ ] 模型转换仍使用单线程模式 (不与OSGB线程配置冲突)

### 2.3 双仓版本组合表

| 主程序版本 | 主程序 commit | converter版本 | converter commit | 说明 |
|-----------|--------------|--------------|-----------------|------|
| P0-P7集成 | 7034033 (PR#40) | v0.2.2 | c1400d5 | P7 merge，UI+进度 |
| **P8 (本PR)** | **待提交** | v0.2.2 (暂用) | c1400d5 | 进度事件接入+打包就绪 |
| P8+v0.2.3 | 同上 | **v0.2.3 (待发布)** | **待提供** | coordinator提供Release URL+sha256 |

#### 版本锁定文件

- 主程序: `apps/desktop/config/converter-runtime.json`
  - `version`: 转换器版本号
  - `windowsX64.url`: Release zip下载链接
  - `windowsX64.sha256`: 完整性校验

- converter能力要求:
  - `modelConfigVersion`: 1
  - `formats`: ["fbx", "obj"]
  - `projectedGeoreference`: true
  - `georeferenceModes`: ["projected"]

### 2.4 状态标注

当前交付状态：**机制完成，容量待验证**

#### 已完成项

- [x] 进度事件协议 (progress_with_detail)
- [x] UI进度显示面板 (completed/total/parallelism/resourceWait)
- [x] 恢复机制 (P3 manifest + checkpoint)
- [x] 重建并行 (P5 bounded workers)
- [x] 纹理并行 (P6 file workers)
- [x] 打包脚本支持多版本converter共存

#### 待后续验证

- [ ] 20-50 GB 真实OSGB持续处理无OOM
- [ ] 100-200 GB 极限容量 (需真实数据+配置Windows机器)
- [ ] 内存准入在不同预算下的实际准入/等待行为
- [ ] 长时间运行后的内存泄漏检查

**说明**: 没有实测百GB数据时，不声称"支持百GB"。交付文档明确标注"容量待验证"，提供机制和验证方法，等待后续硬件+数据配套。

## 3. 打包脚本审查

### 3.1 prepare-converter.ps1

**位置**: `apps/desktop/scripts/prepare-converter.ps1`

**当前功能** (v0.2.2):
- 从 `converter-runtime.json` 读取版本、URL、SHA256
- 下载并校验zip包
- 解压到 `dist/runtime/converter/`
- 自动捆绑MSVC CRT DLLs (如Release包未包含)
- 验证converter能力 (--capabilities-json)

**v0.2.3就绪检查**:
- [x] 支持多版本缓存 (`.cache/converter/$version/`)
- [x] SHA256必须finalized，否则拒绝 (或用 `-LocalZip`)
- [x] MSVC DLLs自动搜索路径覆盖CI和本地环境
- [x] 能力验证确保FBX/OBJ/modelConfigVersion=1

**注意事项**:
- 脚本不会删除旧版本 (v0.2.2) 的运行包
- 新版本下载到独立缓存目录
- 安装包内只保留 `converter-runtime.json` 指定的版本

### 3.2 prepare-runtime.ps1

**位置**: `apps/desktop/scripts/prepare-runtime.ps1`

**当前功能**:
- 调用 `prepare-converter.ps1` 下载converter
- 编译并复制 `processor.exe` 和 `top_rebuild.exe`
- 统一复制MSVC DLLs到 `dist/runtime/bin/`
- 生成 `manifest.json` (文件清单+SHA256)

**v0.2.3就绪检查**:
- [x] 支持 `-ConverterZip` 参数 (本地测试用)
- [x] CRT DLLs从converter目录复制到bin目录
- [x] manifest记录所有文件哈希，便于完整性检查

**打包集成**:
- Tauri `tauri.conf.json` 的 `bundle.resources` 应包含 `dist/runtime/**`
- 或在Tauri beforeBuildCommand中调用 `prepare-runtime.ps1`

## 4. TODO区域：v0.2.3运行包pin更新

**等待事项**: converter仓库发布v0.2.3 Release

**届时需更新**:

1. **apps/desktop/config/converter-runtime.json**:
   ```json
   {
     "version": "0.2.3",
     "windowsX64": {
       "url": "https://github.com/Nicander93/geoforge-converter/releases/download/v0.2.3/geoforge-converter-0.2.3-windows-x64.zip",
       "sha256": "TODO_COORDINATOR_TO_SUPPLY"
     }
   }
   ```

2. **本文档 §2.3双仓版本组合表** 的v0.2.3行:
   - converter commit: 从Release tag获取
   - 确认新版本包含P2进度事件支持 (如已实现)

3. **验证清单**:
   ```powershell
   # 在PR分支上测试
   git checkout cursor/p8-main-app-prep-d91e
   # 手动编辑 converter-runtime.json (填入真实URL+sha256)
   apps/desktop/scripts/prepare-converter.ps1
   # 验证下载+解压+能力检查通过
   ```

**协调说明**: coordinator发布v0.2.3后提供:
- GitHub Release URL
- zip文件SHA256 (64位小写hex)
- converter commit SHA (打tag的commit)

在收到上述信息前，本PR使用v0.2.2完成代码变更和文档。

## 5. 不声称未验证的能力

### 当前交付范围

- [x] 进度事件协议完整 (parallelism + resourceWait)
- [x] UI能显示进度详情
- [x] 打包脚本支持版本切换

### 不在P8范围

- [ ] P2转换器侧block流式输出 (converter仓库任务)
- [ ] 实测百GB OSGB (需硬件+数据)
- [ ] 内存准入硬性上限执行 (P1软预算已实现，硬限制需进程隔离或更复杂监控)
- [ ] 分布式/多节点处理 (计划范围外)

## 6. 验收签字

| 项目 | 状态 | 备注 |
|------|------|------|
| Convert进度事件 | ✅ 已完成 | 传递parallelism=threads |
| Rebuild进度事件 | ✅ 已完成 | 传递parallelism=rebuild_workers |
| Texture进度事件 | ✅ 已完成 | 传递parallelism=file_workers |
| P8_ACCEPTANCE.md | ✅ 已完成 | 本文档 |
| prepare-converter.ps1审查 | ✅ 已审查 | 支持v0.2.3 pin |
| prepare-runtime.ps1审查 | ✅ 已审查 | 打包流程完整 |
| v0.2.3 TODO | 📝 待协调 | 等待Release+URL+sha256 |

---

**交付状态**: 主程序侧P8准备工作完成，v0.2.3 pin待coordinator提供Release详情后更新。
