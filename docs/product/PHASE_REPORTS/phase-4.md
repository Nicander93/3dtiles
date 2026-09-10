## Phase 4 完成报告

Date: **2026-09-10 ~10:07 CST** (Asia/Shanghai)  
Scope: code cleanup & directory convergence (plan §22 Phase 4).  
**No git push.** Did **not** start TopRebuild C++ (Phase 5+). **No claim** of large-area rebuild production support.

### 修改

#### Deleted (product paths)
- `apps/geoforge_shell/` (entire tree)
- `apps/osgb_viewer/` (entire tree)
- `scripts/run_geoforge_shell.sh`
- `scripts/run_osgb_viewer.sh`

#### Historical docs
- Moved full notes → `docs/product/historical/QT_SHELL.md`, `docs/product/historical/OSGB_NATIVE_PREVIEW.md`
- Left stubs at `docs/product/QT_SHELL.md` / `OSGB_NATIVE_PREVIEW.md` pointing to historical/

#### Renamed
- `apps/web` → `apps/desktop` (including `src-tauri`, crate `geoforge-desktop`)
- Root `Cargo.toml` `workspace.exclude` → `apps/desktop/src-tauri`
- `.gitignore` node_modules/dist comments updated
- `package.json` name → `geoforge-desktop`
- Docs / USER_GUIDE / LOCAL_VERIFY / PUSH_CANDIDATES / examples comments updated
- Phase 0–3 reports: path-note banner (historical commands may still say old paths)

#### Python rebuild baseline
- `tools/rebuild_top` → `tools/experiments/rebuild_top_py`
- README: **baseline only, not release runtime**
- `crates/processor` + `crates/rebuild_top_cli`: new path with **one-release fallback** to `tools/rebuild_top/`
- `GEOFORGE_REBUILD_TOP` defaults updated in `scripts/run_geoforge.sh` / `scripts/rebuild_sample.sh` / legacy runner

#### desktop_server
- Moved `apps/desktop_server` → `tools/experiments/desktop_server_py` (+ README)
- `tools/texture_ktx2/run.py` loads module from experiments path (legacy fallback kept)
- `scripts/run_geoforge.sh`: default prints Tauri/processor usage; optional `--legacy-server` starts Python API; if `DISPLAY` set and tauri CLI present, may start `tauri:dev`

### 关键 / 主要路径
- `apps/desktop/` (was `apps/web`)
- `tools/experiments/rebuild_top_py/`
- `tools/experiments/desktop_server_py/`
- `docs/product/historical/`
- `docs/product/PHASE_REPORTS/phase-4.md`

### 如何运行
```bash
# Preferred
cargo build -p processor
cd apps/desktop && npm install && npm run tauri:dev

# Helper
bash scripts/run_geoforge.sh              # usage / optional tauri:dev
bash scripts/run_geoforge.sh --legacy-server   # reference Python API :8787

# Rebuild baseline (experiment)
python tools/experiments/rebuild_top_py/rebuild_top.py -i <tileset> -o <out> -v
```

### 测试
| command | result |
|--------|--------|
| `cargo build -p processor` | **PASS** |
| `cd apps/desktop/src-tauri && cargo build -p geoforge-desktop` | **PASS** (after wiping stale `target/` that still pointed at `apps/web`) |
| `cd apps/desktop && npm run build` | **PASS** (`tsc` + vite) |
| `processor process-tileset --input /workspace/data/OSGBny_3dtiles --output /tmp/phase4-rebuild-smoke --rebuild-top` | **PASS** — invoked `tools/experiments/rebuild_top_py/rebuild_top.py`; exit 0; tileset committed |
| `bash scripts/run_geoforge.sh` | prints Rebuild script = `…/tools/experiments/rebuild_top_py/rebuild_top.py` |

### 验收结果
- [x] Qt product apps + scripts deleted
- [x] Qt docs → historical stubs
- [x] `apps/web` → `apps/desktop`; path refs fixed for build/run
- [x] Python rebuild under `tools/experiments/rebuild_top_py` with baseline README
- [x] `desktop_server` under experiments; run script prefers Tauri/processor
- [x] Product runtime view: Desktop + Processor (+ processing core); Python only as experiment/baseline
- [x] No TopRebuild C++ started
- [x] No large-area rebuild support claimed

### 未解决
- Formal TopRebuild C++ still Phase 5+
- KTX2 still via Python `texture_ktx2` in experiments tree
- `prepare_osgb_preview.sh` retained (GLB Three.js helper; not Qt product path)
- Historical phase reports / V1_STATUS archaeology still mention old paths in body text (banner notes Phase 4 rename)

### 下一阶段
- **Phase 5:** TopRebuild Core 基础模型 + TreeBuilder（SourceBlock / Representation / Quadtree）
- Do not claim large-dataset rebuild until Phase 10

### Final tree (apps/ & tools/)
```text
apps/
└── desktop/                 # React + Tauri (was apps/web)
    ├── src/
    ├── src-tauri/           # geoforge-desktop
    ├── package.json
    └── README.md

tools/
├── experiments/
│   ├── rebuild_top_py/      # Python baseline only
│   └── desktop_server_py/   # legacy FastAPI reference
├── ktx2_postprocess/
└── texture_ktx2/            # wrapper → desktop_server_py module
```
