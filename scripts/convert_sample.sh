#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/setup_runtime.sh"
"$ROOT/scripts/download_sample.sh" "$ROOT/samples/OSGBny"
RUN="$ROOT/.runtime/3dtile-bin/run.sh"
IN="$ROOT/samples/OSGBny"
OUT="$ROOT/samples/OSGBny_3dtiles"
rm -rf "$OUT"
mkdir -p "$OUT"
"$RUN" -f osgb -i "$IN" -o "$OUT" -v
test -f "$OUT/tileset.json"
echo "Converted: $OUT/tileset.json"
# stage preview
PREVIEW="$ROOT/examples/preview"
mkdir -p "$PREVIEW/tiles"
rm -rf "$PREVIEW/tiles"
cp -a "$OUT" "$PREVIEW/tiles"
echo "Preview files staged under $PREVIEW/tiles"
echo "Serve with: python3 -m http.server 8080 --directory $PREVIEW"
