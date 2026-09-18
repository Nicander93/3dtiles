#!/usr/bin/env bash
# Phase 14 smoke: convert+rebuild keep without invoking Python.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

unset PYTHONPATH || true
# Clear stale smoke outs from prior agent shells
unset GEOFORGE_SMOKE_OUT || true
export GEOFORGE_REBUILD_ENGINE=rust
# Do not force Python texture engine
unset GEOFORGE_TEXTURE_ENGINE || true

PROC="${GEOFORGE_PROCESSOR:-$ROOT/target/debug/processor}"
TOP="${GEOFORGE_TOP_REBUILD:-$ROOT/target/debug/top_rebuild}"
if [[ ! -x "$PROC" || ! -x "$TOP" ]]; then
  cargo build -p processor -p top_rebuild --bin top_rebuild
fi
export GEOFORGE_TOP_REBUILD="$TOP"
export GEOFORGE_PROCESSOR="$PROC"

if [[ -z "${GEOFORGE_3DTILE:-}" ]]; then
  for cand in \
    "$ROOT/apps/desktop/src-tauri/resources/bin/run.sh" \
    "$ROOT/.runtime/3dtile-bin/run.sh" \
    "/workspace/runtime/3dtile-bin/run.sh"
  do
    if [[ -x "$cand" ]]; then
      export GEOFORGE_3DTILE="$cand"
      break
    fi
  done
fi

OUT="/tmp/geoforge_phase14_zero_py"
rm -rf "$OUT"
mkdir -p "$OUT"
LOG="$OUT/convert_rebuild.log"

OSGB="${GEOFORGE_SMOKE_OSGB:-/workspace/data/osgb_one_tile}"
if [[ ! -d "$OSGB" ]]; then
  echo "SMOKE_FAIL: OSGB fixture missing at $OSGB" >&2
  exit 1
fi
if [[ -z "${GEOFORGE_3DTILE:-}" || ! -e "${GEOFORGE_3DTILE}" ]]; then
  echo "SMOKE_FAIL: converter not found; set GEOFORGE_3DTILE" >&2
  exit 1
fi

echo "[smoke] convert-osgb + rebuildTop texture=keep (no Python)"
echo "[smoke] GEOFORGE_3DTILE=$GEOFORGE_3DTILE"
echo "[smoke] processor=$PROC which_python=$(command -v python3 || true)"

set +e
env -u PYTHONPATH \
  GEOFORGE_REBUILD_ENGINE=rust \
  GEOFORGE_TOP_REBUILD="$TOP" \
  GEOFORGE_3DTILE="$GEOFORGE_3DTILE" \
  "$PROC" convert-osgb \
  -i "$OSGB" -o "$OUT/final" --texture keep --rebuild-top \
  >"$LOG" 2>&1
RC=$?
set -e

echo "----- log (tail) -----"
tail -n 60 "$LOG" || true

if grep -Ei '(^|[\" /])python[0-9]?([\" ]|$)|rebuild_top\.py|texture_ktx2\.py|uvicorn|desktop_server' "$LOG"; then
  echo "SMOKE_FAIL: python appears to have been invoked" >&2
  exit 1
fi
if ! grep -q 'engine=rust' "$LOG"; then
  echo "SMOKE_FAIL: missing [rebuild] engine=rust" >&2
  exit 1
fi
if ! grep -q 'mode=keep' "$LOG"; then
  echo "SMOKE_WARN: keep mode log line not found (may still be OK)" >&2
fi
if [[ $RC -ne 0 ]]; then
  echo "SMOKE_FAIL: processor exit $RC" >&2
  exit 1
fi
test -f "$OUT/final/tileset.json"

# Optional KTX2 if basisu present
BASISU="${GEOFORGE_BASISU:-$ROOT/vcpkg_installed/x64-linux/tools/basisu/basisu}"
if [[ ! -x "$BASISU" ]]; then
  BASISU="${ROOT}/apps/desktop/src-tauri/binaries/basisu"
fi
if [[ -x "$BASISU" ]]; then
  export GEOFORGE_BASISU="$BASISU"
  KLOG="$OUT/ktx2.log"
  echo "[smoke] process-tileset texture=ktx2-etc1s (rust+basisu)"
  set +e
  env -u PYTHONPATH -u GEOFORGE_TEXTURE_ENGINE \
    GEOFORGE_BASISU="$BASISU" \
    "$PROC" process-tileset \
    -i "$OUT/final" -o "$OUT/ktx2" --texture ktx2-etc1s \
    >"$KLOG" 2>&1
  KRC=$?
  set -e
  tail -n 40 "$KLOG" || true
  if grep -Ei 'python[0-9]?|texture_ktx2\.py' "$KLOG"; then
    echo "SMOKE_FAIL: python used for ktx2" >&2
    exit 1
  fi
  if ! grep -q 'engine=rust' "$KLOG"; then
    echo "SMOKE_FAIL: ktx2 expected engine=rust" >&2
    exit 1
  fi
  if [[ $KRC -ne 0 ]]; then
    echo "SMOKE_FAIL: ktx2 exit $KRC" >&2
    exit 1
  fi
  echo "SMOKE_OK ktx2 via rust+basisu"
else
  echo "[smoke] basisu missing — skip ktx2 (sidecar still required for KTX2)"
fi

echo "SMOKE_OK Phase 14 zero-Python convert+rebuild keep. log=$LOG rc=$RC"
