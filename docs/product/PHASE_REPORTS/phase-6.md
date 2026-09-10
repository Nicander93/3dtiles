## Phase 6 完成报告

Date: **2026-09-10 ~10:16 CST** (Asia/Shanghai)  
Scope: ProxyBuilder geometry prototype — B3DM→GLB, scene transform expand, parent local frame, primitive grouping, meshoptimizer simplify, proxy GLB writer, simplification error metric (plan §22 Phase 6, §§12–15).  
**No git push.** **Texture optimization OFF.** **No weld / atlas / remesh.** **No TilesetWriter HLOD** (Phase 7). Python rebuild baseline **not** replaced.

### 修改
- `crates/top_rebuild/Cargo.toml` — deps: `meshopt` 0.6, `gltf` 1.4 (`utils`, no default), `byteorder` 1.5
- `crates/top_rebuild/src/types.rs` — `Mat4d::inverse`, `from_gltf_cols`, `transform_direction`
- `crates/top_rebuild/src/lib.rs` — export proxy modules
- `crates/top_rebuild/src/bin/top_rebuild_debug.rs` — subcommands `tree` | `proxy` (legacy `--input` kept)

### 新增文件
- `crates/top_rebuild/src/b3dm.rs` — B3DM ↔ GLB payload extract/pack; `load_content_glb` (`.b3dm` / `.glb`)
- `crates/top_rebuild/src/glb.rs` — glTF scene-transform expand (tinygltf-equivalent), minimal GLB writer (no textures), subdivided box fixture mesh
- `crates/top_rebuild/src/proxy_builder.rs` — ProxyBuilder: parent local frame, cross-child grouping, meshopt `simplify` with `LockBorder|ErrorAbsolute`, budget / `BUDGET_NOT_REACHED`, error metric
- `crates/top_rebuild/tests/proxy_2x2.rs` — transform + 2×2 B3DM→proxy + GLB-only path
- `tests/fixtures/top_rebuild/proxy_2x2/` — generated 4× child B3DM + `Proxy_L1_0_0.glb` + README

### 如何运行
```bash
# Unit + integration (Phase 5 + 6)
cargo test -p top_rebuild

# Phase 6 CLI: synthesize 2×2 boxes → parent proxy GLB
cargo run -p top_rebuild --bin top_rebuild_debug -- proxy \
  --out tests/fixtures/top_rebuild/proxy_2x2/Proxy_L1_0_0.glb \
  --max-triangles 1500 --segments 8

# Optional validate
npx @gltf-transform/cli validate tests/fixtures/top_rebuild/proxy_2x2/Proxy_L1_0_0.glb
```

### 测试
| command | result |
|--------|--------|
| `cargo test -p top_rebuild` | **PASS** (14 lib unit + 2 grid_4x4 + 3 proxy_2x2) |
| `top_rebuild_debug proxy --out …/Proxy_L1_0_0.glb --max-triangles 1500 --segments 8` | **PASS** — see evidence below |
| `npx @gltf-transform/cli validate …/Proxy_L1_0_0.glb` | **PASS** — “No errors found” |

### 验收结果（证据）

**CLI proxy build**
```text
proxy_glb tests/fixtures/top_rebuild/proxy_2x2/Proxy_L1_0_0.glb
triangles_before 3072
triangles_after 1500
triangles_target 1500
simplification_error_meters 0.003768
group_count 1
```

**Integration (`proxy_2x2`)**
```text
proxy_2x2 OK tris 3072 -> 1536 err=0.0033 groups=1
path=…/target/top_rebuild_proxy_2x2_test/Proxy_L1_0_0.glb
```

**Transform test** (`to_parent_local` / `transform_parent_local_frame`)
- Child world `(50,50,10)` → parent `(100,100,10)`: local origin maps to `(-50,-50,0)`
- ECEF-scale translations (`6_378_000`) keep double-precision until float mesh store; residual &lt; 1e-6 in parent local

**Files**
- Children: `tests/fixtures/top_rebuild/proxy_2x2/_proxy_children/child_{0..3}.b3dm` (~21 KiB each)
- Proxy: `tests/fixtures/top_rebuild/proxy_2x2/Proxy_L1_0_0.glb` (~45 KiB), reloadable via `gltf` crate

**Texture policy:** OFF — writer emits PBR color materials only; no image/atlas/KTX2.

**Boundary / weld:** V1 no weld; simplify uses `SimplifyOptions::LockBorder` (topological border lock). Gap metrics deferred to Phase 8.

### 未解决 / 明确不做（本阶段）
- No TilesetWriter / REPLACE HLOD / geometricError write (Phase 7)
- No texture resize / atlas / hash dedup / KTX2 (Phase 8)
- No cross-tile weld / remesh / simplifySloppy fallback beyond warning
- Processor still uses Python `tools/experiments/rebuild_top_py`
- Existing `grid_4x4` fixture still has empty stub `.b3dm` (0 bytes); Phase 6 uses dedicated `proxy_2x2` / synthetic generator

### 下一阶段
- Phase 7: TilesetWriter + two-level HLOD (16→4→1), world transform invariant, Cesium far/near refine
