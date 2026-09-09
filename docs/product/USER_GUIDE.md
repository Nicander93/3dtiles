# GeoForge 3D 用户指南（V1）

日期：2026-09-09（Asia/Shanghai）。P0–P7 已闭环。推送候选见 `docs/product/PUSH_CANDIDATES.md`（本批不 push）。

## 启动

```bash
bash scripts/run_geoforge.sh
# UI http://127.0.0.1:8787/
# 健康检查：curl -s http://127.0.0.1:8787/api/health
```

可选 Qt 壳（浏览器回退，非完整 WebEngine）：

```bash
# 先起 API/UI，再：
bash scripts/run_geoforge_shell.sh
# DISPLAY 常用 :2；二进制 apps/geoforge_shell/build/geoforge_shell
```

## 路径与环境变量

`scripts/run_geoforge.sh` 以仓库根目录为基准；下列可用环境变量覆盖（默认仍兼容本机 `/workspace/runtime`）：

| 变量 | 作用 | 默认探测顺序 |
|------|------|----------------|
| `GEOFORGE_RUNTIME` | 3dtile 运行时根 | `/workspace/runtime` → `$ROOT/.runtime` |
| `GEOFORGE_3DTILE` | 转换入口 `run.sh` | `$RUNTIME/3dtile-bin-ktx2/run.sh` → `…/3dtile-bin/run.sh` |
| `GEOFORGE_VENV` | Python venv | `/workspace/venv-3dtiles` → `$ROOT/.venv` |
| `GEOFORGE_SAMPLE` | 样例 OSGB 提示路径 | `/workspace/data/OSGBny/OSGBny` |
| `GEOFORGE_PREVIEW_CACHE` | 预览缓存 | `$ROOT/.geoforge/preview_cache` |
| `MAMBA_ROOT_PREFIX` | Qt 壳 / OSGB 查看器 conda | `/workspace/conda` → `$HOME/conda` |

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

- 3D Tiles：成果页或 `/preview/tiles`（Cesium；本地/近原点 tileset 自动 ENU `modelMatrix`，可 `?enu=lat,lon[,h]`）  
- 可选原生 OSGB：`/preview/osgb` → Qt `osgb_viewer`（需 DISPLAY，常 :2）  
- Qt 产品壳：`run_geoforge_shell.sh`（浏览器回退 + 内嵌 OSGB；截图 `geoforge_shell_shot.png` / `geoforge_shell_osgb.png`）

## 取消任务

排队中立即取消；运行中先 terminate，宽限期后强制结束。

## 已知限制 / 非声称

- **不做** Windows 安装包  
- **不做** 完整/已链接 Qt WebEngine 壳（现为浏览器回退）  
- `_3dtile` 二进制内原生 `--enable-texture-compress` 仍缺；KTX2 靠 wrapper + basisu 后处理  
- 首版不做：裁剪/压平/工程保存、多源融合、分布式  

## Related docs

- `docs/product/ACCEPTANCE.md`
- `docs/product/V1_DELIVERY_PLAN.md`
- `docs/product/V1_STATUS.md`
- `docs/product/PUSH_CANDIDATES.md`（日后 push 清单）
- `docs/product/QT_SHELL.md` · `docs/REBUILD_TOP.md`
- `apps/osgb_viewer/README.md` · `apps/geoforge_shell/README.md`

## English (short)

Start with `scripts/run_geoforge.sh` → http://127.0.0.1:8787/. Convert: scan → rebuild levels 1|2 + keep/KTX2 (basisu post-process) → submit. Cesium ENU framing works. Qt shell via `run_geoforge_shell.sh` is browser-fallback only. No Windows package; no linked WebEngine. See `PUSH_CANDIDATES.md` for a later push. P0–P7 done.
