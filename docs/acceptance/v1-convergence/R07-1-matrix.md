# R07.1 Texture Matrix & Runtime Assertions

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r07-1-texture-matrix-7ab0

## 任务目标

R07 第一个切片：明确纹理处理能力矩阵、识别 gap、改进运行时断言。

## 当前纹理路径决策

### 路径 1: Native KTX2 (Converter 原生)
- **条件**: converter 支持 `--enable-texture-compress` + mode=ktx2-etc1s
- **优势**: 无需后处理，一次转换完成
- **限制**: 仅 ETC1S，无 UASTC 支持
- **检测**: `converter_supports_native_ktx2()` 检查 --help 输出

### 路径 2: Python 后处理
- **工具**: `tools/texture_ktx2/texture_ktx2.py`
- **依赖**: Python 3 + basisu 工具
- **支持**: ETC1S + UASTC
- **限制**: 需要 Python 运行时

### 路径 3: Rust 后处理 (未实现)
- **状态**: ❌ 不存在
- **规划**: 未来替代 Python

## 能力矩阵

| 场景 | Native | Python | Rust | 状态 |
|------|--------|--------|------|------|
| **OSGB → ETC1S** | ✅ 是 | ✅ 是 | ❌ 否 | 生产可用 |
| **OSGB → UASTC** | ❌ 否 | ✅ 是 | ❌ 否 | 仅 Python |
| **Tiles → KTX2 (ETC1S)** | ❌ 否 | ✅ 是 | ❌ 否 | 仅 Python |
| **Tiles → KTX2 (UASTC)** | ❌ 否 | ✅ 是 | ❌ 否 | 仅 Python |
| **KTX2 passthrough** | ✅ 是 | ✅ 是 | ✅ 是 | 全路径支持 |
| **Rebuild → KTX2** | ❌ 否 | ⚠️ 未测 | ❌ 否 | Gap |

### Gap 分析

1. **无 Rust 纹理工具**: 所有后处理依赖 Python
2. **Rebuild 路径未测**: process-tileset + rebuild + texture 组合
3. **UASTC 无原生路径**: converter 不支持 UASTC
4. **Passthrough 条件模糊**: `walk_has_ktx2()` 启发式检测

## 运行时断言改进

### 当前问题

**Python 缺失**:
```rust
// 当前：工具缺失时继续执行，在后续阶段失败
if !tools.python.is_file() {
    // 静默继续
}
```

**Basisu 缺失**:
```rust
// 当前：只在 validate 时检查
if !tools.basisu.is_file() {
    return Err("basisu missing");
}
```

### 改进方案

1. **明确前置检查**:
   - `validate_texture_mode()` 在 pipeline 开始前调用
   - 检查所有必需工具（python, basisu, texture script）
   - 清晰的错误消息，指出缺失的组件

2. **Install 模式断言**:
   - Packaged 模式下，断言 bundled 组件存在
   - 验证 basisu CRT/zstd x64 依赖
   - 检测 DLL 缺失或版本不匹配

3. **Passthrough 条件明确化**:
   - 文档化 `walk_has_ktx2()` 的检测逻辑
   - 明确何时跳过后处理
   - 避免假的 "预算满足"

## 代码改进

### 1. 增强工具检测

```rust
fn check_postprocess_tools(tools: &ToolPaths, mode: &str) -> Result<(), String> {
    if !tools.basisu.is_file() {
        return Err(format!(
            "basisu tool not found at {}. Install texture component or use mode=keep",
            tools.basisu.display()
        ));
    }
    
    if tools.texture_bin.is_file() {
        // Rust tool (future)
        return Ok(());
    }
    
    if !tools.texture_py.is_file() {
        return Err(format!(
            "texture_ktx2.py not found at {}. Install texture component or use mode=keep",
            tools.texture_py.display()
        ));
    }
    
    if !tools.python.is_file() {
        return Err(format!(
            "Python not found at {}. Python 3 is required for texture processing. Install Python or use mode=keep",
            tools.python.display()
        ));
    }
    
    if !command_available(&tools.python) {
        return Err(format!(
            "Python at {} is not executable. Check permissions or install Python 3",
            tools.python.display()
        ));
    }
    
    Ok(())
}
```

