#!/usr/bin/env bash
# Launch native Qt/OSG OSGB viewer with conda env + library paths.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
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

MAMBA=""
if [[ -x /home/box/bin/micromamba ]]; then
  MAMBA=/home/box/bin/micromamba
elif command -v micromamba >/dev/null 2>&1; then
  MAMBA="$(command -v micromamba)"
elif [[ -x "$MAMBA_ROOT_PREFIX/bin/micromamba" ]]; then
  MAMBA="$MAMBA_ROOT_PREFIX/bin/micromamba"
fi

if [[ -z "$MAMBA" ]]; then
  echo "micromamba not found; cannot activate osgb_viewer env (MAMBA_ROOT_PREFIX=$MAMBA_ROOT_PREFIX)" >&2
  exit 1
fi

# shellcheck disable=SC1091
eval "$("$MAMBA" shell hook -s bash)"
micromamba activate osgb_viewer

export LD_LIBRARY_PATH="${CONDA_PREFIX}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# OSG 3.6.5 plugins (IVE reader for .osgb)
if [[ -d "${CONDA_PREFIX}/lib/osgPlugins-3.6.5" ]]; then
  export OSG_LIBRARY_PATH="${CONDA_PREFIX}/lib/osgPlugins-3.6.5"
elif [[ -d "${CONDA_PREFIX}/lib/osgPlugins-3.6.4" ]]; then
  export OSG_LIBRARY_PATH="${CONDA_PREFIX}/lib/osgPlugins-3.6.4"
else
  export OSG_LIBRARY_PATH="${CONDA_PREFIX}/lib${OSG_LIBRARY_PATH:+:$OSG_LIBRARY_PATH}"
fi

BIN="${GEOFORGE_OSGB_VIEWER:-$ROOT/apps/osgb_viewer/build/osgb_viewer}"
if [[ ! -x "$BIN" ]]; then
  echo "osgb_viewer binary not found or not executable: $BIN" >&2
  exit 1
fi

exec "$BIN" "$@"
