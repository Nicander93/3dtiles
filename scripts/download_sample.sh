#!/usr/bin/env bash
# Public OSGB sample from fanvanzh/3dtiles issue #336 attachment
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/samples/OSGBny}"
mkdir -p "$(dirname "$OUT")"
TMP="$(mktemp -d)"
curl -fL -o "$TMP/OSGBny.zip" "https://github.com/user-attachments/files/22841916/OSGBny.zip"
unzip -o "$TMP/OSGBny.zip" -d "$TMP/unz"
# zip contains OSGBny/Data + metadata.xml
rm -rf "$OUT"
mkdir -p "$OUT"
if [[ -d "$TMP/unz/OSGBny" ]]; then
  cp -a "$TMP/unz/OSGBny/." "$OUT/"
else
  cp -a "$TMP/unz/." "$OUT/"
fi
rm -rf "$TMP"
test -f "$OUT/metadata.xml"
test -d "$OUT/Data"
echo "Sample ready: $OUT"
