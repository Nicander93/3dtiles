# P7 验证指南

## 前置条件

- 主程序开发环境已就绪
- 可访问桌面应用 UI
- 有 OSGB 测试数据（建议 4×4 或更大 fixture）

## 验证步骤

### 1. Settings 页面资源配置

#### 1.1 打开设置页面

**操作**
```
启动桌面应用 → 导航到 Settings
```

**预期**
- 看到新的"资源配置"区块（在默认参数区块下方）
- 默认显示"资源模式: 自动"

#### 1.2 切换到自定义模式

**操作**
```
资源模式下拉选择 → 选择"自定义"
```

**预期**
- 下方展开 3 个输入框:
  - CPU 并发数 (1-64)
  - 内存预算 MiB (512-524288)
  - I/O 并发数 (1-16)
- 每个字段显示占位符"自动"
- 有对应的提示文字

#### 1.3 输入自定义值并保存

**操作**
```
CPU 并发数: 4
内存预算: 8192
I/O 并发数: 2
点击"保存"按钮
```

**预期**
- 显示"已保存"提示（2 秒后消失）
- 刷新页面后值仍保留

#### 1.4 切换回自动模式

**操作**
```
资源模式下拉选择 → 选择"自动"
点击"保存"
```

**预期**
- 自定义字段输入框隐藏
- 显示提示"系统自动分配 CPU、内存和 I/O 资源"

### 2. 任务提交携带 execution 配置

#### 2.1 自定义模式提交任务

**操作**
```
1. Settings 设置资源模式=自定义, CPU=4, 内存=8192, I/O=2
2. 导航到 OSGB Convert
3. 填写输入/输出目录
4. 提交任务
5. 查看 task_store 数据库或日志中的 options JSON
```

**预期**
- task options 包含:
```json
{
  "convert": {"threads": 4},
  "execution": {
    "cpuWorkers": 4,
    "memoryBudgetMiB": 8192,
    "ioWorkers": 2
  }
}
```

#### 2.2 自动模式提交任务

**操作**
```
1. Settings 设置资源模式=自动
2. 提交 OSGB 转换任务
3. 查看 task options JSON
```

**预期**
- task options **不包含** execution 字段
- 或 execution 为空对象 {}

### 3. Processing 页面进度显示

#### 3.1 查看运行中任务进度

**操作**
```
1. 启动一个 OSGB 转换任务
2. 导航到 Processing (任务列表)
3. 点击正在运行的任务查看详情
```

**预期 - 任务列表行**
- 状态单元格显示: "convert · N/M" 或 "convert · 已完成 N"
- 若已知 total: 显示 "120/500"
- 若 total 未知: 显示 "已完成 120"

**预期 - 详情面板**
- "处理阶段"下方新增进度详情面板（浅色背景）
- 显示:
  - "进度: 120 / 500" 或 "120 个单元"
  - "并行数: 4 worker(s)" (如果 processor 发送了 parallelism)
  - "状态: 🔜 等待资源" (如果 resourceWait=true)

#### 3.2 验证进度更新限频

**操作**
```
观察运行中任务详情面板，持续刷新
```

**预期**
- 进度数字更新频率约 2-5 次/秒
- 不会出现每秒数十次刷新
- UI 保持流畅

#### 3.3 查看失败任务恢复入口

**操作**
```
1. 故意中断一个已处理部分 Block 的任务 (Ctrl+C 或取消)
2. 任务状态变为 cancelled/failed
3. 点击该任务查看详情
```

**预期**
- 如果 progress.completed > 0:
  - 显示"恢复处理"按钮（带刷新图标 ↻）
  - 按钮提示: "从断点恢复处理"
- 点击按钮跳转到 OSGB Convert 表单，输入/输出预填

#### 3.4 已完成任务不显示恢复按钮

**操作**
```
查看 status=succeeded 的任务详情
```

**预期**
- 只显示"重新处理"按钮
- 不显示"恢复处理"按钮（恢复仅用于中断任务）

