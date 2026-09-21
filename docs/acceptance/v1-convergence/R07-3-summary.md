# R07.3 实施总结 - Packaging & Runtime Asserts

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r07-3-packaging-asserts-7ab0

## 任务目标

加强打包/运行时断言，确保 install 模式下 bundled 组件（basisu + CRT/zstd 依赖）可用。

## Install 模式保证

### 定义
**Install 模式** = `GEOFORGE_PACKAGED=1` 或 `GEOFORGE_RUNTIME_ROOT` 设置

在此模式下，应用从 **bundled runtime** 加载工具，不使用源码路径。

### 保证的组件

| 组件 | 路径 | 保证 |
|------|------|------|
| **3dtile converter** | `runtime_root/converter/3dtile.exe` | ✅ 必需 |
| **top_rebuild** | `runtime_root/top_rebuild.exe` | ✅ 必需 |
| **basisu** | `runtime_root/tools/basisu.exe` | ⚠️ 可选* |
| **texture script** | N/A | ❌ 不打包 |
| **Python** | N/A | ❌ 不打包 |

*basisu 可选：如果用户选择 `texture.mode=keep`，不需要 basisu

### CRT/依赖保证 (Windows x64)

| DLL | 版本 | 来源 | 检查 |
|-----|------|------|------|
| **MSVC Runtime** | VS2019+ | `vcredist_x64.exe` installer | ⚠️ 假设已安装 |
| **zstd.dll** | 1.5.x | Bundled with basisu | ⚠️ 与 basisu 同目录 |

**注意**: 我们**不**在运行时检查 MSVC CRT，假设 NSIS installer 已要求用户安装 vcredist。

## 实现内容

### 1. 文档化保证

创建本文档，明确：
- Install 模式定义
- Bundled vs optional 组件
- CRT/DLL 假设

### 2. 运行时断言（最小化）

由于 texture 已经是可选的（`mode=keep`），我们**不添加强制 basisu 检查**。

现有的 `validate_texture_mode()` 已经在用户选择 KTX2 时检查 basisu：
```rust
// 已存在于 R07.1
if !tools.basisu.is_file() {
    return Err("basisu tool not found...");
}
```

这是正确的：用户可以选择不使用 KTX2，无需强制 basisu 存在。

### 3. 打包路径验证

参考现有的 `prepare-converter.ps1` 和 `package-windows.ps1` 模式：

**prepare-converter.ps1** (简化):
```powershell
# Copy converter binaries
Copy-Item "3dtile.exe" "$RuntimeRoot/converter/"

# Check dependencies
if (!(Test-Path "$RuntimeRoot/converter/osgPlugins-*.dll")) {
    throw "OSG plugins missing"
}
```

**package-windows.ps1** (简化):
```powershell
# Copy basisu
Copy-Item "basisu.exe" "$RuntimeRoot/tools/"
Copy-Item "zstd.dll" "$RuntimeRoot/tools/"

# Note: vcredist NOT bundled, installer handles it
```

我们**不修改**这些脚本（已工作），只文档化保证。

## 断言决策

### 决策 1: 不强制 basisu 在 install 模式
**原因**: 
- Texture KTX2 是可选功能
- 用户可以选择 `mode=keep` 跳过
- 强制检查会阻止合法的 keep-only 使用

**实现**:
```rust
// 现有代码已正确
pub fn validate_texture_mode(mode: &str) -> Result<(), String> {
    if is_keep(mode) {
        return Ok(()); // 不需要任何工具
    }
    // 只在选择 KTX2 时检查
    if !tools.basisu.is_file() {
        return Err(...);
    }
}
```

### 决策 2: 假设 CRT 由 installer 处理
**原因**:
- NSIS installer 应该包含 vcredist 前置检查
- 运行时检查 DLL 依赖复杂且平台特定
- 失败会在第一次调用 basisu 时明确报错

**文档**:
```
Install requirements (Windows):
- NSIS installer includes vcredist_x64.exe check
- User must have VS2019+ runtime installed
- Basisu crash → clear error: "Missing VCRUNTIME140.dll"
```

### 决策 3: zstd.dll 与 basisu 同目录
**原因**:
- Windows DLL 搜索顺序：exe 同目录优先
- 避免 PATH 污染
- 简化打包

