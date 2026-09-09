#!/usr/bin/env bash
# Start GeoForge 3D local desktop API (uvicorn :8787)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export GEOFORGE_DB="${GEOFORGE_DB:-$ROOT/.geoforge/tasks.db}"

# Runtime root: env override, else /workspace/runtime (box default), else $ROOT/.runtime
RUNTIME_ROOT="${GEOFORGE_RUNTIME:-}"
if [[ -z "$RUNTIME_ROOT" ]]; then
  if [[ -d /workspace/runtime ]]; then
    RUNTIME_ROOT="/workspace/runtime"
  else
    RUNTIME_ROOT="$ROOT/.runtime"
  fi
fi

# Prefer KTX2-capable wrapper (basisu post-process) when present
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

export GEOFORGE_REBUILD_TOP="${GEOFORGE_REBUILD_TOP:-$ROOT/tools/rebuild_top/rebuild_top.py}"

# Python / venv: GEOFORGE_VENV or common locations
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

# Preview cache (repo-local by default; override with GEOFORGE_PREVIEW_CACHE)
export GEOFORGE_PREVIEW_CACHE="${GEOFORGE_PREVIEW_CACHE:-$ROOT/.geoforge/preview_cache}"
SAMPLE_HINT="${GEOFORGE_SAMPLE:-${GEOFORGE_SAMPLE_OSGB:-/workspace/data/OSGBny/OSGBny}}"
mkdir -p "$(dirname "$GEOFORGE_DB")" "$GEOFORGE_PREVIEW_CACHE"

if [[ -x "$VENV/bin/uvicorn" ]]; then
  UVICORN="$VENV/bin/uvicorn"
elif [[ -x "$VENV/bin/python" ]]; then
  UVICORN="$VENV/bin/python -m uvicorn"
else
  UVICORN="python3 -m uvicorn"
fi

export PYTHONPATH="$ROOT/apps/desktop_server${PYTHONPATH:+:$PYTHONPATH}"
cd "$ROOT/apps/desktop_server"
echo "[geoforge] GeoForge 3D v0.1.0"
echo "[geoforge] UI / API:  http://127.0.0.1:8787/"
echo "[geoforge] Health:    http://127.0.0.1:8787/api/health"
echo "[geoforge] Convert:   $GEOFORGE_3DTILE"
echo "[geoforge] Sample:    $SAMPLE_HINT  (set GEOFORGE_SAMPLE to override)"
echo "[geoforge] Runtime:   $RUNTIME_ROOT  (set GEOFORGE_RUNTIME to override)"
echo "[geoforge] Docs:      $ROOT/docs/product/USER_GUIDE.md"
WEB_DIST="$ROOT/apps/web/dist"
if [[ ! -f "$WEB_DIST/index.html" ]]; then echo "[geoforge] WARNING: build apps/web"; else echo "[geoforge] Serving UI from $WEB_DIST"; fi
exec $UVICORN app.main:app --host 127.0.0.1 --port 8787 --reload
