# Phase 18 — Windows production package (honest)

Date: **2026-09-19 ~00:05 Asia/Shanghai (CST)**  
Branch: `feat/v1-prod-align` @ post-Phase-17 tip (+ this docs/packaging fixup)  
Authority: production-readiness plan §20 · hardening **H10 / H15** · `RELEASE_ACCEPTANCE.md` · `05-v1-hardening-release-runbook.md`

**Verdict: PARTIAL — installer candidate exists; clean-machine WebView GUI E2E and pinned converter publish remain OPEN. Do NOT claim full production package.**

---

## 1. Inventory (what exists)

### NSIS / Tauri packaging scripts

| Path | Role |
|------|------|
| `apps/desktop/scripts/package-windows.ps1` | Single Windows entry: prepare runtime → stage `resources/runtime` → sidecars/Cesium → required-file checklist → `tauri build --bundles nsis` |
| `apps/desktop/scripts/prepare-runtime.ps1` | Build `processor`/`top_rebuild` Release; fetch/stage converter zip; copy MSVC CRT beside product bins; write `manifest.json` |
| `apps/desktop/scripts/prepare-converter.ps1` | Download pinned zip from `third_party/3dtiles-converter.json` **or** accept `-LocalZip` / `-ConverterZip` candidate |
| `apps/desktop/scripts/prepare-texture.ps1` | Stage `geoforge-texture` + `basisu` under runtime `texture/` |
| `apps/desktop/scripts/fix-basisu-crt.ps1` | Ensure MSVC CRT beside `basisu.exe` (0xc0000135 packaging fix) |
| `apps/desktop/scripts/prepare-sidecars.mjs` | Copy `processor`/`top_rebuild` (+ optional `basisu`) into `src-tauri/binaries/*-<triple>` |
| `apps/desktop/package.json` | `package:windows`, `prepare:runtime`, `prepare:converter`, `prepare:sidecars` |
| `apps/desktop/src-tauri/tauri.conf.json` | `externalBin`: processor, top_rebuild, basisu; `resources`: **both** `resources/runtime/` (Windows) and `resources/bin/` (Linux Phase 14) |
| `.github/workflows/release-windows.yml` | `windows-latest` NSIS build, runtime smoke (`_3dtile --help`, capabilities), artifact + optional GitHub Release on `v*.*.*` tags |
| `.github/workflows/release.yml` / `windows.yml` | Broader CI; not a substitute for clean-machine GUI |

### Linux / zero-Python staging (Phase 14; not Windows NSIS)

| Path | Role |
|------|------|
| `scripts/release/stage_sidecars.sh` | Stage processor / top_rebuild / basisu / Linux `resources/bin` converter |
| `scripts/release/smoke_zero_python.sh` | Convert+rebuild keep without invoking Python (Linux evidence) |

### RELEASE / runbook docs

| Doc | What it proves / gates |
|-----|------------------------|
| `docs/product/RELEASE_ACCEPTANCE.md` | CI vs Windows install checklist; many CI/NSIS CLI rows **[x]**; WebView / clean GUI rows **[ ]** |
| `docs/product/05-v1-hardening-release-runbook.md` | How to rebuild NSIS; install runtime checks; **§4 human Windows acceptance** still required |
| `docs/product/HARDENING_PROGRESS.md` | H10 = candidate verified, formal pin pending; H15 = CI done, install GUI partial |
| `docs/product/HARDENING_REGRESSION.md` | Regressions **15, 31, 43, 52–56, 59, 61–63** — NSIS isolate install + CLI convert / CRT / Unicode / KTX2 |
| `third_party/3dtiles-converter.json` | **Published** pin still `v0.1.0` SHA256 `77cadd01…` — **not** the Unicode/CRT candidate |

### H10 / H15 evidence (summary)

| Card | Proven | Still open |
|------|--------|------------|
| **H10** | Local candidate converter zip → staging → NSIS; `--help`; real OSGB; CRT in converter+bin; Chinese / space paths (regs 52–55, 61) | Fixed **published** URL+SHA256 for candidate; do not rewrite pin from `.cache` zip |
| **H15** | `npm ci`/tests, Rust/Tauri lib tests, space-in-path install, HK 8×8 CLI ladder, Job Object unit tests | Installed **WebView2** Cesium preview; close-with-active-task GUI; user failure samples; ACL/disk-full UI; true **no global VC++** clean VM (static CRT audit ≠ VM proof) |

Latest documented candidate NSIS SHA256 examples (historical, not “clean pass”):  
`a6b92ecd…` (reg 61), `bcd0f067…` (reg 62), `1bfcfd42…` (reg 63 with texture).

---

## 2. Proven vs missing (clean-machine / production package)

### Proven (honest)