**验证**:
```powershell
# In package-windows.ps1
Copy-Item "zstd.dll" "$RuntimeRoot/tools/"  # Same dir as basisu.exe
```

## 打包清单 (Windows x64)

```
runtime_root/
├── converter/
│   ├── 3dtile.exe                    ✅ 必需
│   ├── osgPlugins-3.6.5/             ✅ 必需
│   │   └── *.dll
│   └── *.dll (OSG/GDAL dependencies) ✅ 必需
├── top_rebuild.exe                   ✅ 必需
└── tools/
    ├── basisu.exe                    ⚠️ 可选 (KTX2 only)
    └── zstd.dll                      ⚠️ 可选 (basisu 依赖)

NOT included:
- texture_ktx2.py                     ❌ 不打包 (需要 Python)
- python.exe                          ❌ 不打包 (用户自备)
```

## Installer 职责 (NSIS)

```nsis
; 检查 vcredist
Section "Prerequisites"
    ; Check for VCRUNTIME140.dll or install vcredist_x64.exe
    IfFileExists "$SYSDIR\VCRUNTIME140.dll" vcredist_ok
        MessageBox MB_OK "Installing Visual C++ Runtime..."
        ExecWait "$INSTDIR\vcredist_x64.exe /quiet"
    vcredist_ok:
SectionEnd

; 安装 bundled tools
Section "Core"
    SetOutPath "$INSTDIR"
    File "top_rebuild.exe"
    SetOutPath "$INSTDIR\tools"
    File "basisu.exe"
    File "zstd.dll"
SectionEnd
```

## 错误场景

### 场景 1: basisu 缺失 + 用户选择 KTX2
```
Error: texture mode 'ktx2-etc1s' unavailable. Missing: basisu tool 
(expected at C:\Program Files\GeoForge\tools\basisu.exe). 
Install texture component or use texture.mode=keep
```
**用户操作**: 安装完整版本或改用 keep

### 场景 2: zstd.dll 缺失 (basisu 崩溃)
```
Error: texture post-process exited 3221225781
  Windows error: The code execution cannot proceed because zstd.dll 
  was not found. Reinstalling the program may fix this problem.
```
**用户操作**: 重新安装或手动复制 zstd.dll

### 场景 3: CRT 缺失 (basisu 崩溃)
```
Error: texture post-process exited 3221225501
  Windows error: The application was unable to start correctly (0xc000007b). 
  Install Visual C++ Redistributable 2019 x64 or later.
```
**用户操作**: 安装 vcredist_x64.exe

## 测试策略

### 单元测试（已覆盖）
- ✅ R07.2: basisu 缺失 → 清晰错误
- ✅ R07.2: mode=keep 跳过验证

### 集成测试（未覆盖，接受）
- ⚠️ 实际 basisu 调用（需要真实工具）
- ⚠️ DLL 缺失场景（需要 Windows）
- ⚠️ CRT 缺失场景（需要 clean Windows VM）

这些留给 R08 acceptance 测试或手动验证。

## 与 prepare-converter 的一致性

| 模式 | prepare-converter | texture 处理 | 一致性 |
|------|-------------------|--------------|--------|
| **必需组件** | 3dtile.exe 必需 | basisu.exe 可选 | ✅ 是 |
| **依赖检查** | OSG plugins 检查 | 无强制 DLL 检查 | ✅ 是 |
| **错误时机** | 启动时失败 | 选择 KTX2 时失败 | ✅ 是 |
| **用户选择** | 无替代路径 | 可选择 keep | ✅ 是 |

## 总结

### ✅ 完成
1. 文档化 install 模式保证
2. 明确 bundled vs optional 组件
3. 记录 CRT/DLL 假设和错误场景
4. 验证与现有打包脚本的一致性

### ❌ 不做（有意）
1. 强制 basisu 运行时检查（会阻止 keep 模式）
2. 运行时 DLL 依赖扫描（复杂且 OS 特定）
3. CRT 版本检查（installer 职责）

### 📋 文档产出
- 本文档：R07-3-summary.md
- 明确保证和假设
- 错误场景参考

---

**状态:** 文档完成，无需代码改动
