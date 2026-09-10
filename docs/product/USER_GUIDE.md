> **V1 path update (Phase 4):** Preferred desktop is **Tauri 2** (`apps/desktop`) + React + Cesium + Rust `processor`.  
> Qt shell / OSGB native preview **deleted** (historical stubs: [`QT_SHELL.md`](./QT_SHELL.md), [`OSGB_NATIVE_PREVIEW.md`](./OSGB_NATIVE_PREVIEW.md)).  
> Legacy Python API moved to `tools/experiments/desktop_server_py` (`bash scripts/run_geoforge.sh --legacy-server`).  
> Rebuild Python baseline: `tools/experiments/rebuild_top_py` (not release runtime).  
> Authority: [`03-v1-architecture-rebuild-plan.md`](./03-v1-architecture-rebuild-plan.md). Reports: [`PHASE_REPORTS/`](./PHASE_REPORTS/).

---
# GeoForge 3D 用户指南（V1）

日期：2026-09-10（Asia/Shanghai）。架构重构 Phase 0–10 已推进（TopRebuild Rust 正式路径；规模验证至合成 16×16）。**不宣称**大范围/百平方公里。详见 `PHASE_REPORTS/SUMMARY.md`。推送候选见 `docs/product/PUSH_CANDIDATES.md`（本批不 push）。

## 启动

### 推荐（Phase 3+）：Tauri 桌面 + Processor

```bash
# 从仓库根目录
cargo build -p processor
cd apps/desktop && npm install && npm run tauri:dev
# 需 DISPLAY + WebKitGTK；编译检查见 apps/desktop/README.md
# 或：bash scripts/run_geoforge.sh   # 打印用法；有 DISPLAY 且已装依赖时可直接拉起 tauri:dev
```

### 浏览器 / Vite（无 Tauri 窗口）

```bash
cd apps/desktop && npm run build && npm run preview
# 或 npm run dev → http://127.0.0.1:5173
```

### 遗留：Python HTTP API（参考实现）

```bash
bash scripts/run_geoforge.sh --legacy-server
# UI http://127.0.0.1:8787/  （需先 npm run build 产出 apps/desktop/dist）
```

### 历史 / 已删除：Qt 壳与 OSGB Viewer

Phase 4 已删除 `apps/geoforge_shell/`、`apps/osgb_viewer/` 及对应 scripts。说明见 `docs/product/historical/`。

## 路径与环境变量

`scripts/run_geoforge.sh` 以仓库根目录为基准；下列可用环境变量覆盖（默认仍兼容本机 `/workspace/runtime`）：

| 变量 | 作用 | 默认探测顺序 |
|------|------|----------------|
| `GEOFORGE_RUNTIME` | 3dtile 运行时根 | `/workspace/runtime` → `$ROOT/.runtime` |
| `GEOFORGE_3DTILE` | 转换入口 `run.sh` | `$RUNTIME/3dtile-bin-ktx2/run.sh` → `…/3dtile-bin/run.sh` |
| `GEOFORGE_VENV` | Python venv | `/workspace/venv-3dtiles` → `$ROOT/.venv` |
| `GEOFORGE_SAMPLE` | 样例 OSGB 提示路径 | `/workspace/data/OSGBny/OSGBny` |
| `GEOFORGE_PREVIEW_CACHE` | 预览缓存 | `$ROOT/.geoforge/preview_cache` |
| `GEOFORGE_REBUILD_TOP` | Python rebuild baseline | `tools/experiments/rebuild_top_py/rebuild_top.py` |

本地完整步骤见 `docs/product/LOCAL_VERIFY.md`。

## 样例与输出

- OSGB：默认提示 `/workspace/data/OSGBny/OSGBny`（SRS ENU + SRSOrigin）；可用 `GEOFORGE_SAMPLE` 覆盖
- 输出根：常见为 `/workspace/data/geoforge_outputs/`（本地自定即可）
- P7 E2E（rebuild + KTX2）：`osgbny_p7_e2e_rebuild`（task-3917f396ea60 / art-c704b957a939）

## 转换三步

1. **输入 + 扫描**：查看实际采用的 CRS / 原点 / 单位提示  
2. **处理选项**  
   - 顶层重建：勾选后选 **levels 1 或 2**（默认 1）  
   - 纹理：`keep`（默认）或 **ktx2-etc1s / ktx2 / ktx2-uastc**（basisu 后处理 → `KHR_texture_basisu`）  
   - 可选 CRS/原点覆盖；勾选地理导出且扫描无 SRS、又无覆盖时，提交被阻止  
3. **输出路径 + 提交**；在「正在处理」跟踪阶段 / 取消

同样选项也在 **Tiles 继续处理**（`/tiles/process`）与设置页默认层数。

ENU lat/lon → 3dtile `-c` x/y；原点 Z 可映射 offset。高程基准独立，V1 不自动猜测。

## 试用 KTX2 与 levels

1. 启动 `run_geoforge.sh`，打开转换页，扫描 OSGBny。  
2. 处理步：勾选顶层重建，选 **levels=1**（或 2）；纹理选 **ktx2-etc1s**（能力接口显示 `postprocessBasisu` 可用时选项可点）。  
3. 提交后等 `succeeded`；成果页打开 Cesium 预览（ENU 相机/framing 已接好）。  
4. 或对已有 keep 成果走 process-tileset，只做 KTX2 / 重建。

实证参考：task-3917f396ea60（rebuild L1 + ktx2）；截图 `docs/product/p7_cesium_ktx2_v2.png`。

## 预览

- **3D Tiles（V1）**：成果页或 `/preview/tiles`（Cesium；本地/近原点 tileset 自动 ENU `modelMatrix`，可 `?enu=lat,lon[,h]`）  
- **OSGB Preview 已从产品 UI 移除**（Phase 1）。历史 Qt `osgb_viewer` / `/preview/osgb` 不再作为产品入口。  
- Qt 产品壳：`(deleted) run_geoforge_shell.sh` 仅为历史路径（已取消于 V1）。

## 取消任务

排队中立即取消；运行中先 terminate，宽限期后强制结束。

## 已知限制 / 非声称

- **不做** Windows 安装包  
- **不做** Qt 壳 / OSGB 原生预览（Phase 4 已删除产品路径）  
- `_3dtile` 二进制内原生 `--enable-texture-compress` 仍缺；KTX2 靠 wrapper + basisu 后处理  
- 首版不做：裁剪/压平/工程保存、多源融合、分布式  

## Related docs

- `docs/product/ACCEPTANCE.md`
- `docs/product/V1_DELIVERY_PLAN.md`
- `docs/product/V1_STATUS.md`
- `docs/product/PUSH_CANDIDATES.md`（日后 push 清单）
- `docs/product/historical/QT_SHELL.md` · `docs/product/historical/OSGB_NATIVE_PREVIEW.md`
- `docs/REBUILD_TOP.md` · `tools/experiments/rebuild_top_py/README.md`
- `apps/desktop/README.md` · `tools/experiments/desktop_server_py/README.md`

## English (short)

Preferred: `cd apps/desktop && npm run tauri:dev` with `cargo build -p processor`. Legacy API: `bash scripts/run_geoforge.sh --legacy-server`. Convert: scan → rebuild levels 1|2 + keep/KTX2 → Cesium preview. Qt / OSGB viewer removed in Phase 4. See `PUSH_CANDIDATES.md`. No claim of large-area top-rebuild production support yet.
