#!/usr/bin/env bash
# Phase 12 Layer B — CesiumGS 3d-tiles-validator (dev / acceptance only).
# Does NOT ship Node into the desktop release runtime.
#
# Usage:
#   scripts/acceptance/run_layer_b_validator.sh [tileset.json] [report.json]
#
# Gate: report numErrors == 0 (and process exit 0 when the tool fails hard).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

pick_tileset() {
  local candidates=(
    "/workspace/data/geoforge_outputs/phase12_layer_b_rebuild/tileset.json"
    "/workspace/data/geoforge_outputs/phase8_hlod_4x4/tileset.json"
    "/workspace/data/geoforge_outputs/phase10_4x4/tileset.json"
    "$ROOT/tests/fixtures/top_rebuild/hlod_4x4/tileset.json"
  )
  local c
  for c in "${candidates[@]}"; do
    if [[ -f "$c" ]]; then
      local dir
      dir="$(dirname "$c")"
      if find "$dir" -name '*.b3dm' -size +32c -print -quit | grep -q .; then
        echo "$c"
        return 0
      fi
    fi
  done
  for c in "${candidates[@]}"; do
    [[ -f "$c" ]] && { echo "$c"; return 0; }
  done
  return 1
}

TILESET="${1:-}"
if [[ -z "$TILESET" ]]; then
  TILESET="$(pick_tileset)" || {
    echo "ERROR: no tileset.json found for Layer B" >&2
    exit 2
  }
fi
TILESET="$(cd "$(dirname "$TILESET")" && pwd)/$(basename "$TILESET")"

REPORT="${2:-$ROOT/acceptance-results/phase12-layer-b/validator-report.json}"
mkdir -p "$(dirname "$REPORT")"

if ! command -v npx >/dev/null 2>&1; then
  echo "ERROR: npx/node required for Layer B (dev/acceptance only)" >&2
  exit 2
fi

echo "[layer-b] tileset=$TILESET"
echo "[layer-b] report=$REPORT"
echo "[layer-b] invoking: npx --yes 3d-tiles-validator --tilesetFile ... --reportFile ..."

set +e
npx --yes 3d-tiles-validator \
  --tilesetFile "$TILESET" \
  --reportFile "$REPORT"
RC=$?
set -e

echo "[layer-b] tool_exit=$RC"
NUM_ERRORS="?"
if [[ -f "$REPORT" ]]; then
  echo "[layer-b] report written: $REPORT"
  NUM_ERRORS="$(python3 - << PY
import json
from pathlib import Path
p = Path(r'''$REPORT''')
try:
    d = json.loads(p.read_text())
    print(d.get('numErrors', 'missing'))
except Exception as e:
    print('parse_error')
PY
)"
  echo "[layer-b] numErrors=$NUM_ERRORS"
else
  echo "[layer-b] WARNING: report file not created"
fi

if [[ "$RC" -ne 0 ]]; then
  echo "[layer-b] FAIL (validator process exit $RC)"
  exit "$RC"
fi
if [[ "$NUM_ERRORS" != "0" ]]; then
  echo "[layer-b] FAIL (numErrors=$NUM_ERRORS; production gate requires 0)"
  exit 1
fi
echo "[layer-b] PASS (numErrors=0)"
exit 0
