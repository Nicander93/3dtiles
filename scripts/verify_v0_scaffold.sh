#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== file contracts =="
test -f docs/V0_SCOPE.md
test -f docs/REBUILD_TOP.md
test -f docs/UI_PLAN.md
test -f crates/rebuild_top_cli/src/lib.rs
rg -q 'rebuild_top_cli::command' src/main.rs
rg -q 'rebuild-top' README_ZH.md docs/V0_SCOPE.md

echo "== crate unit tests (no native OSG/CMake) =="
cargo test -p rebuild_top_cli -- --nocapture

echo "== stub CLI help via clap (example binary in tests) =="
cargo test -p rebuild_top_cli parses_rebuild_top_args -- --nocapture

echo "OK: v0 scaffold verification passed"
