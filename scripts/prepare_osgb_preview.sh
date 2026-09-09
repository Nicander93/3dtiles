#!/usr/bin/env bash
# Convert public OSGB root tiles to GLB for Three.js preview
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/setup_runtime.sh"
"$ROOT/scripts/download_sample.sh" "$ROOT/samples/OSGBny"
RUN="$ROOT/.runtime/3dtile-bin/run.sh"
OUT="$ROOT/examples/preview/osgb_glb"
rm -rf "$OUT" && mkdir -p "$OUT"
for d in "$ROOT"/samples/OSGBny/Data/Tile_*; do
  name=$(basename "$d")
  src="$d/$name.osgb"
  [[ -f "$src" ]] || continue
  "$RUN" -f gltf -i "$src" -o "$OUT/$name.glb"
done
python3 - <<PY
import json
from pathlib import Path
root=Path("$OUT")
files=sorted(root.glob("*.glb"))
meta=(Path("$ROOT/samples/OSGBny/metadata.xml")).read_text(errors="replace")
json.dump({
  "source":"OSGBny public oblique sample",
  "models":[{"name":f.stem,"url":"./osgb_glb/"+f.name,"bytes":f.stat().st_size} for f in files],
  "metadata_xml_head": meta[:1500],
}, open(Path("$ROOT/examples/preview/osgb_models.json"),"w"), ensure_ascii=False, indent=2)
print("wrote", len(files), "glb models")
PY
