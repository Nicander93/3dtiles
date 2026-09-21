# R00 基线重现命令

本文档记录如何重现 R00 基线的创建过程。

## 前提条件

- Git 已配置并可访问 https://github.com/Nicander93/3dtiles
- 具有 push 权限（如果需要推送分支）
- 环境已安装：Rust, Cargo, Node.js

## 步骤 1: 获取最新状态

```bash
cd /path/to/3dtiles
git fetch origin master feat/v1-prod-align
```

## 步骤 2: 记录 SHA 和关系

```bash
# 获取 master 当前 tip
git rev-parse origin/master

# 获取 feat/v1-prod-align tip
git rev-parse origin/feat/v1-prod-align

# 获取共同祖先
git merge-base origin/master origin/feat/v1-prod-align

# 统计分支差异
git rev-list --left-right --count origin/master...origin/feat/v1-prod-align
```

**R00 基线记录的值:**
- master: `8c84bb2f1d3c8220805b19ec9e6e4a1dec9b010e`
- feat/v1-prod-align: `81f3494b4002c6fb97064e8c531c58ec1f92a7dc`
- 共同祖先: `671145d9cf86ab28a3ce538aacc3224162a5bd2b`
- 关系: master 领先 9 个提交，feat/v1-prod-align 领先 10 个提交

## 步骤 3: 创建工作分支

```bash
# 从 master 创建新分支（如果不存在）
git checkout -b feat/v1-convergence origin/master

# 或者，如果分支已存在，重置到 master
# git checkout feat/v1-convergence
# git reset --hard origin/master
```

## 步骤 4: 收集环境信息

```bash
# Rust 版本
rustc --version
cargo --version

# Node.js 版本
node --version

# 操作系统
uname -sr  # Linux/macOS
# 或 Windows: systeminfo | findstr /B /C:"OS Name" /C:"OS Version"
```

**R00 记录的版本:**
- rustc: 1.83.0 (90b35a623 2024-11-26)
- cargo: 1.83.0 (5ffbef321 2024-10-29)
- node: v22.14.0
- OS: Linux 6.12.94+

## 步骤 5: 分析分支差异

```bash
# 查看 feat/v1-prod-align 独有的提交
git log --oneline --no-decorate origin/feat/v1-prod-align ^origin/master

# 查看 master 独有的提交
git log --oneline --no-decorate origin/master ^origin/feat/v1-prod-align

# 文件变更统计
git diff --stat origin/master...origin/feat/v1-prod-align
```

## 步骤 6: 创建基线文档

```bash
# 创建文档目录
mkdir -p docs/acceptance/v1-convergence

# 生成文档（手动或使用脚本）
# - baseline.json
# - branch-diff.md
# - status.md
# - rerun-commands.md (本文件)
```

## 步骤 7: 更新 .gitignore

确保 `.gitignore` 包含验收结果目录：

```gitignore
# 验收测试输出（bulky results 不提交）
acceptance-results/
```

## 步骤 8: 提交并推送

```bash
# 添加文件
git add docs/acceptance/v1-convergence/
git add .gitignore  # 如果有修改

# 提交
git commit -m "docs(R00): establish V1 convergence baseline

- Record git state: master@8c84bb2, feat/v1-prod-align@81f3494
- Document branch diff: 9 ahead / 10 ahead
- Create status table for R00-R12 tasks
- Define acceptance output conventions"

# 推送到远程
git push -u origin feat/v1-convergence
```

## 步骤 9: 创建 PR

使用 GitHub CLI 或 Web UI 创建 PR：

```bash
gh pr create \
  --base master \
  --head feat/v1-convergence \
  --title "docs(R00): GeoForge V1 convergence baseline" \
  --body "Establish R00 baseline for V1 convergence plan (R00-R12).

## Changes
- Git state snapshot: master vs feat/v1-prod-align
- Environment inventory (CI + pending local fields)
- Branch diff analysis (9/10 commits, 75 files)
- Task status table for R00-R12

## Next Steps
- R01-R02: converter settings persistence fix
- R03+: progressive merge of feat/v1-prod-align fixes

See \`docs/acceptance/v1-convergence/\` for details."
```

## 验收输出位置

按照约定，bulky 验收输出应放在 **gitignored** 目录中：

```
acceptance-results/
└── v1-convergence/
    └── <run-id>/
        ├── converter-test-output/
        ├── processor-logs/
        ├── cesium-screenshots/
        └── validation-reports/
```

只有 lean 的报告、脚本和 fixture 提交到 `docs/acceptance/v1-convergence/`。

## Windows 特定字段采集

R00 baseline.json 中标记为 `pending-local` 的字段需要在 Windows 实机上运行：

```powershell
# OS 版本
systeminfo | findstr /B /C:"OS Name" /C:"OS Version"

# WebView2 运行时
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv

# CPU
wmic cpu get name

# RAM
wmic computersystem get totalphysicalmemory

# 磁盘
wmic logicaldisk get size,freespace,caption

# 二进制哈希（在构建后）
Get-FileHash apps\desktop\src-tauri\resources\runtime\converter.exe -Algorithm SHA256
Get-FileHash apps\desktop\src-tauri\resources\runtime\processor.exe -Algorithm SHA256
Get-FileHash apps\desktop\src-tauri\resources\runtime\top_rebuild.exe -Algorithm SHA256
Get-FileHash apps\desktop\src-tauri\resources\runtime\basisu.exe -Algorithm SHA256
```

这些字段在 R12 任务中完善。

## 时区说明

所有时间戳使用 Asia/Shanghai (CST, UTC+8)：

```bash
TZ='Asia/Shanghai' date '+%Y-%m-%d %H:%M:%S %Z'
```

R00 创建时间: 2026-09-21 17:06:49 CST
