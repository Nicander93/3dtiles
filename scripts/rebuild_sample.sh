#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IN="${1:-$ROOT/samples/OSGBny_3dtiles}"
OUT="${2:-$ROOT/samples/OSGBny_3dtiles_rebuild}"
PY="$ROOT/.venv/bin/python"
if [[ ! -x "$PY" ]]; then
  python3 -m venv "$ROOT/.venv"
  "$ROOT/.venv/bin/pip" install -r "$ROOT/tools/experiments/rebuild_top_py/requirements.txt"
  PY="$ROOT/.venv/bin/python"
fi
if [[ ! -f "$IN/tileset.json" ]]; then
  echo "missing $IN/tileset.json — run ./scripts/convert_sample.sh first" >&2
  exit 1
fi
"$PY" "$ROOT/tools/experiments/rebuild_top_py/rebuild_top.py" -i "$IN" -o "$OUT" --levels 1 --simplify 0.5 --texture-scale 0.5 -v
test -f "$OUT/tileset.json"
echo "Rebuilt: $OUT"
