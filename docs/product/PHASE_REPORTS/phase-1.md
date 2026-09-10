> **Path note (Phase 4):** `apps/web` → `apps/desktop`; `tools/rebuild_top` → `tools/experiments/rebuild_top_py`; Qt apps deleted; `desktop_server` → `tools/experiments/desktop_server_py`. Historical commands below may still say old paths.

# Phase 1 完成报告

Date: **2026-09-10 09:41 CST** (Asia/Shanghai)  
Branch: `feat/v0-scaffold` (working tree; **no git push**)  
Authority plan: `docs/product/03-v1-architecture-rebuild-plan.md` §Phase 1 / §24

## Phase X 完成报告

### 修改
- Added **Tauri 2** desktop shell under `apps/web/src-tauri/` (crate `geoforge-desktop`); React stays in `apps/web` (not moved to `apps/desktop`).
- Wired directory/file pickers as Tauri commands: `select_input_directory`, `select_output_directory`, `select_tileset_file` (`tauri-plugin-dialog`).
- Frontend: `src/lib/tauri.ts` detects Tauri vs browser; browse buttons on OSGB Convert / Tiles Process; API still hits Python `desktop_server` at `http://127.0.0.1:8787` (absolutize relative artifact URLs in Tauri).
- **Removed OSGB Preview** from product UI: deleted `OsgbPreview.tsx`; removed Router / Sidebar / Workspace entries; Settings/USER_GUIDE updated.
- Did **not** replace task store / Python server; did **not** delete `geoforge_shell` / `osgb_viewer`; did **not** change rebuild algorithm.
- Root `Cargo.toml` `workspace.exclude = ["apps/web/src-tauri"]` so Tauri is an independent workspace.
- Docs: USER_GUIDE preferred path → Tauri; BASELINE_FREEZE Phase 1 status; this report.

### 关键文件
- `apps/web/src-tauri/` (Cargo.toml, tauri.conf.json, src/lib.rs, capabilities, permissions, icons, …)
- `apps/web/src/lib/tauri.ts`
- `apps/web/src/App.tsx`, `layouts/AppLayout.tsx`, `pages/Workspace.tsx`, `OsgbConvert.tsx`, `ProcessTiles.tsx`, `TilesPreview.tsx`, `Settings.tsx`
- `apps/web/src/api/client.ts` (Tauri API base + `absolutizeLocalUrl`)
- `apps/web/vite.config.ts`, `package.json`, `README.md`
- Deleted: `apps/web/src/pages/OsgbPreview.tsx`
- `Cargo.toml` (workspace exclude)
- `docs/product/USER_GUIDE.md`, `BASELINE_FREEZE.md`, `V1_STATUS.md`
- `docs/product/PHASE_REPORTS/phase-1.md`

### 如何运行

**A. Preferred non-Qt desktop (Phase 1)**

```bash
# Terminal A — keep existing Python API
bash scripts/run_geoforge.sh
# curl -s http://127.0.0.1:8787/api/health

# Terminal B — Tauri window (needs DISPLAY, e.g. :2)
cd apps/web
# ensure ~/.cargo/bin on PATH (rustup stable ≥ 1.88)
npm run tauri:dev
# or run built binary:
# ./src-tauri/target/debug/geoforge-desktop
```

**B. Browser baseline (unchanged)**

```bash
bash scripts/run_geoforge.sh   # UI http://127.0.0.1:8787/
# or: cd apps/web && npm run dev
```

**C. Compile-only**

```bash
cd apps/web && npm run build
cd src-tauri && cargo check
# or: cd apps/web && npm run tauri:build   # full; uses beforeBuildCommand
```

**Linux apt deps (Debian/Ubuntu) for WebView build**

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
  patchelf libssl-dev libayatana-appindicator3-dev
```

Note: `fuse3` postinst can hang on some containers (`modprobe`/device); stubbing `/var/lib/dpkg/info/fuse3.postinst` unblocked configure here. WebKit/GTK themselves configured fine afterward.

**Rust**: system `rustc 1.85` is too old for current Tauri crate graph; use user-local rustup stable (verified **1.98.1**).

### 测试

#### 1) React UI build
- command: `cd apps/web && npm run build`
- result: **PASS** — `tsc --noEmit && vite build` (53 modules; dist includes Tauri invoke helpers / no OSGB Preview route)

#### 2) `cargo check` (src-tauri)
- command: `cd apps/web/src-tauri && cargo check` (PATH=`$HOME/.cargo/bin`)
- result: **PASS** — `Finished `dev` profile … in 47.16s`

#### 3) `tauri build --debug --no-bundle`
- command: `cd apps/web && npx tauri build --debug --no-bundle`
- result: **PASS** — `Built application at: …/src-tauri/target/debug/geoforge-desktop` (~211 MB debug ELF)

#### 4) Runtime smoke (DISPLAY=:2)
- command: run `geoforge-desktop` under `DISPLAY=:2`
- result: **PASS (process + window)** — process stayed alive; `xdotool` found window named `geoforge-desktop` / GeoForge. AT-SPI bus warning only (harmless). Full visual Cesium interaction not screenshot-captured (no ImageMagick/xwd in env; apt fetch 502 for imagemagick). Cesium path remains `/preview/tiles` + `public/cesium-preview.html` in the WebView asset tree; API/tileset URLs absolutized to `:8787` when in Tauri.

### 验收结果
- [x] Tauri 2 under `apps/web/src-tauri/`
- [x] React builds and is packaged into Tauri frontendDist
- [x] Directory-select commands implemented + UI browse buttons (Tauri only)
- [x] OSGB Preview removed from Router / Sidebar / Workspace; page deleted
- [x] Still calls Python HTTP API on :8787 (not replaced)
- [x] Did not move `apps/web` → `apps/desktop`
- [x] Did not delete `geoforge_shell` / `osgb_viewer`
- [x] Did not change rebuild algorithm
- [x] Non-Qt desktop binary compiles and launches on DISPLAY=:2
- [x] Phase report + USER_GUIDE / BASELINE notes
- [ ] V1 complete — **NOT claimed** (Phase 1 only)

### 未解决 / blockers
- **DISPLAY** required for `tauri dev` / interactive WebView (this box uses `:2`).
- **rustc ≥ ~1.88** via rustup (system 1.85 insufficient for current lockfile).
- **WebKitGTK 4.1 + GTK3 -dev** required for compile/link (documented apt list).
- `fuse3` dpkg configure can hang in container; document workaround if hit.
- Cesium end-to-end in WebView not screenshot-proven here (tooling gap); code path + compile + window smoke OK.
- Python `desktop_server` still required until Phase 2–3.
- API helpers `nativeOsgbPreview` / `prepareOsgb` remain in `client.ts` (no UI); server endpoints untouched until later cleanup.

### 下一阶段
- **Phase 2：Tauri Task Store / Resource Server**
  - Rust SQLite task/artifact/settings store
  - Local artifact HTTP server for Cesium
  - Frontend `src/api/desktop.ts` adapter (pages should not call `invoke` directly long-term)
  - Still do not delete Qt apps until Phase 4
