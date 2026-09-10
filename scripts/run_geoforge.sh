#!/usr/bin/env bash
# GeoForge 3D launcher (Phase 4+)
# Default: print how to run Tauri desktop + processor; optional --legacy-server for old Python API.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
  cat <<'USAGE'
GeoForge 3D — run helper

Preferred (Phase 3+):
  1) Build / run processor:  cargo build -p processor
  2) Desktop (Tauri):        cd apps/desktop && npm install && npm run tauri:dev
     (needs DISPLAY + WebKitGTK on Linux; compile-only: cd apps/desktop/src-tauri && cargo build)

Browser UI without Tauri (static Vite build):
  cd apps/desktop && npm run build && npm run preview

Legacy Python HTTP API (reference only; not primary V1 path):
  bash scripts/run_geoforge.sh --legacy-server

Env (shared):
  GEOFORGE_3DTILE, GEOFORGE_REBUILD_TOP, GEOFORGE_PYTHON, GEOFORGE_RUNTIME, GEOFORGE_DB
USAGE
}

export GEOFORGE_DB="${GEOFORGE_DB:-$ROOT/.geoforge/tasks.db}"

RUNTIME_ROOT="${GEOFORGE_RUNTIME:-}"
if [[ -z "$RUNTIME_ROOT" ]]; then
  if [[ -d /workspace/runtime ]]; then
    RUNTIME_ROOT="/workspace/runtime"
  else
    RUNTIME_ROOT="$ROOT/.runtime"
  fi
fi

if [[ -z "${GEOFORGE_3DTILE:-}" ]]; then
  for candidate in \
    "$RUNTIME_ROOT/3dtile-bin-ktx2/run.sh" \
    "$RUNTIME_ROOT/3dtile-bin/run.sh" \
    "$ROOT/.runtime/3dtile-bin-ktx2/run.sh" \
    "$ROOT/.runtime/3dtile-bin/run.sh"; do
    if [[ -x "$candidate" ]]; then
      export GEOFORGE_3DTILE="$candidate"
      break
    fi
  done
  export GEOFORGE_3DTILE="${GEOFORGE_3DTILE:-$RUNTIME_ROOT/3dtile-bin/run.sh}"
fi

# New path first; one-release fallback to legacy tools/rebuild_top
if [[ -z "${GEOFORGE_REBUILD_TOP:-}" ]]; then
  if [[ -f "$ROOT/tools/experiments/rebuild_top_py/rebuild_top.py" ]]; then
    export GEOFORGE_REBUILD_TOP="$ROOT/tools/experiments/rebuild_top_py/rebuild_top.py"
  elif [[ -f "$ROOT/tools/rebuild_top/rebuild_top.py" ]]; then
    export GEOFORGE_REBUILD_TOP="$ROOT/tools/rebuild_top/rebuild_top.py"
  else
    export GEOFORGE_REBUILD_TOP="$ROOT/tools/experiments/rebuild_top_py/rebuild_top.py"
  fi
fi

VENV="${GEOFORGE_VENV:-}"
if [[ -z "$VENV" ]]; then
  for candidate in /workspace/venv-3dtiles "$ROOT/.venv" "$ROOT/venv"; do
    if [[ -d "$candidate" ]]; then
      VENV="$candidate"
      break
    fi
  done
  VENV="${VENV:-/workspace/venv-3dtiles}"
fi
export GEOFORGE_PYTHON="${GEOFORGE_PYTHON:-$VENV/bin/python}"

LEGACY=0
case "${1:-}" in
  -h|--help) usage; exit 0 ;;
  --legacy-server|--legacy) LEGACY=1; shift || true ;;
  "") ;;
  *)
    echo "Unknown option: $1" >&2
    usage >&2
    exit 2
    ;;
esac

if [[ "$LEGACY" -eq 0 ]]; then
  echo "[geoforge] Preferred path: Tauri desktop + processor (no Python HTTP required for happy path)."
  echo "[geoforge]   cd $ROOT/apps/desktop && npm run tauri:dev"
  echo "[geoforge]   cargo build -p processor   # from repo root"
  echo "[geoforge] Convert binary: ${GEOFORGE_3DTILE}"
  echo "[geoforge] Rebuild script: ${GEOFORGE_REBUILD_TOP}"
  echo "[geoforge] Docs:           $ROOT/docs/product/USER_GUIDE.md"
  echo "[geoforge] Legacy API:     bash scripts/run_geoforge.sh --legacy-server"
  # If tauri CLI + node_modules present, offer to start tauri:dev when DISPLAY set
  if [[ -n "${DISPLAY:-}" && -x "$ROOT/apps/desktop/node_modules/.bin/tauri" ]]; then
    echo "[geoforge] DISPLAY=$DISPLAY — starting apps/desktop tauri:dev …"
    cd "$ROOT/apps/desktop"
    exec npm run tauri:dev
  fi
  exit 0
fi

# --- legacy Python desktop_server ---
export GEOFORGE_PREVIEW_CACHE="${GEOFORGE_PREVIEW_CACHE:-$ROOT/.geoforge/preview_cache}"
SAMPLE_HINT="${GEOFORGE_SAMPLE:-${GEOFORGE_SAMPLE_OSGB:-/workspace/data/OSGBny/OSGBny}}"
mkdir -p "$(dirname "$GEOFORGE_DB")" "$GEOFORGE_PREVIEW_CACHE"

SERVER_ROOT="$ROOT/tools/experiments/desktop_server_py"
if [[ ! -d "$SERVER_ROOT" ]]; then
  echo "[geoforge] legacy server missing: $SERVER_ROOT" >&2
  exit 1
fi

if [[ -x "$VENV/bin/uvicorn" ]]; then
  UVICORN="$VENV/bin/uvicorn"
elif [[ -x "$VENV/bin/python" ]]; then
  UVICORN="$VENV/bin/python -m uvicorn"
else
  UVICORN="python3 -m uvicorn"
fi

export PYTHONPATH="$SERVER_ROOT${PYTHONPATH:+:$PYTHONPATH}"
cd "$SERVER_ROOT"
echo "[geoforge] LEGACY Python API (reference)"
echo "[geoforge] UI / API:  http://127.0.0.1:8787/"
echo "[geoforge] Health:    http://127.0.0.1:8787/api/health"
echo "[geoforge] Convert:   $GEOFORGE_3DTILE"
echo "[geoforge] Rebuild:   $GEOFORGE_REBUILD_TOP"
echo "[geoforge] Sample:    $SAMPLE_HINT"
echo "[geoforge] Prefer:    cd apps/desktop && npm run tauri:dev"
WEB_DIST="$ROOT/apps/desktop/dist"
if [[ ! -f "$WEB_DIST/index.html" ]]; then
  echo "[geoforge] WARNING: build UI with: cd apps/desktop && npm run build"
else
  echo "[geoforge] Serving UI from $WEB_DIST"
fi
exec $UVICORN app.main:app --host 127.0.0.1 --port 8787 --reload
