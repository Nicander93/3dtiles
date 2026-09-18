# Phase 11–18 status (limited V1 — honest)

Date: **2026-09-19 ~00:05 Asia/Shanghai (CST)**  
Branch: `feat/v1-prod-align`  
Companion: [phase-11-18-gap-audit.md](./phase-11-18-gap-audit.md) · [phase-18.md](./phase-18.md) · [SUMMARY.md](./SUMMARY.md)

## One-paragraph claim (authoritative)

**Limited V1 (not production-ready):** continuous regular `Tile_+x_+y` grids only — synthetic ≤16×16 and HK LandsD contiguous **4×4–16×16** convert/rebuild with documented GE/caveats; PlanD Kowloon 4×4 convert+rebuild exists but **Layer B is not fully green** on that upstream (`numErrors` historically 16); automated Cesium A/B far-view gate **2/3 PASS** (requests/bytes; time miss); Windows NSIS is an **installer candidate** with strong CLI/isolate-install evidence (H10/H15), but **clean-machine WebView2 GUI E2E and published converter pin remain open**. Explicitly **not** full production-ready and **no** 百平方公里 / urban claim.

## Scoreboard

| Phase | Status | One-liner |
|-------|--------|-----------|
| **11** Correctness | DONE (on branch) | P0-1…P0-4 + `phase11_correctness` tests after merge |
| **12** Validator | PARTIAL | Layer A present; PlanD Layer B **not** green |
| **13** Staging commit | DONE / PARTIAL vs H04 | Sibling stage+backup on scaffold line; H04 ownership on hardening |
| **14** Zero-Python | DONE (Linux smoke) | Windows install zero-Python GUI still Phase 18 open |
| **15** Acceptance framework | DONE (scripts restored) | |
| **16** HK public data | PARTIAL | LandsD 5×4–16×16 PASS; PlanD 4×4 Layer B FAIL / caveats |
| **17** Cesium A/B | PARTIAL | Far-view **2/3**; near settle incomplete |
| **18** Windows package | PARTIAL | Candidate NSIS; clean-machine WebView + pin **OPEN** |

## Forbidden marketing lines

- 支持大范围倾斜摄影顶层重建 / 百平方公里生产就绪  
- “Production-ready Windows installer” / “clean machine verified”  
- “PlanD Layer B green” without new `numErrors=0` evidence  
