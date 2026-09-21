# D0 Tiny Fixtures

**目的**: 提供最小可运行的 3D Tiles fixtures 用于 smoke testing 和 CI regression testing。

## 包含的 Fixtures

### single-tile

**描述**: 单个 B3DM tile 的最小 tileset

**文件**:
- `tileset.json`: 根 tileset，单个 tile 引用
- `tile.b3dm`: 最小有效 B3DM（104 bytes）

**特征**:
- 有效的 3D Tiles 1.0 格式
- 最小 B3DM header (28 bytes)
- Feature table: `BATCH_LENGTH: 0`
- 空的 GLB (44 bytes, 只有 asset metadata)
- 无真实几何、无纹理

**用途**:
- Smoke test: processor 可以读取和验证 tileset
- Layer A validator: 验证结构正确性
- 输出格式检查: 确认生成有效的 tileset

**限制**:
- **不代表生产数据**: 无几何、无纹理、无层级
- **不测试空间质量**: 需要 D1/D2 fixtures
- **不测试性能**: 数据量过小

## License

**License**: MIT  
**来源**: Repository-created, 手工构造  
**可分发**: 是

这些 fixtures 是为测试目的专门创建的，不包含任何外部来源的数据。

## 使用

```bash
# 使用 processor 处理 D0 fixture
cargo run --release -p processor -- process-tileset \
  --input docs/acceptance/v1-convergence/fixtures/d0-tiny/single-tile \
  --output /tmp/d0-output

# 运行完整的 D0 acceptance suite
docs/acceptance/v1-convergence/scripts/run-acceptance.sh --level d0
```

## 预期行为

对 `single-tile` 运行 processor：
- ✅ 应该成功完成
- ✅ Layer A 验证应该通过（无 error）
- ✅ 输出应该包含有效的 `tileset.json`
- ⚠️ 可能有 warnings（例如：空 GLB, 无几何）
- ✅ 不应该 crash 或产生无效输出

## 扩展 D0 (Future)

可能添加的 fixtures：
- `multi-tile`: 多个 tile 的层级结构
- `with-texture`: 带简单纹理的 tile
- `i3dm-single`: 单个 I3DM tile
- `pnts-single`: 单个 PNTS tile

当前 R08.1 只包含 `single-tile` 用于建立 harness。