- Tauri 2 + NSIS **candidate** packaging pipeline on Windows (scripts + Actions workflow).
- Isolated silent install + first-start / `tasks.db` (with isolation caveats on restricted agents).
- Packaged converter + processor + top_rebuild CLI smokes; MSVC CRT **bundled** beside converter/bin/texture.
- Space-in-path install; Chinese input/output / Tile names on install runtime (candidate converter `bbe1426` line).
- Texture sidecars (`geoforge-texture` / `basisu`) + KTX2 capability gating when bundled (reg 62–63).
- Linux Phase 14 zero-Python smoke (`smoke_zero_python.sh`) — **not** the same as Windows install zero-Python GUI E2E.
- Desktop crate packaging APIs restored on this branch after merge: `ProcessManager::packaged_runtime_root` → `GEOFORGE_RUNTIME_ROOT` / `GEOFORGE_PACKAGED` (required for install semantics).

### Missing / OPEN (do not greenwash)

| Gate | Status |
|------|--------|
| Clean machine: no Git/Rust/Node/Python/vcpkg/source tree, **no** global VC++ | **OPEN** (CRT-in-zip helps; true clean VM not recorded as pass) |
| Installed **WebView2** GUI E2E: convert + rebuild + KTX2 + Cesium preview + cancel | **OPEN** (`RELEASE_ACCEPTANCE` WebView rows unchecked) |
| Pinned converter hash **publish**: candidate zip at fixed URL + update `third_party/3dtiles-converter.json` | **OPEN** (pin still v0.1.0 / `77cadd01…`) |
| Zero-Python sidecars on **Windows install image** (no `GEOFORGE_*` / Docker / Python) | **PARTIAL** — design/packaged mode exists; install GUI proof missing |
| Close window with active task → interrupted + no orphan processes + restart | **OPEN** (unit + force-kill evidence ≠ WebView click) |
| 50 GB-class / urban / 百平方公里 | **NOT demonstrated** |

---

## 3. Improvements made this session (no Windows GUI, no fake pass)

1. **`tauri.conf.json`**: fixed merge duplicate `externalBin` / `resources` keys so **both** `resources/runtime/` and `resources/bin/` + `basisu` externalBin are retained.
2. **Restored** hardening `process_manager.rs` + `commands.rs` (packaged runtime root, shutdown, processor capabilities) so desktop lib **compiles** again after scaffold merge regression; removed redundant `unsafe` under `-D warnings`.
3. **`package-windows.ps1`**: early abort on Linux/macOS with Windows CI/dev recipe (exit 2).
4. **`prepare-sidecars.mjs`**: optional `basisu` externalBin staging when present.
5. **`resources/runtime/README.md`**: expected Windows layout pointer.
6. This report + `phase-11-18-status.md` + status banner updates.

**This Linux box cannot produce NSIS.** Build on Windows:

```powershell
# Dev machine (from repo root)
pwsh -NoProfile -ExecutionPolicy Bypass -File apps\desktop\scripts\package-windows.ps1
# Optional candidate converter (does NOT update published pin):
#   ... -ConverterZip D:\path\geoforge-converter-0.1.1-unicode-crt.zip

# Or GitHub Actions
# Actions → Release Windows → workflow_dispatch
# or push tag v*.*.*
```

Required on Windows: Rust stable, Node 20, WebView2 runtime (Edge/WebView2 Evergreen), MSVC build tools for product crates, network for pinned converter zip (or `-ConverterZip`).

---

## 4. Pin checklist (before claiming “published package”)

- [ ] Publish converter zip with versioned URL (not `.cache` only).
- [ ] Update `third_party/3dtiles-converter.json` `windowsX64.url` + `sha256` to that release.
- [ ] Rebuild NSIS **without** `-ConverterZip` override; record installer SHA256.
- [ ] Clean VM install: no global VC++ redist, no `GEOFORGE_*`, no Docker, no Python on PATH for convert/rebuild/KTX2.
- [ ] GUI: convert → rebuild → KTX2 (if claimed) → Cesium WebView load; cancel; close-with-active-task; restart.
- [ ] Fill every remaining `[ ]` in `RELEASE_ACCEPTANCE.md` Windows section with pass/fail + evidence path.
- [ ] Do **not** mark Phase 18 DONE until the above are evidence-backed.

---

## 5. Relation to Phases 11–17

| Phase | Relevance to package claim |
|-------|----------------------------|
| 11–14 | Code on branch after merge; Linux zero-Python smoke ≠ Windows install proof |
| 15–16 | PlanD HK 4×4 Layer B historically **FAIL**; LandsD 5×4–16×16 separate evidence |
| 17 | Cesium A/B far-view **2/3 PASS** on PlanD 4×4 (harness) — not install WebView |
| 18 | This file — package **candidate**, clean-machine **OPEN** |

---

## Non-claims

- **NOT** full production-ready.
- **NOT** 百平方公里 / urban scale.
- **NOT** “clean-machine WebView E2E passed.”
- Candidate NSIS SHA256 lists are **dev-machine / isolated-dir** evidence, not published pin proof.
