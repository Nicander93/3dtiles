# GeoForge 3D Desktop (Tauri + React)

本地 · 高效 · 开放 — Vite + React + TypeScript Chinese UI in a **Tauri 2** desktop shell.

**Product path (Phase 4+):** `apps/desktop` (renamed from `apps/web`).  
Qt shell / OSGB native viewer removed. Legacy Python API is under `tools/experiments/desktop_server_py` (optional `--legacy-server`).

## Requirements

- Node.js 18+ (20 recommended)
- Rust toolchain for Tauri / `geoforge-desktop`
- Optional: Python only for rebuild baseline / KTX2 until Phase 9
- Linux Tauri: WebKitGTK/GTK deps (see below)

## Run (Tauri — preferred)

```bash
# from repo root
cargo build -p processor
cd apps/desktop
npm install
npm run tauri:dev
```

Compile-only / CI-friendly:

```bash
cd apps/desktop
npm run build
cd src-tauri && cargo build -p geoforge-desktop
# or: npm run tauri:build
```

## Run (browser / Vite)

```bash
cd apps/desktop
npm install
npm run dev
```

Open http://127.0.0.1:5173. For the old FastAPI shell:

```bash
bash scripts/run_geoforge.sh --legacy-server   # http://127.0.0.1:8787/
```

## Linux apt deps (Debian/Ubuntu)

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev patchelf \
  libssl-dev libayatana-appindicator3-dev
```

## Env

- `VITE_API_BASE=http://127.0.0.1:8787` (browser/legacy)
- `VITE_TILES_PREVIEW_URL=/cesium-preview.html`
- Processor: `GEOFORGE_3DTILE`, `GEOFORGE_REBUILD_TOP`, `GEOFORGE_PYTHON`

## Routes

- `/` 工作区 · `/osgb/convert` · `/processing` · `/preview/tiles` · `/history` · `/results` · `/tiles/process` · `/settings`

Removed: `/preview/osgb` (Phase 1). Qt apps deleted (Phase 4).

## Key files

- `src/` — React UI
- `src/api/desktop.ts` — Tauri / HTTP adapter
- `src-tauri/` — Tauri 2 (`geoforge-desktop`)
