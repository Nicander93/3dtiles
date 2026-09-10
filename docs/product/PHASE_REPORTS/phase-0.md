> **Path note (Phase 4):** `apps/web` → `apps/desktop`; `tools/rebuild_top` → `tools/experiments/rebuild_top_py`; Qt apps deleted; `desktop_server` → `tools/experiments/desktop_server_py`. Historical commands below may still say old paths.

# Phase 0 完成报告

Date: **2026-09-10 09:15–09:20 CST (Asia/Shanghai)**  
Branch: `feat/v0-scaffold` @ `68892faf7a90c0e8d6130576e3970254ccd81f9d`  
Authority plan: `docs/product/03-v1-architecture-rebuild-plan.md`

## Phase X 完成报告

### 修改
- Copied architecture rebuild plan into repo with supersedes front note.
- Recorded git baseline freeze (`feat/v0-scaffold`, not `master`).
- Retitled `V1_STATUS.md` as HISTORICAL / not current V1 complete; pointed to `03` plan.
- Marked `V1_DELIVERY_PLAN.md`, `USER_GUIDE.md`, `ACCEPTANCE.md`, `QT_SHELL.md`, `OSGB_NATIVE_PREVIEW.md` as **CANCELLED FOR V1** regarding Qt / OSGB Preview / `geoforge_shell` / `osgb_viewer` (historical only).
- Did **not** delete `apps/geoforge_shell` or `apps/osgb_viewer`.
- Did **not** add Tauri (Phase 1).
- Did **not** claim V1 complete. No git push.

### 创建文件
- `docs/product/03-v1-architecture-rebuild-plan.md`
- `docs/product/BASELINE_FREEZE.md`
- `docs/product/PHASE_REPORTS/phase-0.md`

### 如何运行
- React UI build: `cd apps/web && npm run build`
- Converter: `/workspace/runtime/3dtile-bin-ktx2/run.sh -f osgb -i <OSGB> -o <OUT> -v`
- Rebuild baseline: `/workspace/venv-3dtiles/bin/python tools/rebuild_top/rebuild_top.py -i <tileset_dir> -o <OUT> --levels 1 -v`
- Existing product shell (unchanged): `bash scripts/run_geoforge.sh` → UI `http://127.0.0.1:8787/`

### 测试

#### 1) React UI build
- command:
  ```bash
  cd /workspace/repos/3dtiles/apps/web && npm run build
  ```
- result: **PASS**
  - `tsc --noEmit && vite build`
  - `✓ 51 modules transformed`
  - `dist/assets/index-m1IAP7Gl.js` 224.34 kB; built in ~923ms

#### 2) Converter smoke (KTX2 wrapper / GEOFORGE_3DTILE runtime)
- command:
  ```bash
  /workspace/runtime/3dtile-bin-ktx2/run.sh -f osgb \
    -i /workspace/data/osgb_one_tile \
    -o /workspace/data/geoforge_outputs/phase0_converter_smoke_20260910_011444 \
    -v
  ```
- result: **PASS** (exit 0)
  - `task over, cost 0.67 s`
  - `tileset.json` present (`TILESET_OK`)
  - output: `/workspace/data/geoforge_outputs/phase0_converter_smoke_20260910_011444`
  - input used: `osgb_one_tile` (small OSGB path; `OSGBny/OSGBny` also available)

#### 3) Python rebuild_top baseline smoke
- command (multi-tile known tileset):
  ```bash
  /workspace/venv-3dtiles/bin/python \
    /workspace/repos/3dtiles/tools/rebuild_top/rebuild_top.py \
    -i /workspace/data/geoforge_outputs/osgbny_p4_keep \
    -o /workspace/data/geoforge_outputs/phase0_rebuild_smoke_osgbny \
    --levels 1 -v
  ```
- result: **PASS** (exit 0)
  - `root children: 6` → `level 1: 6 tiles -> 5 groups`
  - merged parent: `./Data/Merge_L1_3_3/Merge_L1_3_3.b3dm` with `refine REPLACE`
  - `tileset.json` written
  - Also smoke-ran on `one_tile_3dtiles` (1 root child; exit 0, no multi-group merge — expected)

### 验收结果
- [x] Record current git commit + branch in `BASELINE_FREEZE.md` (SHA, Asia/Shanghai date, master vs feat/v0-scaffold note)
- [x] Confirm React UI builds (`npm run build` PASS)
- [x] Confirm converter smoke on small OSGB path (PASS)
- [x] Confirm Python rebuild baseline smoke (PASS, Merge_L1 + REPLACE)
- [x] Add `03-v1-architecture-rebuild-plan.md` (content kept + supersedes front note)
- [x] Update old product docs: Qt / OSGB Preview cancelled for V1; `V1_STATUS` historical
- [x] Do **not** delete Qt/osgb code
- [x] Phase report written
- [x] Baseline still runnable; new architecture doc in repo; historical status clarified
- [ ] V1 complete — **NOT claimed** (Phase 0 only)

### 未解决
- Tauri not yet added (Phase 1).
- Qt / `osgb_viewer` / `geoforge_shell` code still present by design until Phase 4 cleanup.
- New V1 (Tauri + Processor + Proxy HLOD) not started beyond documentation/baseline freeze.
- Converter smoke used keep-convert path (no `--enable-texture-compress` this run); KTX2 wrapper binary itself is present and previously proven.

### 下一阶段
- **Phase 1：Tauri 壳接入现有 React**
  - Add Tauri 2 under `apps/web`
  - Run React + Cesium in Tauri WebView
  - Directory-picker command
  - Remove OSGB Preview UI entry
  - Do **not** replace Python server yet; do not move `apps/web`; do not change rebuild algorithm
