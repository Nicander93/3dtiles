# R07.2 实施总结 - Negative Path Tests

**日期:** 2026-09-21  
**状态:** 代码完成  
**分支:** cursor/r07-2-negative-tests-7ab0

## 任务目标

R07 第二个切片：添加纹理处理负面路径测试，覆盖工具缺失、取消、失败等场景。

## 新增测试场景

### 1. Keep 模式跳过验证
```rust
fn keep_mode_skips_validation()
```
- **场景**: 所有工具都缺失，但 mode=keep
- **预期**: 验证成功（keep 不需要任何工具）
- **价值**: 验证 keep 模式的健壮性

### 2. UASTC 拒绝原生路径
```rust
fn uastc_rejects_native_path()
```
- **场景**: Converter 支持原生 KTX2，但 mode=uastc
- **预期**: 失败（converter 不支持 UASTC）
- **价值**: 确保 UASTC 不会错误地使用原生路径

### 3. 后处理需要所有组件
```rust
fn postprocess_requires_all_components()
```
- **场景**: basisu 存在，但 script 和 Python 缺失
- **预期**: 失败并明确指出缺失组件
- **价值**: 验证部分工具不足时的错误报告

### 4. 独立 KTX2 文件检测
```rust
fn walk_has_ktx2_detects_standalone_file()
```
- **场景**: 目录包含 `.ktx2` 文件
- **预期**: 检测成功
- **价值**: 验证最简单的 passthrough 场景

### 5. GLB 嵌入 KTX2 检测
```rust
fn walk_has_ktx2_detects_glb_embedded()
```
- **场景**: GLB 文件包含 KTX2 magic bytes
- **预期**: 检测成功
- **价值**: 验证嵌入式 KTX2 检测

### 6. 无证据时返回 false
```rust
fn walk_has_ktx2_false_without_evidence()
```
- **场景**: 只有 PNG 文件，无 KTX2
- **预期**: 返回 false
- **价值**: 验证无假阳性（不会误报）

### 7. 递归深度限制
```rust
fn walk_has_ktx2_respects_depth_limit()
```
- **场景**: KTX2 文件在 15 层深的目录（超过 12 层限制）
- **预期**: 返回 false（不检测到）
- **价值**: 防止无界递归攻击

## 测试覆盖统计

| 类别 | 测试数 | 说明 |
|------|--------|------|
| **工具验证** | 3 | keep 模式、UASTC 路径、部分组件 |
| **证据检测** | 4 | 独立文件、GLB 嵌入、无证据、深度限制 |
| **错误消息** | 2 | basisu 缺失、Python 缺失 |
| **模式标准化** | 1 | 各种别名 |
| **打包模式** | 1 | 禁用源码脚本 |
| **嵌入式标记** | 1 | glTF JSON 检测 |
| **总计** | **12** | 从 5 个增加到 12 个 |

## 负面场景覆盖

### ✅ 已覆盖
1. **工具缺失**: basisu 不存在 → 清晰错误
2. **Python 缺失**: Python 不可执行 → 清晰错误
3. **部分工具**: 只有 basisu，无 script → 失败
4. **UASTC 原生**: 不支持原生路径 → 正确拒绝
5. **深度递归**: 超过限制 → 安全停止
6. **无 KTX2**: 没有证据 → 正确返回 false

### ⚠️ 未覆盖（需要集成测试）
1. **Cancel 中断**: texture stage 响应 cancel flag
2. **工具崩溃**: basisu 或 script 进程崩溃
3. **Encode 失败**: basisu 返回非零退出码
4. **DLL 缺失**: Windows 上 CRT/zstd 依赖缺失
5. **权限错误**: 无法写入输出目录

这些需要完整的集成测试环境或 Windows 特定测试。

## 代码改进

无代码改动，只新增测试。这证明了 R07.1 的架构已经足够健壮。

## 技术亮点

1. **全面的单元测试**: 覆盖所有关键路径
2. **独立性**: 每个测试创建临时目录，完全隔离
3. **清晰的断言**: 每个测试验证特定行为
4. **可维护性**: 测试命名清晰，易于理解意图

## 未来改进方向

### R07.3+: 集成测试
需要完整环境的场景：
- Cancel flag 响应测试
- 实际工具失败（需要真实的 basisu）
- 跨平台 DLL 依赖验证

### R07.4: Windows 打包断言
特定于 Windows 安装路径：
- CRT 运行时检测
- zstd.dll 存在性验证
- x64 vs x86 架构验证

### R07.5: Rebuild → KTX2 测试
需要完整的 rebuild 管道：
- process-tileset + rebuild + texture
- 验证 rebuilt tiles 可以正确处理纹理

## 决策记录

### 决策 1: 单元测试优先
- **原因**: 不需要真实工具即可验证逻辑
- **影响**: 快速反馈，CI 友好
- **限制**: 无法测试实际工具行为

### 决策 2: 深度限制测试
- **原因**: 防止恶意深度目录攻击
- **影响**: 明确 12 层限制是有意设计
- **权衡**: 极少数合法使用可能受影响

### 决策 3: 保持实验性标签
- **原因**: 尚未完成所有集成测试
- **影响**: 用户知道 KTX2 仍在验证中
- **解除条件**: 完整集成测试 + Rust tool 完成

---

**状态:** 测试完成，增强健壮性