### 4. 向后兼容验证

#### 4.1 打开旧任务

**操作**
```
查看数据库中 P7 之前创建的任务 (progress 为 number 或空 object)
```

**预期**
- 任务列表正常显示
- 详情面板不报错
- progress 为 number 时显示百分比
- progress 无 completed 字段时不显示进度详情面板

#### 4.2 published 任务不显示重跑

**操作**
```
查看已发布成功的任务 (status=completed, artifactId 存在)
```

**预期**
- 显示"查看成果"按钮
- 显示"重新处理"按钮
- 不显示为"运行中"或"排队"状态

### 5. 日志内存有界验证

#### 5.1 长时间运行任务

**操作**
```
让一个大任务运行数小时，或手动向 task_store 插入超长日志
```

**预期**
- task_store.log_text 不超过 200,000 字节 (task_store.rs L282)
- 旧日志自动截断，保留尾部
- UI 不因日志过长卡顿

## 边界情况

### E1. 自定义资源超限
```
输入 CPU=999, 内存=9999999
```
**预期**: HTML5 input 限制生效 (max=64, max=524288)

### E2. 空 progress 对象
```
task.progress = {}
```
**预期**: UI 不崩溃，不显示进度详情面板

### E3. 只有 completed 无 total
```
task.progress = { completed: 50 }
```
**预期**: 显示"已完成 50 个单元"

### E4. parallelism=0
```
task.progress = { completed: 10, total: 100, parallelism: 0 }
```
**预期**: 显示"并行数: 0 worker(s)" (资源等待或队列满)

## 自动化验证（需环境修复）

### TypeScript 类型检查
```bash
cd apps/desktop
npx tsc --noEmit
```

### Rust 编译检查
```bash
cd crates/processor
cargo check --lib --tests
```

### 单元测试
```bash
cd crates/processor
cargo test progress_throttle
```

## 验收结果模板

```markdown
## P7 验收结果

**日期**: YYYY-MM-DD  
**执行人**: [Name]  
**环境**: Windows 11 / macOS Sonoma / Ubuntu 22.04  

### 测试结果

| 测试项 | 状态 | 备注 |
|--------|------|------|
| Settings 自动/自定义切换 | ✅ / ❌ | |
| 自定义值保存并持久化 | ✅ / ❌ | |
| 自定义模式提交任务包含 execution | ✅ / ❌ | |
| 自动模式不包含 execution | ✅ / ❌ | |
| Processing 显示 N/M 进度 | ✅ / ❌ | |
| 未知 total 显示"已完成 N" | ✅ / ❌ | |
| 显示并行数 parallelism | ⚠️ / ❌ | processor 未发送 |
| 资源等待标识 resourceWait | ⚠️ / ❌ | processor 未发送 |
| 失败任务显示恢复按钮 | ✅ / ❌ | |
| 恢复按钮跳转正确 | ✅ / ❌ | |
| 旧任务向后兼容 | ✅ / ❌ | |
| 日志截断机制生效 | ✅ / ❌ | |

### 截图
- Settings 资源配置区块: [附图]
- Processing 进度详情面板: [附图]
- 恢复处理按钮: [附图]

### 问题与建议
[列举发现的问题]

### 结论
- [ ] 通过，可合并到 master
- [ ] 有问题，需修正
```

## 故障排除

### Q1: Settings 保存后刷新丢失
**检查**: localStorage 是否被清空，api.getSettings() 是否返回正确值

### Q2: Processing 不显示进度详情
**检查**: task.progress 类型是否为 object, completed 字段是否存在

### Q3: 恢复按钮不显示
**检查**: task.status 是否为 failed/cancelled/interrupted, progress.completed 是否 > 0

### Q4: TypeScript 编译报错
**检查**: TaskProgressDetail 类型定义是否在 types.ts 中

### Q5: Rust 编译报错 edition2024
**解决**: 升级 Rust toolchain 或降级 clap 版本

---

**验证负责人签字**: _________________  
**日期**: _________________
