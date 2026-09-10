#!/usr/bin/env bash
# Stage processor / top_rebuild / basisu / 3dtile into Tauri sidecar + resource dirs.
# Linux-focused (Phase 14). Windows → Phase 18.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TRIPLE="${GEOFORGE_TARGET_TRIPLE:-$(rustc -vV 2>/dev/null | awk '/host:/{print $2}')}"
TRIPLE="${TRIPLE:-x86_64-unknown-linux-gnu}"
BIN_DIR="$ROOT/apps/desktop/src-tauri/binaries"
RES_BIN="$ROOT/apps/desktop/src-tauri/resources/bin"
PROFILE="${GEOFORGE_STAGE_PROFILE:-debug}"
TARGET="$ROOT/target/$PROFILE"

mkdir -p "$BIN_DIR" "$RES_BIN"

stage_bin() {
  local src="$1" name="$2"
  if [[ -f "$src" ]]; then
    cp -f "$src" "$BIN_DIR/${name}-${TRIPLE}"
    cp -f "$src" "$BIN_DIR/${name}"
    chmod +x "$BIN_DIR/${name}-${TRIPLE}" "$BIN_DIR/${name}" || true
    echo "[stage] $name <- $src"
  else
    echo "[stage] SKIP $name (missing $src)" >&2
  fi
}

stage_bin "$TARGET/processor" processor
stage_bin "$TARGET/top_rebuild" top_rebuild

BASISU_SRC="${GEOFORGE_BASISU:-$ROOT/vcpkg_installed/x64-linux/tools/basisu/basisu}"
stage_bin "$BASISU_SRC" basisu

CONV_SRC="${GEOFORGE_3DTILE_DIR:-}"
if [[ -z "$CONV_SRC" ]]; then
  for cand in \
    "${GEOFORGE_RUNTIME:-}/3dtile-bin" \
    "$ROOT/.runtime/3dtile-bin" \
    "/workspace/runtime/3dtile-bin"
  do
    if [[ -n "$cand" && -d "$cand" && -e "$cand/_3dtile" ]]; then
      CONV_SRC="$cand"
      break
    fi
  done
fi

if [[ -n "${CONV_SRC:-}" && -d "$CONV_SRC" ]]; then
  echo "[stage] 3dtile runtime <- $CONV_SRC"
  # Preserve README.md if present
  README_BAK=""
  if [[ -f "$RES_BIN/README.md" ]]; then
    README_BAK="$(mktemp)"
    cp "$RES_BIN/README.md" "$README_BAK"
  fi
  rm -rf "$RES_BIN"
  mkdir -p "$RES_BIN"
  cp -a "$CONV_SRC/." "$RES_BIN/"
  find "$RES_BIN" -name '*.lib' -delete 2>/dev/null || true
  if [[ -n "$README_BAK" ]]; then
    cp "$README_BAK" "$RES_BIN/README.md"
    rm -f "$README_BAK"
  fi
  chmod +x "$RES_BIN/run.sh" "$RES_BIN/_3dtile" 2>/dev/null || true
else
  echo "[stage] SKIP 3dtile runtime (set GEOFORGE_3DTILE_DIR or GEOFORGE_RUNTIME)" >&2
  [[ -f "$RES_BIN/README.md" ]] || echo "stage converter here" > "$RES_BIN/README.md"
fi

echo "[stage] done → $BIN_DIR , $RES_BIN"
echo "[stage] triple=$TRIPLE profile=$PROFILE"
