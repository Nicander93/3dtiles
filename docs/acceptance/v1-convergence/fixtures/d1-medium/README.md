# D1 Medium Fixtures

**目的**: 提供中等规模的 3D Tiles fixtures 用于空间质量 spot check 和集成测试。

**状态**: 🚧 R08.2 定义，R08.3 实现

## 规格

- **大小**: 10-100MB
- **覆盖**: 真实几何、纹理、层级
- **CI 可行**: ⚠️ 可能（取决于大小和许可）
- **用途**: 空间质量验证、真实几何测试

## 计划的 Fixtures

### small-grid (R08.3 实现)

**方案**: Tiny synthetic fixture with grid structure

**内容**:
- 2x2 grid of Tile blocks (Tile_+000_+000 至 Tile_+001_+001)
- 每个 block: 2-3 个 tiles (简单层级)
- B3DM 包含最小几何（例如：简单立方体或平面）
- 总大小: ~1-5MB

**特征**:
- ✅ License: MIT (repository-created)
- ✅ 可提交到 repo
- ✅ CI 可运行
- ⚠️ 合成几何（非真实建筑）

**实现**: R08.3

**结构**:
```
small-grid/
├── tileset.json          # Root tileset
├── Tile_+000_+000/
│   ├── tileset.json
│   ├── L0.b3dm
│   ├── L1.b3dm
│   └── L2.b3dm
├── Tile_+000_+001/
│   └── ...
├── Tile_+001_+000/
│   └── ...
└── Tile_+001_+001/
    └── ...
```

### cesium-sample (R08.3+ 调研)

**方案**: 使用 Cesium 示例数据

**候选来源**:
1. **Cesium 3D Tiles Samples**
   - URL: https://github.com/CesiumGS/3d-tiles-samples
   - License: Apache 2.0 或 CC-BY
   - 示例: Tileset/, TilesetWithDiscreteLOD/, etc.

2. **其他开源数据集**
   - 待调研

**实现**: R08.3+ （如果许可允许）

**结构**:
```
cesium-sample/
├── README.md           # 来源和许可说明
├── download.sh         # 下载脚本
└── data/               # 下载后的数据（gitignored）
```

## 获取说明

### small-grid

**当前状态**: 未实现

**R08.3 实施计划**:
1. 手工创建或脚本生成 2x2 grid
2. 每个 tile 包含简单几何（立方体或平面）
3. 提交到 repo

### cesium-sample

**当前状态**: 未调研

**R08.3+ 实施计划**:
1. 调研 Cesium 示例数据许可
2. 选择合适的示例（大小 <100MB）
3. 创建 download.sh 脚本
4. 文档化许可和来源

## 使用

```bash
# R08.3 实现后
cd docs/acceptance/v1-convergence

# 运行 D1 small-grid (R08.3+)
./scripts/run-acceptance.sh --level d1 --fixture small-grid

# 下载和运行 cesium-sample (如果可用)
cd fixtures/d1-medium/cesium-sample
./download.sh
cd ../../..
./scripts/run-acceptance.sh --level d1 --fixture cesium-sample
```

## 预期行为

对 `small-grid` 运行 processor：
- ✅ 应该成功完成
- ✅ Layer A 验证通过
- ✅ Spatial quality hooks 运行（即使部分 "not checked"）
- ✅ 输出包含 rebuild 后的 tileset
- ⚠️ 可能有 warnings（GE 或 BV 松散）

## License

### small-grid
- **License**: MIT
- **来源**: Repository-created, 手工构造
- **可分发**: 是

### cesium-sample (待定)
- **License**: 待验证（目标 Apache 2.0 或 CC-BY）
- **来源**: Cesium 3D Tiles Samples 或其他开源
- **可分发**: 取决于许可

---

**状态**: R08.2 定义完成，R08.3 实现
**下一步**: 实现 small-grid synthetic fixture
