# Work Unit Resume

## 概述

GeoForge 支持工作单元级别的恢复功能，允许在任务失败或中断后从上次状态继续执行，避免重新处理已完成的工作单元。

## ResumePolicy 选项

任务配置中的 `execution.resumePolicy` 控制恢复行为：

### `off` (默认)

- 不支持恢复
- 如果临时工作目录已存在，任务将拒绝执行
- 适用于：快速任务、开发调试、确保干净状态的场景

```json
{
  "execution": {
    "resumePolicy": "off"
  }
}
```

### `retain-on-failure`

- 失败或取消时保留临时工作目录
- **不会**自动从保留的状态恢复
- 主要用于诊断：保留中间结果以便人工检查
- 下次执行时仍需要手动清理或使用 `resume` 策略

```json
{
  "execution": {
    "resumePolicy": "retain-on-failure"
  }
}
```

### `resume`

- 支持从上次失败/中断状态恢复
- 自动跳过已成功完成的工作单元
- 仅重新执行失败或未完成的工作单元
- 适用于：大数据集、长时间运行的任务

```json
{
  "execution": {
    "resumePolicy": "resume"
  }
}
```

## 工作清单

工作清单 (`.geoforge-work-manifest.json`) 存储在临时工作目录中，记录每个工作单元的状态：

### 清单结构

```json
{
  "version": 1,
  "taskId": "task-001",
  "toolVersions": {
    "processorVersion": "0.1.0",
    "converterVersion": "0.2.2"
  },
  "units": {
    "block-01": {
      "jobKey": "block-01",
      "unitType": "convert",
      "status": "succeeded",
      "attempts": 1,
      "inputFingerprint": "abc123...",
      "paramHash": "def456...",
      "outputChecksum": "789xyz...",
      "dependencies": [],
      "lastUpdatedMs": 1234567890000
    }
  }
}
```

### 工作单元状态

- **`pending`**: 尚未开始
- **`running`**: 正在执行中
- **`succeeded`**: 已成功完成
- **`failed`**: 执行失败

程序重启后，所有 `running` 状态自动重置为 `pending`。

## 指纹与可复用性规则

工作单元只有在以下条件**全部**满足时才会被复用：

1. **状态为 `succeeded`**
2. **输入指纹匹配** (`inputFingerprint`)
   - 基于输入文件的大小和修改时间
   - 确保输入数据未发生变化
3. **参数哈希匹配** (`paramHash`)
   - 基于任务选项的规范化 JSON
   - 确保转换/重建参数未改变
4. **工具版本匹配** (隐式检查)
   - 处理器版本记录在清单中
   - 转换器版本变化时可能需要重新执行

如果任何条件不满足，该工作单元将被重新执行。

## 崩溃窗口处理

系统设计考虑了以下崩溃窗口：

### 1. 转换/重建过程中崩溃

- 工作单元状态为 `running`
- 重启后自动重置为 `pending`
- 部分输出会被重新执行

### 2. 成果重命名后、清单写入前崩溃

- 恢复时验证输出目录
- 如果输出存在且有效，重新认领该成果
- 更新清单状态为 `succeeded`

### 3. 发布完成后、数据库注册前崩溃

- 已提交的成果不会被删除
- TempGuard 的 `mark_committed()` 确保晚到的取消不会移除输出

### 4. 取消操作的保留行为

- `off`: 取消时删除所有临时目录
- `retain-on-failure`: 取消时保留临时目录
- `resume`: 取消时保留临时目录和清单

## 使用示例

### 场景 1: 大数据集首次运行

```json
{
  "taskId": "large-dataset-001",
  "operation": "convert-osgb",
  "input": { "path": "/data/osgb-100gb" },
  "output": { "path": "/output/tiles" },
  "options": {
    "execution": {
      "cpuWorkers": 4,
      "resumePolicy": "resume"
    }
  }
}
```

- 任务失败后，重新执行相同配置
- 自动跳过已完成的 Block
- 只处理失败或未完成的部分

### 场景 2: 诊断失败原因

```json
{
  "execution": {
    "resumePolicy": "retain-on-failure"
  }
}
```

- 失败后保留中间结果
- 人工检查 `.geoforge-task-<id>/` 目录
- 查看清单了解哪些单元失败
- 检查部分输出和日志

### 场景 3: 参数调整后重新运行

修改 `texture.mode` 从 `keep` 到 `ktx2`:

```json
{
  "execution": {
    "resumePolicy": "resume"
  },
  "texture": {
    "mode": "ktx2"
  }
}
```

- 转换阶段的工作单元可以复用（参数未影响转换）
- 纹理阶段会重新执行（参数哈希不匹配）

## 限制与注意事项

1. **不支持跨机器恢复**
   - 清单中的路径是本地绝对路径
   - 文件系统依赖（锁、原子操作）限制可移植性

2. **网络文件系统**
   - 原子重命名和文件锁在 NFS 上可能不可靠
   - 建议使用本地磁盘作为临时工作目录

3. **工具版本变化**
   - 转换器升级后，建议手动清理旧清单
   - 当前未强制版本匹配检查

4. **并行执行限制**
   - 一个任务 ID 同时只能有一个进程执行
   - `prepare_temp` 使用排他性目录创建防止竞态

5. **磁盘空间**
   - `retain-on-failure` 和 `resume` 会占用额外磁盘空间
   - 需要手动清理不再需要的旧临时目录

## 最佳实践

1. **大数据集**：始终使用 `resumePolicy: "resume"`
2. **开发调试**：使用 `"off"` 确保干净状态
3. **生产环境**：考虑使用 `"retain-on-failure"` 以便故障排查
4. **定期清理**：删除不再需要的 `.geoforge-task-*` 目录
5. **监控空间**：关注临时目录的磁盘使用情况
