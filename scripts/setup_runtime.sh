#!/usr/bin/env bash
# Extract a runnable _3dtile binary (+ libs) from the public Docker image
# without needing a local Docker daemon (uses crane).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLS="${TOOLS_DIR:-$ROOT/.runtime}"
mkdir -p "$TOOLS"
cd "$TOOLS"
if [[ ! -x crane ]]; then
  VER=v0.20.3
  curl -fsSL -o crane.tar.gz "https://github.com/google/go-containerregistry/releases/download/${VER}/go-containerregistry_Linux_x86_64.tar.gz"
  tar -xzf crane.tar.gz crane
  chmod +x crane
fi
if [[ ! -x 3dtile-bin/_3dtile ]]; then
  echo "Exporting winner1/3dtiles:1.0 (one-time, ~1GB extract)..."
  ./crane export winner1/3dtiles:1.0 3dtiles-fs.tar
  mkdir -p imgroot
  # extract only needed paths to avoid permission noise
  tar -xf 3dtiles-fs.tar -C imgroot 3dtiles/target/release/_3dtile 3dtiles/lib usr/lib64/libgeos* usr/lib64/libproj* 2>/dev/null || \
    tar -xf 3dtiles-fs.tar -C imgroot --wildcards '3dtiles/target/release/_3dtile' '3dtiles/lib/*' 'usr/lib64/libgeos*' 'usr/lib64/libproj*' || true
  mkdir -p 3dtile-bin/lib
  cp -a imgroot/3dtiles/target/release/_3dtile 3dtile-bin/
  cp -a imgroot/3dtiles/lib/. 3dtile-bin/lib/
  cp -a imgroot/usr/lib64/libgeos*.so* 3dtile-bin/lib/ 2>/dev/null || true
  cp -a imgroot/usr/lib64/libproj*.so* 3dtile-bin/lib/ 2>/dev/null || true
  cat > 3dtile-bin/run.sh <<'EOR'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export LD_LIBRARY_PATH="$HERE/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export OSG_LIBRARY_PATH="$HERE/lib/osgPlugins-3.7.0"
exec "$HERE/_3dtile" "$@"
EOR
  chmod +x 3dtile-bin/run.sh 3dtile-bin/_3dtile
  rm -f 3dtiles-fs.tar
  echo "Runtime ready: $TOOLS/3dtile-bin/run.sh"
else
  echo "Runtime already present: $TOOLS/3dtile-bin/run.sh"
fi
