# GeoForge 3D — V1 对照交付计划

依据：`01-product-definition.md`、`02-v1-implementation.md` §2 功能边界、合集图 `mockups/`。  
原则：按阶段闭环可验证能力；少打断用户；最终可本地启动、真实数据可用。

## 设计必须项 → 阶段映射

| 设计必须项 | 阶段 | 完成标准（可验收） |
| --- | --- | --- |
| OSGB 输入扫描与错误报告 | P0 | 公开样例扫描 ok；坏路径有明确原因 |
| OSGB→3D Tiles 转换 | P0 | 产出可被 Cesium 加载的 tileset |
| 顶层重建（真实概览几何） | P0 | Merge_L* + REPLACE 父节点，根 children 减少 |
| 成果登记 / 打开 / 预览入口 | P0→P2 | SQLite artifacts；可预览、可继续处理 |
| 产品壳 UI（对齐合集图） | P1 | React 为主壳；工作区/三步转换/任务中心 |
| OSGB 原生预览 + 导出入口 | P2 | 一键启动 osgb_viewer；导出跳转转换 |
| 3D Tiles 预览 | P2 | 本地成果 tileset 在 Cesium 中打开 |
| 任务排队/阶段/取消/日志/历史/重跑 | P3 | 阶段事件准确；取消可用；历史可重跑 |
| 独立 process-tileset 路径 | P3 | 已有 Tiles 可只做重建/纹理 |
| 纹理 keep / KTX2 | P4 | keep 默认；KTX2 可选且真实生效或诚实失败 |
| CRS / 原点 / 高程提示 | P5 | 扫描展示；缺参阻止地理导出有说明 |
| 一键启动与验收说明 | P6 | run 脚本 + ACCEPTANCE 勾选表 |

首版不做（明确排除）：裁剪/压平/工程保存、多源融合、分布式、Windows 安装包（可后置）、完整 Qt WebEngine 壳（P6 尽力，不阻塞交付）。

## 阶段状态

- [x] **P0** 基础管线验收固化（本批冒烟通过）
- [x] **P1** React 壳 + 合集图对齐（核心页已对齐）
- [x] **P2** 双预览联通
- [x] **P3** 任务与双路径完备
- [x] **P4** 纹理压缩路径（keep 默认；KTX2 ETC1S/UASTC 经 basisu 后处理，KHR_texture_basisu 实证）
- [x] **P5** 坐标与定位 UX
- [x] **P6** 可交付打包与验收清单
- [x] **P7** hardening（E2E KTX2+rebuild、Cesium 加载、壳/文档）

## 当前工作焦点

**P0–P7 done（文档同步）**：KTX2 basisu 后处理、Cesium ENU framing、rebuild levels 1|2 UI、geoforge_shell 浏览器回退、`run_geoforge.sh` / `run_geoforge_shell.sh`。非声称不变（无 Windows、无已链接 WebEngine）。日后推送见 `PUSH_CANDIDATES.md`（本批不 push）。

## 运行入口

```bash
bash /workspace/repos/3dtiles/scripts/run_geoforge.sh
# UI http://127.0.0.1:8787/
```

样例数据：`/workspace/data/OSGBny/OSGBny`

## 执行笔记（2026-09-09 P1+P2）

- Backend artifacts/preview/native/prepare/SPA
- Web build + mockup pages
- Cesium ?tileset=
- Smoke convert + native pid
- Blocker: DISPLAY for native; P3-P5 remaining


## 执行笔记（2026-09-09 P3）

- Stages: convert-osgb emits scan→convert→rebuild→texture(keep=skipped)→check→done; UI stepper derives from stage/options
- Cancel: queued immediate; running SIGTERM grace then SIGKILL → cancelled
- Logs: task.log + `.geoforge/logs/{id}.log`; GET `/api/tasks/{id}/logs`
- History: list + 重新执行 clones POST /api/tasks
- process-tileset Web UI `/tiles/process` + Results「继续处理」; E2E on osgbny_v1 with rebuildTop
- KTX2 still honest deferred (P4)

## 执行笔记（2026-09-09 P4）

- keep wired and green
- KTX2 honest fail on current runtime binary
- Binary rebuild owned by another worker (do not touch 3dtile-bin here)

## 执行笔记（2026-09-09 P5）

- OsgbConvert shows effective CRS/origin/unit hint
- geo options + geographicExport pre-submit/API gate MISSING_CRS
- ENU -> 3dtile -c x/y; Z -> offset when overridden
- Settings vertical-datum note

## 执行笔记（2026-09-09 P6）

- ACCEPTANCE.md + USER_GUIDE.md
- run_geoforge.sh prints URLs / UI dist warning
- Workspace ProcessTiles link + badge counts
- Honest: no Windows package; no full Qt WebEngine embed yet; `apps/geoforge_shell` browser-fallback + OSGB embed; osgb_viewer optional


## 执行笔记（2026-09-09 UI polish + Qt shell）

- Web：合集图对齐（侧栏分组、三栏校验卡、任务卡+横向阶段条、预览工具栏/信息面板、工作区入口卡）
- Qt：`apps/geoforge_shell` 可构建；无 WebEngine 时浏览器回退 + 内嵌 OSGB
- 文档：QT_SHELL.md；截图 geoforge_shell_shot.png
- 仍不争用 3dtile-bin / KTX2

## 执行笔记（2026-09-09 P7a）

- P4 closed: KTX2 via basisu post-process (`texture_ktx2.py` / wrapper), not rebuilt `_3dtile`
- Focus → P7 hardening (E2E rebuild+ktx2, process-tileset ktx2-only, docs/ACCEPTANCE sync)
- E2E convert-osgb **task-3917f396ea60** succeeded: OSGBny → `osgbny_p7_e2e_rebuild`, rebuildTop levels=1, texture.mode=ktx2-etc1s; Merge_L* + 78× KHR_texture_basisu; art-c704b957a939; tileset HTTP 200; Cesium preview accepts `?tileset=`
- process-tileset ktx2-only **task-5f1d943144ed**: `osgbny_p4_keep` → `osgbny_p7_process_ktx2`


## 执行笔记（2026-09-09 P7 complete）

- **P7a E2E:** task-3917f396ea60 → `osgbny_p7_e2e_rebuild` / art-c704b957a939；rebuildTop L1 + ktx2-etc1s；78× `KHR_texture_basisu`
- **Cesium KTX2 load:** API preview-url `/artifacts/art-c704b957a939/tileset.json`；
  `cesium-preview.html?tileset=...`；headless WebGL OK；`Cesium3DTileset.fromUrl` → tilesLoaded；
  screenshot `docs/product/p7_cesium_ktx2.png`
- **P7b:** geoforge_shell WebEngine link blocked (glibc/udev)；browser-fallback shell delivered
- **P7c:** rebuild_top geometricError flooring；docs/REBUILD_TOP.md
- **No git push** this batch
