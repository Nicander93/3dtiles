#!/usr/bin/env bash
# Launch GeoForge Qt shell (geoforge_shell) with conda env + DISPLAY.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Conda/micromamba root: env override, else /workspace/conda when present
if [[ -z "${MAMBA_ROOT_PREFIX:-}" ]]; then
  if [[ -d /workspace/conda ]]; then
    export MAMBA_ROOT_PREFIX="/workspace/conda"
  elif [[ -d "$HOME/conda" ]]; then
    export MAMBA_ROOT_PREFIX="$HOME/conda"
  else
    export MAMBA_ROOT_PREFIX="${HOME}/.local/share/mamba"
  fi
fi
export DISPLAY="${DISPLAY:-:2}"
API_URL="${GEOFORGE_API_URL:-http://127.0.0.1:8787/api/health}"

MAMBA=""
if [[ -x /home/box/bin/micromamba ]]; then
  MAMBA=/home/box/bin/micromamba
elif command -v micromamba >/dev/null 2>&1; then
  MAMBA="$(command -v micromamba)"
elif [[ -x "$MAMBA_ROOT_PREFIX/bin/micromamba" ]]; then
  MAMBA="$MAMBA_ROOT_PREFIX/bin/micromamba"
fi

ENV_PREFIX="${CONDA_PREFIX:-}"
if [[ -z "$ENV_PREFIX" || "$(basename "$ENV_PREFIX")" != "osgb_viewer" ]]; then
  if [[ -n "$MAMBA" ]]; then
    # shellcheck disable=SC1091
    eval "$("$MAMBA" shell hook -s bash)"
    micromamba activate osgb_viewer
  elif [[ -d "$MAMBA_ROOT_PREFIX/envs/osgb_viewer" ]]; then
    # Fallback: source activate without micromamba
    # shellcheck disable=SC1091
    source "$MAMBA_ROOT_PREFIX/etc/profile.d/conda.sh" 2>/dev/null || true
    if command -v conda >/dev/null 2>&1; then
      conda activate osgb_viewer
    else
      export PATH="$MAMBA_ROOT_PREFIX/envs/osgb_viewer/bin:$PATH"
      export CONDA_PREFIX="$MAMBA_ROOT_PREFIX/envs/osgb_viewer"
    fi
  else
    echo "osgb_viewer env not found under MAMBA_ROOT_PREFIX=$MAMBA_ROOT_PREFIX; micromamba/conda required" >&2
    exit 1
  fi
fi

export LD_LIBRARY_PATH="${CONDA_PREFIX}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

if ! curl -sf -m 2 "$API_URL" >/dev/null 2>&1; then
  echo "[geoforge_shell] API not reachable at $API_URL" >&2
  echo "[geoforge_shell] Start the desktop API first: $ROOT/scripts/run_geoforge.sh" >&2
  exit 1
fi

BIN="${GEOFORGE_SHELL:-$ROOT/apps/geoforge_shell/build/geoforge_shell}"
if [[ ! -x "$BIN" ]]; then
  echo "geoforge_shell binary not found or not executable: $BIN" >&2
  exit 1
fi

exec "$BIN" "$@"
