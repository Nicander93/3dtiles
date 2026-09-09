#!/usr/bin/env bash
# User-space deps for apps/osgb_viewer (no root/apt).
# Installs micromamba (if needed) + conda-forge env: Qt6 + OpenSceneGraph + toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAMBA_BIN="${MAMBA_BIN:-$HOME/bin/micromamba}"
export MAMBA_ROOT_PREFIX="${MAMBA_ROOT_PREFIX:-/workspace/conda}"
ENV_NAME="${ENV_NAME:-osgb_viewer}"

mkdir -p "$(dirname "$MAMBA_BIN")" "$MAMBA_ROOT_PREFIX"

if [[ ! -x "$MAMBA_BIN" ]]; then
  echo "Downloading micromamba..."
  TMP="$(mktemp -d)"
  curl -Ls https://micro.mamba.pm/api/micromamba/linux-64/latest -o "$TMP/micromamba.tar.bz2"
  python3 - <<PY
import bz2, tarfile, io, os
raw = bz2.decompress(open("$TMP/micromamba.tar.bz2","rb").read())
tf = tarfile.open(fileobj=io.BytesIO(raw), mode="r:")
member = next(m for m in tf.getmembers() if m.name.endswith("bin/micromamba"))
tf.extract(member, "$TMP")
os.replace("$TMP/" + member.name, "$MAMBA_BIN")
os.chmod("$MAMBA_BIN", 0o755)
print("installed", "$MAMBA_BIN")
PY
  rm -rf "$TMP"
fi

"$MAMBA_BIN" --version

# Pins validated on this box (2026-09-09):
#   openscenegraph 3.6.5, qt6-main 6.11.2, cmake 4.4.3, gxx 15.3.0
"$MAMBA_BIN" create -y -n "$ENV_NAME" -c conda-forge \
  "cxx-compiler" "cmake>=3.22" ninja pkg-config \
  "openscenegraph=3.6.5" "qt6-main>=6.5" \
  libgl mesa-libgl-devel-cos7-x86_64 xorg-libx11 xorg-libxext

cat <<MSG

Deps ready. Activate and build:

  export MAMBA_ROOT_PREFIX=$MAMBA_ROOT_PREFIX
  eval "\$($MAMBA_BIN shell hook -s bash)"
  micromamba activate $ENV_NAME
  cd $ROOT
  cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="\$CONDA_PREFIX"
  cmake --build build -j
  export LD_LIBRARY_PATH="\$CONDA_PREFIX/lib:\${LD_LIBRARY_PATH:-}"
  export OSG_LIBRARY_PATH="\$CONDA_PREFIX/lib/osgPlugins-3.6.5"
  ./build/osgb_headless_load /workspace/data/OSGBny/OSGBny
  ./build/osgb_viewer /workspace/data/OSGBny/OSGBny

MSG