### 2. Passthrough 条件文档

```rust
/// Check if output directory already contains KTX2 textures
/// 
/// Passthrough conditions:
/// 1. Native KTX2 mode is available (converter supports it)
/// 2. At least one .ktx2 file exists in output
/// 3. Mode is ktx2-etc1s or ktx2-uastc
/// 
/// This is a heuristic: we assume if KTX2 files exist and native
/// mode was available, the converter already encoded textures.
/// We do NOT re-encode to avoid double compression.
/// 
/// False positive risk: User manually placed KTX2 files → we skip encoding.
/// This is acceptable because explicit texture=keep achieves same result.
fn walk_has_ktx2(dir: &Path) -> bool {
    // ... existing implementation
}
```

### 3. Packaging 断言

```rust
#[cfg(feature = "packaged")]
fn verify_bundled_components() -> Result<(), String> {
    let tools = tool_paths();
    
    // In packaged mode, bundled tools MUST exist
    if !tools.basisu.is_file() {
        return Err(
            "PACKAGING ERROR: basisu tool missing from bundle. \
             This is a build/installer issue, not a configuration error."
                .into(),
        );
    }
    
    // Check CRT dependencies on Windows
    #[cfg(target_os = "windows")]
    {
        // basisu requires MSVC runtime
        // This check would need platform-specific DLL enumeration
        // Placeholder for now
    }
    
    Ok(())
}
```

## 实施内容（本 PR）

本 PR **不实现** Rust texture tool，只是：

1. ✅ 创建 gap matrix 文档（本文件）
2. ✅ 改进 `postprocess_available()` 的错误消息
3. ✅ 增强 `validate_texture_mode_with_tools()` 错误详情
4. ✅ 添加 passthrough 条件的文档注释
5. ✅ 添加测试验证错误路径

**不包括**:
- ❌ Rust geoforge-texture 实现
- ❌ 生产级 KTX2 声明
- ❌ 移除实验性标签

## 测试策略

### 负面场景测试

1. **缺失 basisu**: 模拟工具路径不存在
2. **缺失 Python**: 模拟 Python 不可用
3. **Encode 失败**: 验证错误传播
4. **Cancel 中断**: 验证 texture stage 响应 cancel

### Matrix 覆盖

- ✅ OSGB → ETC1S (native)
- ✅ OSGB → ETC1S (postprocess)
- ⚠️ OSGB → UASTC (需要 Python)
- ⚠️ Tiles → KTX2 (需要 Python)
- ⚠️ Rebuild → KTX2 (未测试)

## 下一步 (后续 PR)

1. **R07.2**: 实现 Rust geoforge-texture 替代 Python
2. **R07.3**: 添加 rebuild → KTX2 测试覆盖
3. **R07.4**: Windows packaging CRT/DLL 验证
4. **R07.5**: 移除实验性标签（如果 Rust tool 完成）

## 决策记录

### 决策 1: 保留 Python 路径
- **原因**: Rust texture tool 尚不存在
- **影响**: 短期内仍需 Python 依赖
- **风险**: 用户必须安装 Python 使用 UASTC

### 决策 2: Native KTX2 优先
- **原因**: converter 原生支持更快、更简单
- **影响**: ETC1S 推荐使用原生路径
- **限制**: UASTC 必须使用后处理

### 决策 3: 实验性标签保留
- **原因**: Rust tool 未完成，Python 依赖未消除
- **影响**: 文档和 UI 仍标记 KTX2 为实验性
- **解除条件**: Rust tool 完成 + 充分测试

---

**状态:** 文档完成，代码改进最小化
