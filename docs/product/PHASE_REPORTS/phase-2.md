> **Path note (Phase 4):** `apps/web` → `apps/desktop`; `tools/rebuild_top` → `tools/experiments/rebuild_top_py`; Qt apps deleted; `desktop_server` → `tools/experiments/desktop_server_py`. Historical commands below may still say old paths.

# Phase 2 完成报告

Date: **2026-09-10 09:49 CST** (Asia/Shanghai)  
Branch: working tree under `/workspace/repos/3dtiles` (**no git push**)  
Authority plan: `docs/product/03-v1-architecture-rebuild-plan.md` §Phase 2 / §5.4 / §24

## Phase 2 完成报告

### 修改
- **Rust SQLite** in `apps/web/src-tauri`: `tasks`, `artifacts`, `settings` tables under app data dir (`GEOFORGE_DATA_DIR` or Tauri app_data / `.geoforge`).
- Modules: `db.rs`, `task_store.rs`, `artifact_store.rs`, `settings_store.rs`, `resource_server.rs`, `python_bridge.rs`, `commands.rs`, `state.rs`.
- **Local artifact HTTP server** (axum): bind `127.0.0.1` ephemeral/fixed port; serve only registered roots; canonicalize + reject `..`; Content-Type; Range; CORS for Cesium.
- Tauri commands (§5.4 + helpers): `submit_task`, `cancel_task`, `get_task`, `list_tasks`, `get_task_logs`, `list_artifacts`, `register_artifact`, `get_preview_url`, `open_artifact_directory`, `get_settings`, `update_settings`, `get_resource_server_info` (+ Phase 1 dialogs).
- Frontend: `apps/web/src/api/desktop.ts` dual-mode adapter (prefer Tauri `invoke` when `__TAURI__`; else HTTP `desktop_server`). Pages import adapter; `lib/tauri.ts` re-exports (no direct page `invoke`).
- Settings page persists via Rust SQLite in Tauri (localStorage fallback in browser).
- Results: “打开目录” via `open_artifact_directory` in Tauri.
- TilesPreview: resolves preview via `get_preview_url` → Rust `http://127.0.0.1:<port>/artifacts/<id>/tileset.json`.
- Smoke example: `cargo run --example phase2_smoke`.
- **Did not** delete `apps/desktop_server`, move `apps/web`, or start `crates/processor` (Phase 3).

### 执行桥（诚实说明）
| Capability | Owner now |
|---|---|
| Task / artifact / settings persistence | **Rust SQLite** |
| Cesium tileset HTTP | **Rust resource server** |
| `submit_task` / `cancel_task` **execution** | Still **HTTP-proxied** to Python `desktop_server` (:8787); status polled back into Rust SQLite; on success artifact registered for Rust preview |
| `health` / `capabilities` / `scanOsgb` | Still **Python HTTP** |
| Processor crate | **Not started** (Phase 3) |

### 关键文件
- `apps/web/src-tauri/Cargo.toml` (+ rusqlite, axum, reqwest, …)
- `apps/web/src-tauri/src/{lib,commands,db,task_store,artifact_store,settings_store,resource_server,python_bridge,state}.rs`
- `apps/web/src-tauri/examples/phase2_smoke.rs`
- `apps/web/src-tauri/permissions/path-dialogs.toml`, `capabilities/default.json`
- `apps/web/src/api/desktop.ts`, `api/client.ts` (comment), `lib/tauri.ts`
- Pages/hooks: Settings, Results, TilesPreview, OsgbConvert, ProcessTiles, Processing, History, Workspace, useTasks, AppLayout
- `docs/product/PHASE_REPORTS/phase-2.md`

### 如何运行

**Smoke (no GUI) — Phase 2 acceptance core**
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd apps/web/src-tauri
cargo run --example phase2_smoke
# expect: http_status=200 OK … PHASE2_SMOKE_OK
```

**Desktop (Tauri) + optional Python executor**
```bash
# Terminal A — still needed for convert/scan execution
bash scripts/run_geoforge.sh

# Terminal B
cd apps/web
npm run tauri:dev
# or: ./src-tauri/target/debug/geoforge-desktop
```

**Browser fallback (unchanged HTTP)**
```bash
bash scripts/run_geoforge.sh   # UI + API :8787
```

### 测试
#### 1) `cargo check` / `cargo build` (geoforge-desktop)
- command: `cd apps/web/src-tauri && cargo check && cargo build`
- result: **PASS**

#### 2) `npm run build`
- command: `cd apps/web && npm run build`
- result: **PASS** — `tsc --noEmit && vite build`

#### 3) Phase 2 smoke (register tileset → Rust server → curl 200)
- command: `cargo run --example phase2_smoke`
- fixture: `examples/preview/tiles/tileset.json`
- result: **PASS**
  - SQLite persist OK
  - `GET http://127.0.0.1:<port>/artifacts/<id>/tileset.json` → **200** (3491 bytes)
  - path traversal → client error
  - settings persist OK
  - `PHASE2_SMOKE_OK`

### 验收结果
- [x] Rust SQLite task store (status, stages, log path, params snapshot)
- [x] artifact store + settings in same SQLite (app data dir)
- [x] Local artifact HTTP server on 127.0.0.1 (path-traversal safe, Range, CORS)
- [x] Frontend `src/api/desktop.ts` adapter; pages do not call `invoke()` directly
- [x] Commands from §5.4 wired (plus `register_artifact` / `get_resource_server_info` / logs)
- [x] Restart persistence (SQLite) proven in smoke
- [x] Cesium tileset URL served from **Rust** localhost (smoke curl 200; UI uses `get_preview_url`)
- [x] `desktop_server` retained; `apps/web` not moved; Processor not started
- [ ] Full convert E2E without Python — **deferred to Phase 3**
- [ ] V1 complete — **NOT claimed**

### 未解决 / blockers
- **Convert/scan/capabilities** still require Python `desktop_server` until Phase 3 Processor.
- `submit_task` creates Rust task then **bridges** execution to Python; dual task IDs linked via `pythonTaskId` / `progress.pythonTaskId`.
- Interactive Tauri WebView Cesium screenshot not re-run this phase (DISPLAY-dependent); smoke proves Rust HTTP path Cesium needs.
- Path traversal probe may surface as **404** (axum route normalize) rather than 400; still blocked.
- System `rustc` may be too old; use rustup stable (≥1.88) as in Phase 1.

### 下一阶段
- **Phase 3：Processor**
  - `crates/processor` + JSONL events
  - convert-osgb / process-tileset pipelines
  - Tauri spawn Processor; retire Python HTTP for execution
  - Keep `desktop_server` code as reference until migration proven
