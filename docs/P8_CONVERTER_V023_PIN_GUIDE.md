# P8 Follow-up: Converter v0.2.3 Runtime Pin 更新指南

## 概述

本文档供**协调员**在converter v0.2.3 Release发布后更新主程序侧的运行包锁定配置。

## 前置条件

1. converter仓库 `Nicander93/geoforge-converter` 已发布v0.2.3 Release
2. Release包含Windows x64构建zip: `geoforge-converter-0.2.3-windows-x64.zip`
3. 已计算zip文件的SHA256哈希
4. 已记录Release tag对应的commit SHA

## 更新步骤

### 1. 更新converter-runtime.json

**文件**: `apps/desktop/config/converter-runtime.json`

**变更内容**:

```json
{
  "name": "geoforge-converter",
  "version": "0.2.3",
  "source": "https://github.com/Nicander93/geoforge-converter",
  "upstream": "https://github.com/fanvanzh/3dtiles",
  "upstreamCommit": "GeoForge engines/3dtiles-converter snapshot bdb8b7fd31c1eaa7ef65ad9f3419c6ab8c56c149",
  "windowsX64": {
    "url": "https://github.com/Nicander93/geoforge-converter/releases/download/v0.2.3/geoforge-converter-0.2.3-windows-x64.zip",
    "sha256": "TODO_REPLACE_WITH_ACTUAL_SHA256_64_HEX_LOWERCASE"
  }
}
```

**SHA256计算方法** (Windows PowerShell):

```powershell
$zipPath = "path\to\geoforge-converter-0.2.3-windows-x64.zip"
(Get-FileHash -Algorithm SHA256 -Path $zipPath).Hash.ToLowerInvariant()
```

**SHA256计算方法** (Linux/macOS):

```bash
sha256sum geoforge-converter-0.2.3-windows-x64.zip | awk '{print $1}'
```

### 2. 更新P8验收文档版本表

**文件**: `docs/P8_ACCEPTANCE.md`

**位置**: §2.3 双仓版本组合表

**待填写信息**:

| 字段 | 说明 | 示例 |
|------|------|------|
| converter commit | v0.2.3 tag指向的commit SHA (前7位) | `a1b2c3d` |
| 说明列 | 简述v0.2.3包含的关键改动 | "P2 block流式输出+进度事件" |

**修改示例**:

```markdown
| P8+v0.2.3 | (P8 PR commit) | **v0.2.3** | **a1b2c3d** | P2 block流式输出+进度事件 |
```

### 3. 验证更新

在本地测试环境验证新版本converter能正常下载和使用：

```powershell
# 1. 切换到P8分支
cd /path/to/3dtiles
git checkout cursor/p8-main-app-prep-d91e

# 2. 清理旧缓存（可选，测试下载）
Remove-Item -Recurse -Force .cache/converter/0.2.3 -ErrorAction SilentlyContinue

# 3. 运行准备脚本
apps/desktop/scripts/prepare-converter.ps1

# 4. 检查输出
# 应看到:
# - "Downloading https://github.com/.../v0.2.3/..."
# - "Cache hit: ..." (第二次运行时)
# - "SHA256 ..." (校验通过)
# - "Converter staged at dist\runtime\converter (version 0.2.3)"

# 5. 验证能力
dist/runtime/converter/_3dtile.exe --capabilities-json
# 应返回包含 modelConfigVersion=1, formats=["fbx","obj"] 的JSON
```

### 4. 提交变更

```bash
# 在P8分支上提交更新
git add apps/desktop/config/converter-runtime.json
git add docs/P8_ACCEPTANCE.md
git commit -m "P8: Update converter runtime pin to v0.2.3

- Converter v0.2.3 Release URL + SHA256
- Update P8_ACCEPTANCE.md version table with commit SHA
- Verification: prepare-converter.ps1 downloads and validates v0.2.3"

git push origin cursor/p8-main-app-prep-d91e
```

### 5. 更新PR描述

在PR #41 (假设) 的描述中追加:

```markdown
## Follow-up: v0.2.3 Pin Update

Updated `converter-runtime.json` to lock converter **v0.2.3** (commit `a1b2c3d`).

**Verified**:
- [x] SHA256 matches Release zip
- [x] `prepare-converter.ps1` downloads and validates
- [x] Converter capabilities include modelConfigVersion=1
- [x] OSGB small-sample test passes with v0.2.3

**Release Notes**: <https://github.com/Nicander93/geoforge-converter/releases/tag/v0.2.3>
```

## 检查清单

更新完成后，协调员应确认:

- [ ] `converter-runtime.json` version字段为 `"0.2.3"`
- [ ] windowsX64.url指向v0.2.3 Release zip
- [ ] windowsX64.sha256为64位小写hex字符串
- [ ] SHA256已在本地计算确认，与Release文件一致
- [ ] `docs/P8_ACCEPTANCE.md` §2.3表格已填写v0.2.3行
- [ ] 本地运行 `prepare-converter.ps1` 无错误
- [ ] `_3dtile.exe --capabilities-json` 返回有效JSON
- [ ] 小样本OSGB转换测试通过

## 回退预案

如果v0.2.3存在严重问题，可临时回退到v0.2.2:

```json
{
  "version": "0.2.2",
  "windowsX64": {
    "url": "https://github.com/Nicander93/geoforge-converter/releases/download/v0.2.2/geoforge-converter-0.2.2-windows-x64.zip",
    "sha256": "228c75a09d07c6c6c5c76ac1b57812206bd229656fae5907a4716545fe7a797d"
  }
}
```

回退后需在PR中说明原因并重新测试。

## 附录：v0.2.2 → v0.2.3 预期变化

根据计划文档P2，v0.2.3可能包含:

- [ ] Block级流式输出 (osgb.rs channel接收方不阻塞)
- [ ] Block完成后立即写manifest
- [ ] 进度事件通过stdout JSONL发送 (可选)
- [ ] 线程安全修正 (osgb23dtile.cpp static bool)

**注意**: 如果v0.2.3尚未实现P2，主程序P8进度事件仍正常工作 (显示并行度，总数为0)。converter侧进度可在后续版本补充。

---

**文档版本**: P8.1 (初始版本，等待v0.2.3 Release)  
**维护者**: 协调员  
**最后更新**: 2026-09-27 (P8 PR提交时)
