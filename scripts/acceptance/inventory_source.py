#!/usr/bin/env python3
"""Source inventory for staged OSGB (plan §11.1)."""
from __future__ import annotations

import argparse
import json
import os
import random
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))


def inventory(staged_root: Path, sample_blocks: int = 10) -> dict:
    data = staged_root / "Data"
    meta = staged_root / "metadata.xml"
    tiles = sorted([p for p in data.iterdir() if p.is_dir()]) if data.is_dir() else []
    osgb_files = list(staged_root.rglob("*.osgb")) if staged_root.exists() else []
    textures = []
    for pat in ("*.jpg", "*.jpeg", "*.png", "*.dds", "*.webp"):
        textures.extend(staged_root.rglob(pat))

    input_bytes = 0
    for f in osgb_files + textures:
        try:
            input_bytes += f.stat().st_size
        except OSError:
            pass

    lod_counter: Counter[str] = Counter()
    for f in osgb_files:
        name = f.name
        # Tile_xxx_L12_... or tile_x_y_L12
        if "_L" in name:
            part = name.split("_L", 1)[1]
            lod = part.split("_", 1)[0]
            lod = "".join(ch for ch in lod if ch.isdigit()) or "root"
            lod_counter[lod] += 1
        else:
            lod_counter["root"] += 1

    meta_text = meta.read_text(errors="replace") if meta.is_file() else ""
    srs = None
    origin = None
    if "<SRS>" in meta_text:
        srs = meta_text.split("<SRS>", 1)[1].split("</SRS>", 1)[0].strip()
    if "<SRSOrigin>" in meta_text:
        origin = meta_text.split("<SRSOrigin>", 1)[1].split("</SRSOrigin>", 1)[0].strip()

    rng = random.Random(42)
    sample = tiles[:]
    rng.shuffle(sample)
    sample = sample[:sample_blocks]
    block_samples = []
    for t in sample:
        files = sorted(t.glob("*.osgb"))
        block_samples.append(
            {
                "name": t.name,
                "osgbCount": len(files),
                "entryExists": (t / f"{t.name}.osgb").is_file(),
                "filesHead": [f.name for f in files[:12]],
            }
        )

    return {
        "schema": "geoforge.acceptance.source_inventory.v1",
        "stagedRoot": str(staged_root),
        "gridCount": len(tiles),
        "osgbFileCount": len(osgb_files),
        "textureFileCount": len(textures),
        "inputBytes": input_bytes,
        "lodDepthDistribution": dict(sorted(lod_counter.items(), key=lambda kv: kv[0])),
        "metadataCRS": srs,
        "origin": origin,
        "rootTransform": None,
        "sampledBlocks": block_samples,
        "createdAt": datetime.now(timezone.utc).isoformat(),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--staged", type=Path, required=True)
    ap.add_argument("-o", "--output", type=Path, required=True)
    args = ap.parse_args()
    inv = inventory(args.staged)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(inv, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({k: inv[k] for k in ("gridCount", "osgbFileCount", "inputBytes", "metadataCRS")}, indent=2))
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
