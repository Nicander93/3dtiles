# P3 验证指南

## 快速验证清单

在合并 PR #36 之前，建议完成以下验证：

### 1. 编译检查

```bash
cd /path/to/3dtiles
git checkout cursor/p3-work-unit-resume-bb13
cargo build --release
```

预期：编译成功，无错误和警告。

### 2. 单元测试

```bash
# 测试工作清单模块
cargo test -p geoforge-processor work_manifest

# 测试 commit 模块（包含 TempGuard）
cargo test -p geoforge-processor commit

# 测试 pipeline 模块（包含恢复逻辑）
cargo test -p geoforge-processor pipeline

# 测试协议解析
cargo test -p geoforge-protocol execution_options
```

预期：所有测试通过。

### 3. 协议兼容性

```bash
# 验证旧任务配置仍可解析（向后兼容）
cargo run --bin geoforge-processor -- --help
```

检查点：
- ExecutionOptions 解析旧的 `convert.threads` 配置
- 默认 resumePolicy 为 `off`
- 无 execution 配置时使用默认值

### 4. 功能测试（可选）

如果有 OSGB 测试数据：

```bash
# 创建测试任务配置
cat > test-resume.json <<EOF
{
  "taskId": "test-resume-001",
  "operation": "convert-osgb",
  "input": { "path": "/path/to/small-osgb" },
  "output": { "path": "/tmp/test-output" },
  "options": {
    "execution": {
      "cpuWorkers": 2,
      "resumePolicy": "resume"
    }
  }
}
EOF

# 首次执行（预期成功）
geoforge-processor test-resume.json

# 检查清单文件
ls -la /tmp/.geoforge-task-test-resume-001/.geoforge-work-manifest.json
cat /tmp/.geoforge-task-test-resume-001/.geoforge-work-manifest.json | jq .

# 删除输出，保留临时目录
rm -rf /tmp/test-output

# 再次执行（预期恢复，跳过已完成单元）
geoforge-processor test-resume.json
```

检查点：
- 第二次执行日志显示 "[resume] loaded manifest: X succeeded, Y pending/failed"
- 已完成的 Block 未重新转换
- 最终输出正确

### 5. 策略验证

#### 5.1 ResumePolicy: off (默认)

```bash
# 第一次执行
geoforge-processor task-off.json

# 不删除临时目录，再次执行
geoforge-processor task-off.json
```

预期：第二次执行拒绝，错误信息包含 "already exists" 和 "resumePolicy"。

#### 5.2 ResumePolicy: retain-on-failure

```bash
cat > task-retain.json <<EOF
{
  "taskId": "test-retain-001",
  "operation": "convert-osgb",
  "input": { "path": "/invalid/path" },
  "output": { "path": "/tmp/test-retain" },
  "options": {
    "execution": { "resumePolicy": "retain-on-failure" }
  }
}
EOF

geoforge-processor task-retain.json
```

预期：失败后临时目录 `/tmp/.geoforge-task-test-retain-001/` 保留。

#### 5.3 ResumePolicy: resume

```bash
# 中断第一次执行 (Ctrl+C)
geoforge-processor task-resume.json
# 中途 Ctrl+C

# 再次执行
geoforge-processor task-resume.json
```

预期：
- 加载清单
- Running 状态重置为 Pending
- 已完成的单元不重新执行

### 6. 指纹验证

修改输入或参数后恢复：

```bash
# 首次执行
geoforge-processor task.json

# 修改输入文件（触碰修改时间）
touch /path/to/input/Data/Tile_001/Tile_001.osgb

# 再次执行
geoforge-processor task.json
```

预期：inputFingerprint 不匹配，Tile_001 重新执行。

```bash
# 修改参数
sed -i 's/"cpuWorkers": 2/"cpuWorkers": 4/' task.json

# 再次执行
geoforge-processor task.json
```

预期：paramHash 不匹配，受影响单元重新执行。

## 回归测试

确保已有功能未受影响：

```bash
# 运行完整测试套件
cargo test

# 特别关注
cargo test -p geoforge-processor scan
cargo test -p geoforge-processor convert
cargo test -p geoforge-processor rebuild
cargo test -p geoforge-processor texture
cargo test -p geoforge-processor validate
```

预期：所有既有测试仍然通过。

## 性能基准（可选）

如果有 P0 基准脚本：

```bash
cd scripts
./benchmark-large-dataset.sh --with-resume

# 对比
# - 首次执行时间
# - 恢复执行时间（应显著减少）
# - 内存峰值（无明显增长）
```

## 已知限制

由于当前 PR 环境的 Cargo 依赖索引问题，未能在提交时运行测试。建议：

1. 在本地干净环境执行上述验证
2. 或等待 CI 构建完成
3. 检查 CI 测试结果

## 问题排查

### 编译失败

```bash
# 清理重建
cargo clean
cargo build
```

### 测试失败

检查：
- 工作清单序列化/反序列化
- 指纹计算稳定性
- 临时目录权限

### 恢复不生效

检查：
- resumePolicy 配置是否正确
- 清单文件是否存在和有效
- 临时目录 ownership marker 是否匹配

## 合并前确认

- [ ] 编译成功
- [ ] 所有单元测试通过
- [ ] 至少一次成功的恢复场景测试
- [ ] 文档完整（work-unit-resume.md）
- [ ] 向后兼容（旧配置仍可用）
- [ ] 无性能退化
